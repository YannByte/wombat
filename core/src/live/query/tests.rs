use super::*;
use std::sync::atomic::AtomicBool;

fn state() -> Shared {
    Arc::new((Mutex::new(BTreeMap::new()), Condvar::new()))
}
fn code(error: anyhow::Error) -> &'static str {
    error
        .downcast_ref::<crate::dto::OperationError>()
        .unwrap()
        .code
}
#[test]
fn batched_reports_use_the_selected_revision_even_when_a_newer_view_exists() {
    let source = tempfile::tempdir().unwrap();
    let index = tempfile::tempdir().unwrap();
    fs::create_dir(source.path().join("sessions")).unwrap();
    let path = source.path().join("sessions/a.jsonl");
    let row = |id: &str| {
        json!({"type":"event_msg","timestamp":"2026-10-14T00:00:00Z","payload":{"type":"token_usage_record","response_id":id,"usage":{"input_tokens":100,"output_tokens":10,"total_tokens":110}}}).to_string()+"\n"
    };
    fs::write(
        &path,
        format!(
            "{{\"type\":\"session_meta\",\"payload\":{{\"id\":\"a\"}}}}\n{}",
            row("one")
        ),
    )
    .unwrap();
    let roots = vec![source.path().to_string_lossy().into_owned()];
    let key = source_key(&roots);
    let mut db = crate::live_index::open(&index.path().join("index.sqlite")).unwrap();
    let mut caches = BTreeMap::new();
    let first = sync(&mut db, &key, &roots, false, None, &mut caches)
        .unwrap()
        .unwrap();
    use std::io::Write;
    fs::OpenOptions::new()
        .append(true)
        .open(path)
        .unwrap()
        .write_all(row("two").as_bytes())
        .unwrap();
    let second = sync(&mut db, &key, &roots, false, Some(&first), &mut caches)
        .unwrap()
        .unwrap();
    let mut entry = Entry::new(roots);
    entry.views.push_back((Instant::now(), Arc::clone(&first)));
    entry.views.push_back((Instant::now(), second));
    let state = Arc::new((Mutex::new(BTreeMap::from([(key, entry)])), Condvar::new()));
    let (jobs, receiver) = mpsc::sync_channel(1);
    let request=serde_json::from_value(json!({"query":{"action":"usage","snapshotId":first.manifest.snapshot_ref.snapshot_id,"scope":{"allTime":true},"compact":true},"mode":"cached","also":[{"action":"threads","scope":{"allTime":true},"compact":true},{"action":"usage","scope":{"allTime":true},"presentation":"models","compact":true}]})).unwrap();
    let result = query(
        request,
        &state,
        &jobs,
        &Mutex::new(crate::config::Store::default()),
    )
    .unwrap();
    assert_eq!(result.result.summary.tokens.total, Some(110));
    assert_eq!(result.additional.len(), 2);
    for additional in result.additional {
        assert_eq!(additional.summary.tokens.total, Some(110));
        assert_eq!(
            additional.snapshot_ref.snapshot_id,
            result.result.snapshot_ref.snapshot_id
        );
    }
    assert!(receiver.try_recv().is_err());
}
#[test]
fn usage_business_validation_precedes_view_selection_and_sync() {
    let state = state();
    let (jobs, receiver) = mpsc::sync_channel(1);
    let configs = Mutex::new(crate::config::Store::default());
    for value in [
        json!({"query":{"action":"steps"}}),
        json!({"query":{"action":"usage"},"verify":true}),
        json!({"query":{"action":"refresh"},"mode":"cached"}),
        json!({"query":{"action":"usage","snapshotId":"live::revision"}}),
        json!({"query":{"action":"usage"},"waitMs":0,"mode":"fresh"}),
        json!({"query":{"action":"usage"},"waitMs":600001,"mode":"fresh"}),
        json!({"query":{"action":"usage"},"waitMs":1000}),
        json!({"query":{"action":"usage"},"rebuild":true}),
        json!({"query":{"action":"usage"},"also":[{"action":"refresh"}]}),
        json!({"query":{"action":"usage"},"also":[{"action":"usage","roots":["/outside"]}]}),
        json!({"query":{"action":"usage"},"also":[{"action":"steps"}]}),
    ] {
        let request: Request = serde_json::from_value(value).unwrap();
        assert_eq!(
            code(query(request, &state, &jobs, &configs).unwrap_err()),
            "INVALID_ARGUMENT"
        );
        assert!(state.0.lock().unwrap().is_empty());
        assert!(receiver.try_recv().is_err());
    }
}

#[test]
fn expired_live_entry_resolves_only_exact_config_retained_snapshot() {
    let root = tempfile::tempdir().unwrap();
    let snapshot = Arc::new(
        crate::usage_store::memory(
            Default::default(),
            "live:scope:retained".into(),
            crate::pricing_sync::current_at(root.path()).unwrap(),
            None,
        )
        .unwrap(),
    );
    let view = crate::config::View {
        snapshot: Some(Arc::clone(&snapshot)),
        items: vec![],
        issues: vec![],
        projects: vec![],
        roots: vec![],
        project_roots: vec![],
        revision: "config-revision".into(),
        checked: "2026-10-04T00:00:00Z".into(),
        history_status: "unknown".into(),
        analysis: Default::default(),
        hook_registry: Default::default(),
        config_collection: Default::default(),
        observation_versions: Default::default(),
    };
    let configs = Mutex::new(crate::config::Store::default());
    configs.lock().unwrap().insert(view);
    let state = state();
    let (jobs, receiver) = mpsc::sync_channel(1);
    let cancelled = AtomicBool::new(false);
    let selector = selection::ReadViewSelector::new(
        vec![],
        Some("live:scope:retained".into()),
        Mode::Cached,
        false,
        false,
    )
    .unwrap();
    let (actual, freshness) =
        select_with_retained(&selector, &state, &jobs, &configs, &cancelled).unwrap();
    assert!(Arc::ptr_eq(&actual, &snapshot));
    assert!(state.0.lock().unwrap().is_empty());
    assert_eq!(freshness.status, "fixed");
    assert_eq!(
        freshness.checked_at.as_deref(),
        Some("2026-10-04T00:00:00Z")
    );
    let absent = selection::ReadViewSelector::new(
        vec![],
        Some("live:scope:absent".into()),
        Mode::Auto,
        false,
        false,
    )
    .unwrap();
    assert_eq!(
        code(
            select_with_retained(&absent, &state, &jobs, &configs, &cancelled)
                .err()
                .unwrap()
        ),
        "VIEW_EXPIRED"
    );
    assert!(receiver.try_recv().is_err());
    cancelled.store(true, std::sync::atomic::Ordering::Relaxed);
    assert_eq!(
        code(
            select_with_retained(&selector, &state, &jobs, &configs, &cancelled)
                .err()
                .unwrap()
        ),
        "CANCELLED"
    );
}
