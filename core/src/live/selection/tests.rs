use super::*;

#[test]
fn cached_history_interest_does_not_suppress_a_later_auto_backfill() {
    let root = tempfile::tempdir().unwrap();
    let roots = vec![root.path().to_string_lossy().into_owned()];
    let key = source_key(&roots);
    let mut view = fixture(&key, "recent");
    Arc::get_mut(&mut view)
        .unwrap()
        .manifest
        .issues
        .push(Issue {
            code: "historyDeferred".into(),
            message: "Deferred synthetic history".into(),
            source_instance_id: None,
            evidence: None,
        });
    let state = shared(&key, roots.clone(), view);
    {
        let mut entries = state.0.lock().unwrap();
        let entry = entries.get_mut(&key).unwrap();
        entry.requested = 1;
        entry.completed = 1;
        entry.last_sync = Instant::now();
    }
    let (jobs, receiver) = mpsc::sync_channel(2);
    let cached = ReadViewSelector::new(roots.clone(), None, Mode::Cached, false, false).unwrap();
    select_view(&cached, &state, &jobs, &AtomicBool::new(false)).unwrap();
    assert!(receiver.try_recv().is_err());
    let automatic = ReadViewSelector::new(roots, None, Mode::Auto, false, false).unwrap();
    let (_, freshness) = select_view(&automatic, &state, &jobs, &AtomicBool::new(false)).unwrap();
    assert_eq!(freshness.status, "syncing");
    assert!(!receiver.try_recv().unwrap().restore_only);
    assert!(
        state.0.lock().unwrap()[&key]
            .pending
            .unwrap()
            .include_history
    );
}

#[test]
fn concurrent_cached_readers_share_one_restore_job_and_published_view() {
    let root = tempfile::tempdir().unwrap();
    let roots = vec![root.path().to_string_lossy().into_owned()];
    let key = source_key(&roots);
    let state: Shared = Arc::new((Mutex::new(BTreeMap::new()), Condvar::new()));
    let (jobs, receiver) = mpsc::sync_channel(4);
    std::thread::scope(|scope| {
        let read = || {
            let request =
                ReadViewSelector::new(roots.clone(), None, Mode::Cached, false, false).unwrap();
            select_view(&request, &state, &jobs, &AtomicBool::new(false))
                .unwrap()
                .0
        };
        let first = scope.spawn(read);
        let job = receiver.recv_timeout(Duration::from_secs(1)).unwrap();
        assert!(job.restore_only);
        let second = scope.spawn(read);
        let snapshot = fixture(&key, "restored");
        {
            let mut entries = state.0.lock().unwrap();
            let entry = entries.get_mut(&key).unwrap();
            assert!(entry.restoring);
            entry
                .views
                .push_back((Instant::now(), Arc::clone(&snapshot)));
            entry.restoring = false;
        }
        state.1.notify_all();
        assert!(Arc::ptr_eq(&first.join().unwrap(), &snapshot));
        assert!(Arc::ptr_eq(&second.join().unwrap(), &snapshot));
        assert!(receiver.try_recv().is_err());
    });
}

#[test]
fn cached_restore_failure_retains_its_storage_error() {
    let root = tempfile::tempdir().unwrap();
    let roots = vec![root.path().to_string_lossy().into_owned()];
    let key = source_key(&roots);
    let state: Shared = Arc::new((Mutex::new(BTreeMap::new()), Condvar::new()));
    let (jobs, receiver) = mpsc::sync_channel(1);
    let request = ReadViewSelector::new(roots, None, Mode::Cached, false, false).unwrap();
    std::thread::scope(|scope| {
        let reader = scope.spawn(|| select_view(&request, &state, &jobs, &AtomicBool::new(false)));
        assert!(
            receiver
                .recv_timeout(Duration::from_secs(1))
                .unwrap()
                .restore_only
        );
        {
            let mut entries = state.0.lock().unwrap();
            let entry = entries.get_mut(&key).unwrap();
            entry.restoring = false;
            entry.error = Some("unsupported index format".into());
            entry.error_code = Some("INDEX_VERSION_UNSUPPORTED");
        }
        state.1.notify_all();
        let error = reader.join().unwrap().err().unwrap();
        assert_eq!(code(error), "INDEX_VERSION_UNSUPPORTED");
    });
}

fn fixture(key: &str, revision: &str) -> Arc<Snapshot> {
    let root = tempfile::tempdir().unwrap();
    Arc::new(
        crate::usage_store::memory(
            Default::default(),
            format!("live:{key}:{revision}"),
            crate::pricing_sync::current_at(root.path()).unwrap(),
            None,
        )
        .unwrap(),
    )
}
fn shared(key: &str, roots: Vec<String>, snapshot: Arc<Snapshot>) -> Shared {
    let mut entry = Entry::new(roots);
    entry.views.push_back((Instant::now(), snapshot));
    Arc::new((
        Mutex::new(BTreeMap::from([(key.into(), entry)])),
        Condvar::new(),
    ))
}
fn code(error: anyhow::Error) -> &'static str {
    error
        .downcast_ref::<crate::dto::OperationError>()
        .unwrap()
        .code
}

