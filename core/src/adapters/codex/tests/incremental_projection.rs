use super::*;

#[test]
fn canonical_selection_preserves_raw_candidates_and_reuses_unchanged_allocations() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        "sessions/a.jsonl",
        &[
            meta("t"),
            context("u", "model", "low"),
            direct("t", "u", "r", "2026-09-29T00:00:01Z", 100, 60, 10),
            json!({"type":"response_item","payload":{"type":"function_call","name":"read_file","call_id":"call","arguments":"{}"}}),
        ],
    );
    let collected = collect(dir.path());
    let mut facts = Facts::default();
    let mut events = collected.events.clone();
    events.sort_by_key(|event| {
        (
            event.position().file_id.clone(),
            event.position().byte_offset,
            event.position().ordinal,
        )
    });
    for event in &events {
        super::super::event_projection::apply(&mut facts, event, &mut collected.sources[0].clone());
    }
    facts.events = events
        .into_iter()
        .map(|event| (event.shared_id(), event))
        .collect();
    let mut report = collected.sources[0].clone();
    let cancelled = std::sync::atomic::AtomicBool::new(false);
    facts.resolve_operations(&mut report, &cancelled).unwrap();
    let before = serde_json::to_value(&facts).unwrap();
    let projected = projection::build(&mut facts, &mut report, &cancelled).unwrap();
    assert_eq!(serde_json::to_value(&facts).unwrap(), before);
    assert_eq!(projected.measurements.len(), 1);
    assert_eq!(projected.operations.len(), 1);
    assert!(Arc::ptr_eq(
        &projected.measurements[0],
        &facts.measurements[&projected.measurements[0].id].measurement
    ));
    assert!(Arc::ptr_eq(
        &projected.operations[0],
        &facts.operations[&projected.operations[0].id]
    ));
    assert!(projected.metadata.measurements.is_empty());
    assert!(projected.metadata.operations.is_empty());
}

#[test]
fn indexed_direct_coverage_preserves_owner_and_half_open_boundaries() {
    let dir = tempfile::tempdir().unwrap();
    write(
        dir.path(),
        "sessions/base.jsonl",
        &[
            meta("t"),
            context("u", "gpt-5.4", "high"),
            direct("t", "u", "base", "2026-09-29T00:00:01Z", 100, 60, 10),
        ],
    );
    let collected = collect(dir.path());
    let base = &collected.measurements[0];
    // Independent coverage expectations: adjacent ranges form a union; endpoint
    // contact has no overlap; another owner never contributes coverage.
    for (ranges, legacy_range, other_owner, removed, partial) in [
        (vec![(0, 50), (50, 100)], (0, 100), false, true, false),
        (vec![(0, 49), (50, 100)], (0, 100), false, true, true),
        (vec![(100, 110)], (0, 100), false, false, false),
        (vec![(0, 20)], (20, 40), false, false, false),
        (vec![(10, 50)], (20, 40), false, true, false),
        (vec![(0, 100)], (0, 100), true, false, false),
    ] {
        let mut facts = Facts::default();
        let mut report = collected.sources[0].clone();
        report.issues.clear();
        for (i, (start, end)) in ranges.iter().enumerate() {
            let mut m = base.as_ref().clone();
            m.id = format!("direct-{i}");
            if other_owner {
                m.thread_id = Some("other-owner".into());
            }
            facts.measurements.insert(
                m.id.clone(),
                Candidate {
                    measurement: m.into(),
                    direct: true,
                    cumulative: Some(*end),
                    interval_start: Some(*start),
                    fingerprint: String::new(),
                },
            );
        }
        let mut legacy = base.as_ref().clone();
        legacy.id = "legacy".into();
        facts.measurements.insert(
            legacy.id.clone(),
            Candidate {
                measurement: legacy.into(),
                direct: false,
                cumulative: Some(legacy_range.1),
                interval_start: Some(legacy_range.0),
                fingerprint: String::new(),
            },
        );
        let excluded = facts
            .direct_exclusions(
                &BTreeSet::new(),
                &mut report,
                &std::sync::atomic::AtomicBool::new(false),
            )
            .unwrap();
        assert_eq!(excluded.contains("legacy"), removed);
        assert_eq!(
            report.issues.iter().any(|i| i.code == "usageOverlap"),
            partial
        );
    }
}

