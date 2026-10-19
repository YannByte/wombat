//! At most two expensive query reductions run across all retained live views.
//! Cached responses do not occupy a slot; RAII releases it on errors or unwinding.
use anyhow::Result;
use std::sync::{Condvar, Mutex};
use std::{
    cell::RefCell,
    time::{Duration, Instant},
};
static ACTIVE: Mutex<usize> = Mutex::new(0);
static READY: Condvar = Condvar::new();
struct Work {
    deadline: Instant,
    metadata: usize,
    bytes: usize,
}
thread_local! { static WORK: RefCell<Option<Work>> = const { RefCell::new(None) }; }
pub(crate) struct Budget(bool);
impl Budget {
    pub(crate) fn enter() -> Self {
        Self(WORK.with_borrow_mut(|work| {
            if work.is_some() {
                return false;
            }
            *work = Some(Work {
                deadline: Instant::now() + Duration::from_secs(10),
                metadata: 0,
                bytes: 0,
            });
            true
        }))
    }
}
impl Drop for Budget {
    fn drop(&mut self) {
        if self.0 {
            WORK.with_borrow_mut(|work| *work = None);
        }
    }
}
pub(crate) fn expired() -> bool {
    WORK.with_borrow(|work| work.as_ref().is_some_and(|w| Instant::now() >= w.deadline))
}
pub(crate) fn charge(metadata: usize, bytes: usize) -> Result<()> {
    WORK.with_borrow_mut(|work| {
        if let Some(w) = work {
            w.metadata = w.metadata.saturating_add(metadata);
            w.bytes = w.bytes.saturating_add(bytes);
            if Instant::now() >= w.deadline
                || w.metadata > 16_000_000
                || w.bytes > 1024 * 1024 * 1024
            {
                return Err(crate::dto::operation_error(
                    "RESOURCE_LIMIT",
                    "Query execution budget exceeded",
                ));
            }
        }
        Ok(())
    })
}
pub(crate) fn query_error(error: anyhow::Error) -> anyhow::Error {
    if error.downcast_ref::<rusqlite::Error>().is_some_and(|e| matches!(e,
        rusqlite::Error::SqliteFailure(code, _) if code.code == rusqlite::ErrorCode::OperationInterrupted)) {
        crate::dto::operation_error("RESOURCE_LIMIT", "Query execution budget exceeded")
    } else { error }
}
pub(crate) fn lock<T>(mutex: &Mutex<T>) -> Result<std::sync::MutexGuard<'_, T>> {
    loop {
        charge(0, 0)?;
        match mutex.try_lock() {
            Ok(guard) => return Ok(guard),
            Err(std::sync::TryLockError::Poisoned(_)) => {
                return Err(crate::dto::operation_error(
                    "RESOURCE_LIMIT",
                    "Query worker unavailable",
                ));
            }
            Err(std::sync::TryLockError::WouldBlock) => {
                std::thread::sleep(Duration::from_millis(1))
            }
        }
    }
}
pub(crate) struct SqlBudget<'a>(&'a rusqlite::Connection);
impl<'a> SqlBudget<'a> {
    pub(crate) fn new(db: &'a rusqlite::Connection) -> Result<Self> {
        db.progress_handler(1000, Some(expired))?;
        Ok(Self(db))
    }
}
impl Drop for SqlBudget<'_> {
    fn drop(&mut self) {
        // Registration checked ownership; this borrow prevents closing or
        // transferring the connection while the handler is installed.
        self.0
            .progress_handler(0, None::<fn() -> bool>)
            .expect("query connection remains owned");
    }
}
pub(super) struct Permit;
pub(super) fn acquire() -> Result<Permit> {
    let mut active = ACTIVE.lock().unwrap_or_else(|e| e.into_inner());
    while *active >= 2 {
        charge(0, 0)?;
        active = READY
            .wait_timeout(active, Duration::from_millis(50))
            .unwrap_or_else(|e| e.into_inner())
            .0;
    }
    *active += 1;
    Ok(Permit)
}
impl Drop for Permit {
    fn drop(&mut self) {
        let mut active = ACTIVE.lock().unwrap_or_else(|e| e.into_inner());
        *active -= 1;
        READY.notify_one();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::{AtomicUsize, Ordering};
    #[test]
    fn sqlite_deadline_interrupts_computation_and_clears_its_handler() {
        let db = rusqlite::Connection::open_in_memory().unwrap();
        let budget = Budget::enter();
        WORK.with_borrow_mut(|work| work.as_mut().unwrap().deadline = Instant::now());
        let error = {
            let _sql = SqlBudget::new(&db).unwrap();
            db.query_row("WITH RECURSIVE n(x) AS (VALUES(0) UNION ALL SELECT x+1 FROM n WHERE x<10000000) SELECT sum(x) FROM n", [], |r| r.get::<_, i64>(0)).unwrap_err()
        };
        assert_eq!(
            query_error(error.into())
                .downcast_ref::<crate::dto::OperationError>()
                .unwrap()
                .code,
            "RESOURCE_LIMIT"
        );
        // A failed request cannot leave a handler on the leased connection.
        let n: i64 = db.query_row("SELECT 42", [], |r| r.get(0)).unwrap();
        assert_eq!(n, 42);
        drop(budget);
        let _new = Budget::enter();
        assert!(!expired());
    }
    #[test]
    fn nested_reductions_share_byte_and_metadata_budgets() {
        let _budget = Budget::enter();
        charge(16_000_000, 1024 * 1024 * 1024).unwrap();
        {
            let _nested = Budget::enter();
            assert!(charge(1, 0).is_err());
        }
        assert!(charge(0, 0).is_err());
    }
    #[test]
    fn concurrent_reductions_never_exceed_two_and_release_after_failure() {
        let running = AtomicUsize::new(0);
        let peak = AtomicUsize::new(0);
        std::thread::scope(|scope| {
            for _ in 0..8 {
                scope.spawn(|| {
                    let _permit = acquire().unwrap();
                    let n = running.fetch_add(1, Ordering::SeqCst) + 1;
                    peak.fetch_max(n, Ordering::SeqCst);
                    std::thread::sleep(std::time::Duration::from_millis(5));
                    running.fetch_sub(1, Ordering::SeqCst);
                });
            }
        });
        assert!(peak.load(Ordering::SeqCst) <= 2);
        assert_eq!(running.load(Ordering::SeqCst), 0);
        let failure = || -> anyhow::Result<()> {
            let _permit = acquire()?;
            anyhow::bail!("synthetic query failure")
        };
        assert!(failure().is_err());
        drop(acquire().unwrap());
    }
}