#[test]
fn auto_reads_reuse_committed_views_until_five_minutes_and_manual_refresh_bypasses_delay() {
    let root = tempfile::tempdir().unwrap();
    let roots = vec![root.path().to_string_lossy().into_owned()];
    let key = source_key(&roots);
    let state = shared(&key, roots.clone(), fixture(&key, "ready"));
    {
        let mut entries = state.0.lock().unwrap();
        let entry = entries.get_mut(&key).unwrap();
        entry.requested = 1;
        entry.completed = 1;
        entry.last_sync = Instant::now() - Duration::from_secs(299);
        assert!(!scheduling::auto_due(entry));
    }
    let (jobs, receiver) = mpsc::sync_channel(1);
    let cancelled = AtomicBool::new(false);
    let request = ReadViewSelector::new(roots.clone(), None, Mode::Auto, false, false).unwrap();
    for _ in 0..5 {
        let (_, freshness) = select_view(&request, &state, &jobs, &cancelled).unwrap();
        assert_eq!(freshness.status, "current");
        assert!(receiver.try_recv().is_err());
    }
    for (elapsed, mode) in [(1, Mode::Fresh), (301, Mode::Auto)] {
        state.0.lock().unwrap().get_mut(&key).unwrap().last_sync =
            Instant::now() - Duration::from_secs(elapsed);
        let request = ReadViewSelector::new(roots.clone(), None, mode, false, false).unwrap();
        std::thread::scope(|scope| {
            let reader = scope.spawn(|| select_view(&request, &state, &jobs, &cancelled).unwrap());
            let job = receiver.recv_timeout(Duration::from_secs(2)).unwrap();
            assert_eq!(job.key, key);
            let mut entries = state.0.lock().unwrap();
            let entry = entries.get_mut(&key).unwrap();
            let work = scheduling::begin(entry, true).unwrap();
            scheduling::finish(entry, work);
            entry.last_sync = Instant::now();
            drop(entries);
            state.1.notify_all();
            let status = reader.join().unwrap().1.status;
            assert!(status == "current" || request.mode == Mode::Auto && status == "syncing");
        });
    }
}

#[test]
fn selection_policy_is_business_independent_and_keeps_wait_limits() {
    let root = tempfile::tempdir().unwrap();
    let roots = vec![root.path().to_string_lossy().into_owned()];
    for (mode, refresh, expected) in [
        (Mode::Auto, false, WaitPolicy::Auto),
        (Mode::Auto, true, WaitPolicy::Fresh),
        (Mode::Fresh, false, WaitPolicy::Fresh),
        (Mode::Cached, false, WaitPolicy::None),
    ] {
        let selected = ReadViewSelector::new(roots.clone(), None, mode, false, refresh).unwrap();
        assert_eq!(selected.wait, expected);
    }
    assert_eq!(WaitPolicy::Auto.timeout(), Duration::from_secs(2));
    assert_eq!(WaitPolicy::Fresh.timeout(), Duration::from_secs(10));
    assert_eq!(WaitPolicy::None.timeout(), Duration::ZERO);
    assert_eq!(
        code(
            ReadViewSelector::new(vec![], None, Mode::Cached, true, false)
                .err()
                .unwrap()
        ),
        "INVALID_ARGUMENT"
    );
    assert_eq!(
        code(
            ReadViewSelector::new(vec![], None, Mode::Cached, false, true)
                .err()
                .unwrap()
        ),
        "INVALID_ARGUMENT"
    );
}

#[test]
fn fixed_selector_binds_explicit_roots_and_never_queues_or_advances() {
    let root = tempfile::tempdir().unwrap();
    let other = tempfile::tempdir().unwrap();
    let roots = vec![root.path().to_string_lossy().into_owned()];
    let key = source_key(&roots);
    let old = fixture(&key, "old");
    let state = shared(&key, roots.clone(), Arc::clone(&old));
    state
        .0
        .lock()
        .unwrap()
        .get_mut(&key)
        .unwrap()
        .views
        .push_back((Instant::now(), fixture(&key, "latest")));
    let (jobs, receiver) = mpsc::sync_channel(1);
    let cancelled = AtomicBool::new(false);
    for mode in [Mode::Auto, Mode::Fresh, Mode::Cached] {
        for selected_roots in [roots.clone(), vec![]] {
            let request = ReadViewSelector::new(
                selected_roots,
                Some(old.manifest.snapshot_ref.snapshot_id.clone()),
                mode.clone(),
                false,
                false,
            )
            .unwrap();
            assert_eq!(request.wait, WaitPolicy::None);
            let (actual, freshness) = select_view(&request, &state, &jobs, &cancelled).unwrap();
            assert!(Arc::ptr_eq(&actual, &old));
            assert_eq!(freshness.status, "fixed");
            assert!(receiver.try_recv().is_err());
        }
    }
    let mismatch = ReadViewSelector::new(
        vec![other.path().to_string_lossy().into_owned()],
        Some(old.manifest.snapshot_ref.snapshot_id.clone()),
        Mode::Cached,
        false,
        false,
    )
    .err()
    .unwrap();
    assert_eq!(code(mismatch), "INVALID_ARGUMENT");
    for identity in ["saved", "live::id", "live:scope:", "live:scope:id:extra"] {
        assert_eq!(
            code(
                ReadViewSelector::new(vec![], Some(identity.into()), Mode::Cached, false, false)
                    .err()
                    .unwrap()
            ),
            "INVALID_ARGUMENT"
        );
    }
    let expired = ReadViewSelector::new(
        vec![],
        Some(format!("live:{key}:expired")),
        Mode::Auto,
        false,
        false,
    )
    .unwrap();
    assert_eq!(
        code(
            select_view(&expired, &state, &jobs, &cancelled)
                .err()
                .unwrap()
        ),
        "VIEW_EXPIRED"
    );
    assert!(receiver.try_recv().is_err());
}