#[test]
fn live_cached_operations_keep_old_views_and_restart_equivalence() {
    use std::io::Write;
    let root = tempfile::tempdir().unwrap();
    let index = tempfile::tempdir().unwrap();
    let path = write(
        root.path(),
        "sessions/live.jsonl",
        &[
            meta("t"),
            context("u", "gpt-5.4", "high"),
            direct("t", "u", "r", "2026-09-29T00:00:01Z", 100, 60, 10),
            json!({"type":"response_item","payload":{"type":"function_call","name":"read_file","call_id":"call","arguments":"PRIVATE_ARGUMENT"}}),
        ],
    );
    let source = CodexAdapter
        .discover(&DiscoveryRequest {
            roots: vec![root.path().into()],
        })
        .sources
        .remove(0);
    let db = crate::live_index::open(&index.path().join("index.sqlite")).unwrap();
    let mut cache = incremental::Cache::default();
    let synced = incremental::sync_cached(&db, &source, false, &mut cache)
        .unwrap()
        .unwrap();
    let first = synced.collected;
    let old_status = first.operations[0].status.clone();
    let mut file = fs::OpenOptions::new().append(true).open(path).unwrap();
    writeln!(
        file,
        "{}",
        direct("t", "u", "r2", "2026-09-29T00:00:02Z", 100, 60, 10)
    )
    .unwrap();
    let synced = incremental::sync_cached(&db, &source, false, &mut cache)
        .unwrap()
        .unwrap();
    let second = synced.collected;
    assert_eq!(first.operations[0], second.operations[0]);
    writeln!(file,"{}",json!({"type":"response_item","payload":{"type":"function_call_output","call_id":"call","output":{"isError":true,"content":"PRIVATE_OUTPUT"}}})).unwrap();
    let synced = incremental::sync_cached(&db, &source, false, &mut cache)
        .unwrap()
        .unwrap();
    let third = synced.collected;
    assert_eq!(third.operations[0].status.as_ref(), "failed");
    assert_eq!(first.operations[0].status, old_status);
    writeln!(
        file,
        "{}",
        direct("t", "u", "r3", "2026-09-29T00:00:03Z", 100, 60, 10)
    )
    .unwrap();
    let mut restored = incremental::Cache::default();
    let restart = incremental::sync_cached(&db, &source, false, &mut restored)
        .unwrap()
        .unwrap()
        .collected;
    assert_eq!(third.operations[0], restart.operations[0]);
    let previous = third
        .measurements
        .iter()
        .find(|r| r.response_id.as_deref() == Some("r"))
        .unwrap();
    let resumed = restart
        .measurements
        .iter()
        .find(|r| r.id == previous.id)
        .unwrap();
    assert_eq!(previous, resumed);
    let full = collect(root.path());
    assert_eq!(
        serde_json::to_value(&restart.operations).unwrap(),
        serde_json::to_value(&full.operations).unwrap()
    );
    assert_eq!(
        serde_json::to_value(&restart.measurements).unwrap(),
        serde_json::to_value(&full.measurements).unwrap()
    );
    assert_eq!(
        restart
            .measurements
            .iter()
            .map(|m| m.tokens.total.unwrap())
            .sum::<u64>(),
        330
    );
}

#[test]
fn live_projection_delta_retracts_late_legacy_and_preserves_prior_facts() {
    use std::io::Write;
    let root = tempfile::tempdir().unwrap();
    let index = tempfile::tempdir().unwrap();
    let path = write(
        root.path(),
        "sessions/delta.jsonl",
        &[
            meta("t"),
            context("u", "gpt-5.4", "high"),
            legacy(
                counts(100, 60, 10),
                Some(counts(100, 60, 10)),
                "2026-09-29T00:00:01Z",
            ),
        ],
    );
    let source = CodexAdapter
        .discover(&DiscoveryRequest {
            roots: vec![root.path().into()],
        })
        .sources
        .remove(0);
    let db = crate::live_index::open(&index.path().join("index.sqlite")).unwrap();
    let mut cache = incremental::Cache::default();
    let first = incremental::sync_cached(&db, &source, false, &mut cache)
        .unwrap()
        .unwrap();
    let old = first.collected.measurements[0].clone();
    let mut file = fs::OpenOptions::new().append(true).open(path).unwrap();
    writeln!(
        file,
        "{}",
        direct("t", "u", "r", "2026-09-29T00:00:01Z", 100, 60, 10)
    )
    .unwrap();
    let second = incremental::sync_cached(&db, &source, false, &mut cache)
        .unwrap()
        .unwrap();
    assert_ne!(second.collected.measurements[0].id, old.id);
    let stored = crate::live_index::load_map(&db, &format!("projection:{}", source.id)).unwrap();
    assert!(stored["measurements"].get(&old.id).is_none());
    assert_eq!(second.collected.measurements.len(), 1);
    let retained = second.collected.measurements[0].clone();
    writeln!(
        file,
        "{}",
        direct("t", "u", "r", "2026-09-29T00:00:01Z", 200, 60, 10)
    )
    .unwrap();
    let third = incremental::sync_cached(&db, &source, false, &mut cache)
        .unwrap()
        .unwrap();
    assert_eq!(third.collected.measurements.len(), 1);
    assert_eq!(
        third.collected.measurements[0].tokens.total, None,
        "conflicting evidence remains unknown"
    );
    assert_eq!(retained.tokens.total, Some(110));
    assert_eq!(old.tokens.total, Some(110));
}

