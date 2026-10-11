//! At most two expensive query reductions run across all retained live views.
//! Cached responses do not occupy a slot; RAII releases it on errors or unwinding.
use std::sync::{Condvar, Mutex};
static ACTIVE: Mutex<usize> = Mutex::new(0);
static READY: Condvar = Condvar::new();
pub(super) struct Permit;
pub(super) fn acquire() -> Permit {
    let mut active = ACTIVE.lock().unwrap_or_else(|e| e.into_inner());
    while *active >= 2 {
        active = READY.wait(active).unwrap_or_else(|e| e.into_inner());
    }
    *active += 1;
    Permit
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
    fn concurrent_reductions_never_exceed_two_and_release_after_failure() {
        let running = AtomicUsize::new(0);
        let peak = AtomicUsize::new(0);
        std::thread::scope(|scope| {
            for _ in 0..8 {
                scope.spawn(|| {
                    let _permit = acquire();
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
            let _permit = acquire();
            anyhow::bail!("synthetic query failure")
        };
        assert!(failure().is_err());
        drop(acquire());
    }
}