#[test]
fn cached_ready_selection_preserves_view_without_scan_work() {
    let root = tempfile::tempdir().unwrap();
    let roots = vec![root.path().to_string_lossy().into_owned()];
    let key = source_key(&roots);
    let snapshot = fixture(&key, "cached");
    let state = shared(&key, roots.clone(), Arc::clone(&snapshot));
    let (jobs, receiver) = mpsc::sync_channel(1);
    let cancelled = AtomicBool::new(false);
    let request = ReadViewSelector::new(roots, None, Mode::Cached, false, false).unwrap();
    let (actual, _) = select_view(&request, &state, &jobs, &cancelled).unwrap();
    assert!(Arc::ptr_eq(&actual, &snapshot));
    assert!(receiver.try_recv().is_err());
    assert_eq!(state.0.lock().unwrap()[&key].requested, 0);
}

#[test]
fn cancelling_reader_preserves_shared_pending_work_and_other_readers() {
    let root = tempfile::tempdir().unwrap();
    let roots = vec![root.path().to_string_lossy().into_owned()];
    let key = source_key(&roots);
    let snapshot = fixture(&key, "committed");
    let state = shared(&key, roots.clone(), snapshot);
    let (jobs, receiver) = mpsc::sync_channel(1);
    let cancelled = AtomicBool::new(false);
    let request = ReadViewSelector::new(roots, None, Mode::Fresh, false, false).unwrap();
    std::thread::scope(|scope| {
        let reader = scope.spawn(|| select_view(&request, &state, &jobs, &cancelled));
        let job = receiver.recv_timeout(Duration::from_secs(2)).unwrap();
        assert_eq!(job.key, key);
        cancelled.store(true, Ordering::Relaxed);
        state.1.notify_all();
        assert_eq!(code(reader.join().unwrap().err().unwrap()), "CANCELLED");
    });
    let mut entries = state.0.lock().unwrap();
    assert!(entries[&key].pending.is_some());
    assert!(entries[&key].syncing);
    let work = scheduling::begin(entries.get_mut(&key).unwrap(), true).unwrap();
    scheduling::finish(entries.get_mut(&key).unwrap(), work);
    drop(entries);
    cancelled.store(false, Ordering::Relaxed);
    let fixed = ReadViewSelector::new(
        vec![],
        Some(format!("live:{key}:committed")),
        Mode::Cached,
        false,
        false,
    )
    .unwrap();
    assert_eq!(
        select_view(&fixed, &state, &jobs, &cancelled)
            .unwrap()
            .1
            .status,
        "fixed"
    );
}

#[test]
fn unknown_fixed_read_cannot_seed_default_roots_for_later_live_sync() {
    let root = tempfile::tempdir().unwrap();
    let roots = vec![root.path().to_string_lossy().into_owned()];
    let key = source_key(&roots);
    let state: Shared = Arc::new((Mutex::new(BTreeMap::new()), Condvar::new()));
    let (jobs, receiver) = mpsc::sync_channel(1);
    let cancelled = AtomicBool::new(false);
    let fixed = ReadViewSelector::new(
        vec![],
        Some(format!("live:{key}:unknown")),
        Mode::Cached,
        false,
        false,
    )
    .unwrap();
    assert_eq!(
        code(
            select_view(&fixed, &state, &jobs, &cancelled)
                .err()
                .unwrap()
        ),
        "VIEW_EXPIRED"
    );
    assert!(state.0.lock().unwrap().is_empty());
    assert!(receiver.try_recv().is_err());
    let live = ReadViewSelector::new(roots.clone(), None, Mode::Fresh, false, false).unwrap();
    std::thread::scope(|scope| {
        let reader = scope.spawn(|| select_view(&live, &state, &jobs, &cancelled));
        assert_eq!(
            receiver.recv_timeout(Duration::from_secs(2)).unwrap().key,
            key
        );
        assert_eq!(state.0.lock().unwrap()[&key].roots, roots);
        cancelled.store(true, Ordering::Relaxed);
        state.1.notify_all();
        assert_eq!(code(reader.join().unwrap().err().unwrap()), "CANCELLED");
    });
}