#[test]
fn canonical_and_alternate_operation_identities_survive_restart_and_turn_reuse() {
    use std::io::Write;
    let root = tempfile::tempdir().unwrap();
    let index = tempfile::tempdir().unwrap();
    let path = write(
        root.path(),
        "sessions/aliases.jsonl",
        &[
            meta("t"),
            context("u", "gpt-5.4", "low"),
            direct("t", "u", "r", "2026-09-29T00:00:01Z", 100, 60, 10),
            json!({"type":"response_item","payload":{"type":"function_call","call_id":"native-call","name":"mcp__docs__search","arguments":"{}"}}),
            json!({"type":"event_msg","payload":{"type":"item_completed","item":{"type":"McpToolCall","id":"native-item","call_id":"native-call","server":"docs","tool":"search","status":"completed"}}}),
        ],
    );
    let source = CodexAdapter
        .discover(&DiscoveryRequest {
            roots: vec![root.path().into()],
        })
        .sources
        .remove(0);
    let db = crate::live_index::open(&index.path().join("index.sqlite")).unwrap();
    let mut cache = incremental::Cache::default();
    let before = incremental::sync_cached(&db, &source, false, &mut cache)
        .unwrap()
        .unwrap()
        .collected;
    assert_eq!(before.operations.len(), 1);
    assert_eq!(before.operations[0].status.as_ref(), "completed");
    let mut file = fs::OpenOptions::new().append(true).open(path).unwrap();
    writeln!(file, "{}", json!({"type":"response_item","payload":{"type":"function_call_output","item_id":"native-item","output":{"isError":true}}})).unwrap();
    let mut restored = incremental::Cache::default();
    let changed = incremental::sync_cached(&db, &source, false, &mut restored)
        .unwrap()
        .unwrap()
        .collected;
    assert_eq!(changed.operations.len(), 1);
    assert_eq!(changed.operations[0].status.as_ref(), "failed");
    assert_eq!(changed.operations[0].id, before.operations[0].id);
    assert_eq!(before.operations[0].status.as_ref(), "completed");
    writeln!(file, "{}", context("other-turn", "gpt-5.4", "low")).unwrap();
    writeln!(file, "{}", json!({"type":"response_item","payload":{"type":"function_call","call_id":"native-call","name":"read_file","arguments":"{}"}})).unwrap();
    let after = incremental::sync_cached(&db, &source, false, &mut restored)
        .unwrap()
        .unwrap()
        .collected;
    assert_eq!(after.operations.len(), 2);
    assert_eq!(after.measurements.len(), 1);
    assert_eq!(
        serde_json::to_value(&after.operations).unwrap(),
        serde_json::to_value(&collect(root.path()).operations).unwrap()
    );
}

#[test]
fn cancelled_accounting_checks_direct_fast_path_and_fork_reduction_without_mutation() {
    use std::sync::atomic::AtomicBool;
    let root = tempfile::tempdir().unwrap();
    write(
        root.path(),
        "sessions/base.jsonl",
        &[
            meta("t"),
            context("u", "model", "low"),
            direct("t", "u", "r", "2026-09-29T00:00:01Z", 100, 60, 10),
        ],
    );
    let collected = collect(root.path());
    let parents = HashMap::new();
    let forest = session_relations::SessionRelations::new(&parents);
    for direct in [false, true] {
        for thread in [None, collected.measurements[0].thread_id.clone()] {
            let mut row = collected.measurements[0].as_ref().clone();
            row.thread_id = thread;
            let mut facts = Facts::default();
            facts.measurements.insert(
                row.id.clone(),
                Candidate {
                    measurement: row.into(),
                    direct,
                    cumulative: Some(100),
                    interval_start: Some(0),
                    fingerprint: "synthetic-counter".into(),
                },
            );
            let before = serde_json::to_value(&facts.measurements).unwrap();
            let mut report = collected.sources[0].clone();
            report.issues.clear();
            let cancelled = AtomicBool::new(true);
            let error = facts
                .direct_exclusions(&BTreeSet::new(), &mut report, &cancelled)
                .unwrap_err();
            assert_eq!(
                error
                    .downcast_ref::<crate::dto::OperationError>()
                    .unwrap()
                    .code,
                "CANCELLED"
            );
            assert_eq!(serde_json::to_value(&facts.measurements).unwrap(), before);
            assert!(report.issues.is_empty());
            let error = facts
                .inherited_measurements(&forest, &cancelled)
                .unwrap_err();
            assert_eq!(
                error
                    .downcast_ref::<crate::dto::OperationError>()
                    .unwrap()
                    .code,
                "CANCELLED"
            );
            assert_eq!(serde_json::to_value(&facts.measurements).unwrap(), before);
        }
    }
}
