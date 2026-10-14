//! Coalesce requests per source while preserving explicit verification strength.
use super::*;

const AUTO_REFRESH_INTERVAL: Duration = Duration::from_secs(300);

pub(super) fn auto_due(entry: &Entry) -> bool {
    !entry.syncing && entry.pending.is_none() && entry.last_sync.elapsed() >= AUTO_REFRESH_INTERVAL
}

/// Wake readers even when the writer returns early or unwinds outside the state lock.
pub(super) struct WorkerLifecycle {
    shared: Shared,
    completed: bool,
}
impl WorkerLifecycle {
    pub(super) fn new(shared: Shared) -> Self {
        Self {
            shared,
            completed: false,
        }
    }
    pub(super) fn completed(&mut self) {
        self.completed = true;
    }
}
impl Drop for WorkerLifecycle {
    fn drop(&mut self) {
        if self.completed {
            return;
        }
        let mut entries = self
            .shared
            .0
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        for entry in entries.values_mut() {
            entry.error_code = Some("CORE_UNAVAILABLE");
            entry.error = Some("后台同步意外停止，请重试".into());
            entry.completed = entry.requested;
            entry.running = None;
            entry.pending = None;
            entry.syncing = false;
        }
        self.shared.0.clear_poison();
        self.shared.1.notify_all();
    }
}

#[cfg(test)]
pub(super) fn request(
    entry: &mut Entry,
    key: &str,
    verify: bool,
    jobs: &mpsc::SyncSender<Job>,
) -> Result<u64> {
    request_work(entry, key, verify, false, jobs)
}
pub(super) fn request_work(
    entry: &mut Entry,
    key: &str,
    verify: bool,
    rebuild: bool,
    jobs: &mpsc::SyncSender<Job>,
) -> Result<u64> {
    if let Some(work) = &mut entry.pending {
        work.verify |= verify;
        work.rebuild |= rebuild;
        work.include_history |= entry.include_history;
        return Ok(work.ticket);
    }
    if let Some(work) = entry.running
        && (!verify || work.verify)
        && (!rebuild || work.rebuild)
        && (!entry.include_history || work.include_history)
    {
        return Ok(work.ticket);
    }
    let ticket = entry.requested + 1;
    jobs.try_send(Job {
        key: key.into(),
        restore_only: false,
    })
    .map_err(|error| match error {
        mpsc::TrySendError::Full(_) => {
            operation_error("UPDATE_BUSY", "同步请求队列已满，请稍后重试")
        }
        mpsc::TrySendError::Disconnected(_) => {
            operation_error("CORE_UNAVAILABLE", "后台同步服务已停止，请重新启动 Wombat")
        }
    })?;
    entry.requested = ticket;
    entry.pending = Some(SyncWork {
        ticket,
        verify,
        rebuild,
        include_history: entry.include_history,
    });
    entry.syncing = true;
    entry.following = true;
    Ok(ticket)
}

pub(super) fn begin(entry: &mut Entry, queued: bool) -> Option<SyncWork> {
    let work = if queued {
        entry.pending.take()?
    } else {
        if entry.pending.is_some() {
            return None;
        }
        // Background work participates in the same completion/waiter protocol.
        entry.requested += 1;
        SyncWork {
            ticket: entry.requested,
            verify: false,
            rebuild: false,
            include_history: entry.include_history,
        }
    };
    entry.running = Some(work);
    entry.syncing = true;
    Some(work)
}

pub(super) fn finish(entry: &mut Entry, work: SyncWork) {
    entry.completed = work.ticket;
    entry.running = None;
    entry.syncing = entry.pending.is_some();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn worker_panic_wakes_waiters_and_retains_views() {
        let root = tempfile::tempdir().unwrap();
        let view = Arc::new(
            crate::usage_store::memory(
                Default::default(),
                "live:a:old".into(),
                crate::pricing_sync::current_at(root.path()).unwrap(),
                None,
            )
            .unwrap(),
        );
        let mut entry = Entry::new(vec![]);
        entry.views.push_back((Instant::now(), view));
        let (tx, rx) = mpsc::sync_channel(1);
        request(&mut entry, "a", false, &tx).unwrap();
        rx.try_recv().unwrap();
        begin(&mut entry, true).unwrap();
        request(&mut entry, "a", true, &tx).unwrap();
        let shared: Shared = Arc::new((
            Mutex::new(BTreeMap::from([("a".into(), entry)])),
            Condvar::new(),
        ));
        let worker_shared = Arc::clone(&shared);
        let worker = std::thread::spawn(move || {
            let _lifecycle = WorkerLifecycle::new(Arc::clone(&worker_shared));
            let _receiver = rx;
            panic!("synthetic adapter failure");
        });
        let entries = shared.0.lock().unwrap();
        let (entries, _) = shared
            .1
            .wait_timeout_while(entries, Duration::from_secs(2), |entries| {
                entries["a"].syncing
            })
            .unwrap();
        let entry = &entries["a"];
        assert!(!entry.syncing);
        assert_eq!(entry.error_code, Some("CORE_UNAVAILABLE"));
        assert_eq!(entry.completed, entry.requested);
        assert!(entry.running.is_none() && entry.pending.is_none());
        assert_eq!(entry.views.len(), 1);
        drop(entries);
        assert!(worker.join().is_err());
        let mut entries = shared.0.lock().unwrap();
        let error = request(entries.get_mut("a").unwrap(), "a", false, &tx).unwrap_err();
        assert_eq!(
            error
                .downcast_ref::<crate::dto::OperationError>()
                .unwrap()
                .code,
            "CORE_UNAVAILABLE"
        );
    }

    #[test]
    fn slow_sync_coalesces_repeated_queries_and_verification() {
        let (tx, rx) = mpsc::sync_channel(32);
        let mut entry = Entry::new(vec![]);
        let first = request(&mut entry, "a", false, &tx).unwrap();
        for _ in 0..100 {
            assert_eq!(request(&mut entry, "a", false, &tx).unwrap(), first);
        }
        assert_eq!(rx.try_recv().unwrap().key, "a");
        let running = begin(&mut entry, true).unwrap();
        for _ in 0..100 {
            assert_eq!(request(&mut entry, "a", false, &tx).unwrap(), first);
        }
        assert!(rx.try_recv().is_err());
        let verified = request(&mut entry, "a", true, &tx).unwrap();
        assert!(verified > first);
        for _ in 0..100 {
            assert_eq!(request(&mut entry, "a", true, &tx).unwrap(), verified);
        }
        finish(&mut entry, running);
        assert!(entry.syncing);
        assert!(entry.completed < verified);
        rx.try_recv().unwrap();
        let running = begin(&mut entry, true).unwrap();
        assert!(running.verify);
        assert_eq!(request(&mut entry, "a", true, &tx).unwrap(), verified);
        finish(&mut entry, running);
        assert_eq!(entry.completed, verified);
        assert!(!entry.syncing);
        assert!(rx.try_recv().is_err());
        assert!(request(&mut entry, "a", false, &tx).unwrap() > verified);
    }

    #[test]
    fn queued_work_upgrades_and_background_work_accepts_waiters() {
        let (tx, rx) = mpsc::sync_channel(1);
        let mut entry = Entry::new(vec![]);
        let ticket = request(&mut entry, "a", false, &tx).unwrap();
        assert_eq!(request(&mut entry, "a", true, &tx).unwrap(), ticket);
        // A request can arrive after the worker selected a background source.
        assert!(begin(&mut entry, false).is_none());
        rx.try_recv().unwrap();
        let work = begin(&mut entry, true).unwrap();
        assert!(work.verify);
        finish(&mut entry, work);
        let background = begin(&mut entry, false).unwrap();
        assert_eq!(
            request(&mut entry, "a", false, &tx).unwrap(),
            background.ticket
        );
        assert!(rx.try_recv().is_err());
        finish(&mut entry, background);
        assert_eq!(entry.completed, background.ticket);
    }

    #[test]
    fn timed_out_readers_reuse_work_and_fresh_reader_wakes_on_completion() {
        let root = tempfile::tempdir().unwrap();
        let roots = vec![root.path().to_string_lossy().into_owned()];
        let key = source_key(&roots);
        let view = Arc::new(
            crate::usage_store::memory(
                Default::default(),
                format!("live:{key}:test"),
                crate::pricing_sync::current_at(root.path()).unwrap(),
                None,
            )
            .unwrap(),
        );
        let mut entry = Entry::new(roots.clone());
        entry.views.push_back((Instant::now(), view));
        let shared: Shared = Arc::new((
            Mutex::new(BTreeMap::from([(key.clone(), entry)])),
            Condvar::new(),
        ));
        let (tx, rx) = mpsc::sync_channel(32);
        let request = Request {
            query: serde_json::from_value(
                json!({"action":"usage","roots":roots,"scope":{"allTime":true}}),
            )
            .unwrap(),
            mode: Mode::Auto,
            verify: false,
            wait_ms: None,
            rebuild: false,
            include_history: false,
            also: vec![],
        };
        let selector = selection::ReadViewSelector::new(
            roots.clone(),
            None,
            request.mode.clone(),
            request.verify,
            false,
        )
        .unwrap();
        let cancelled = std::sync::atomic::AtomicBool::new(false);
        // More readers than the queue capacity, with the worker deliberately paused.
        std::thread::scope(|scope| {
            let readers: Vec<_> = (0..40)
                .map(|_| {
                    scope.spawn(|| {
                        select_view(&selector, &shared, &tx, &cancelled)
                            .unwrap()
                            .1
                            .status
                    })
                })
                .collect();
            for reader in readers {
                assert_eq!(reader.join().unwrap(), "syncing");
            }
        });
        assert_eq!(rx.try_recv().unwrap().key, key);
        assert!(rx.try_recv().is_err());
        let work = begin(shared.0.lock().unwrap().get_mut(&key).unwrap(), true).unwrap();
        let previous_touch = shared.0.lock().unwrap()[&key].touched;
        std::thread::scope(|scope| {
            let fresh = scope.spawn(|| {
                let request = Request {
                    mode: Mode::Fresh,
                    ..request.clone()
                };
                let selector = selection::ReadViewSelector::new(
                    request.query.roots.unwrap_or_default(),
                    None,
                    request.mode,
                    request.verify,
                    false,
                )
                .unwrap();
                select_view(&selector, &shared, &tx, &cancelled)
                    .unwrap()
                    .1
                    .status
            });
            // Observe the reader releasing the state lock into its completion wait.
            let deadline = Instant::now() + Duration::from_secs(2);
            loop {
                let entries = shared.0.lock().unwrap();
                if entries[&key].touched != previous_touch {
                    break;
                }
                assert!(Instant::now() < deadline);
                drop(entries);
                std::thread::yield_now();
            }
            finish(shared.0.lock().unwrap().get_mut(&key).unwrap(), work);
            shared.1.notify_all();
            assert_eq!(fresh.join().unwrap(), "current");
        });
        assert!(rx.try_recv().is_err());
    }

    #[test]
    fn rejected_work_does_not_leave_phantom_sync_and_disconnect_is_distinct() {
        let (tx, rx) = mpsc::sync_channel(1);
        let mut first = Entry::new(vec![]);
        let mut second = Entry::new(vec![]);
        request(&mut first, "a", false, &tx).unwrap();
        let error = request(&mut second, "b", false, &tx).unwrap_err();
        assert_eq!(
            error
                .downcast_ref::<crate::dto::OperationError>()
                .unwrap()
                .code,
            "UPDATE_BUSY"
        );
        assert!(!second.syncing);
        assert_eq!(second.requested, 0);
        rx.try_recv().unwrap();
        request(&mut second, "b", false, &tx).unwrap();
        drop(rx);
        let mut third = Entry::new(vec![]);
        let error = request(&mut third, "c", false, &tx).unwrap_err();
        assert_eq!(
            error
                .downcast_ref::<crate::dto::OperationError>()
                .unwrap()
                .code,
            "CORE_UNAVAILABLE"
        );
        assert!(!third.syncing);
    }
}
