//! Independent source fixtures for canonical association across collection modes.
use super::*;
use std::io::Write;

fn request(call: Option<&str>, item: Option<&str>) -> Value {
    json!({"type":"response_item","payload":{"type":"function_call","call_id":call,"id":item,"name":"read_file","arguments":"{\"path\":\"/synthetic/SKILL.md\"}"}})
}
fn source(root: &Path) -> SourceInstance {
    CodexAdapter
        .discover(&DiscoveryRequest {
            roots: vec![root.into()],
        })
        .sources
        .remove(0)
}

#[test]
fn late_bridge_retracts_old_rows_preserves_ids_and_agrees_after_restart() {
    let root = tempfile::tempdir().unwrap();
    let index = tempfile::tempdir().unwrap();
    let path = write(
        root.path(),
        "sessions/a.jsonl",
        &[
            meta("t"),
            context("u", "model", "low"),
            request(Some("call"), None),
            request(None, Some("item")),
        ],
    );
    let source = source(root.path());
    let db = crate::live_index::open(&index.path().join("index.sqlite")).unwrap();
    let mut cache = incremental::Cache::default();
    let first = incremental::sync_cached(&db, &source, false, &mut cache)
        .unwrap()
        .unwrap();
    assert_eq!(first.collected.operations.len(), 2);
    let canonical = first
        .collected
        .operations
        .iter()
        .find(|op| op.call_id.as_deref() == Some("call"))
        .unwrap()
        .id
        .clone();
    let retired = first
        .collected
        .operations
        .iter()
        .find(|op| op.item_id.as_deref() == Some("item"))
        .unwrap()
        .id
        .clone();
    let original_events: BTreeSet<_> = first
        .collected
        .events
        .iter()
        .map(|e| e.id().to_owned())
        .collect();
    let projection_scope = "independent-projection";
    crate::live_index::put_packed(
        &db,
        projection_scope,
        "operations",
        first
            .collected
            .operations
            .iter()
            .map(|row| (row.id.as_str(), row)),
    )
    .unwrap();
    writeln!(
        fs::OpenOptions::new().append(true).open(&path).unwrap(),
        "{}",
        request(Some("call"), Some("item"))
    )
    .unwrap();
    let tx = db.unchecked_transaction().unwrap();
    let second = incremental::sync_cached(&tx, &source, false, &mut cache)
        .unwrap()
        .unwrap();
    assert!(
        second.operations.is_none(),
        "retirement requires full source replacement"
    );
    assert_eq!(second.collected.operations.len(), 1);
    assert_eq!(second.collected.operations[0].id, canonical);
    assert_eq!(second.collected.operations[0].evidence.len(), 3);
    assert_eq!(
        first.collected.operations.len(),
        2,
        "prior view remains immutable"
    );
    assert!(
        original_events.is_subset(
            &second
                .collected
                .events
                .iter()
                .map(|e| e.id().to_owned())
                .collect()
        )
    );
    crate::live_index::put_packed(
        &tx,
        projection_scope,
        "operations",
        second
            .collected
            .operations
            .iter()
            .map(|row| (row.id.as_str(), row)),
    )
    .unwrap();
    crate::live_index::retain_packed(
        &tx,
        projection_scope,
        "operations",
        second.collected.operations.iter().map(|r| r.id.as_str()),
    )
    .unwrap();
    tx.commit().unwrap();
    let parser_scope = format!("parser:{}:{}:1:facts", source.id, VERSION);
    let parser = crate::live_index::load_map(&db, &parser_scope).unwrap();
    assert!(
        !parser.contains_key("operations"),
        "canonical facts have one persistent owner"
    );
    assert!(
        !parser.contains_key("aliases"),
        "aliases are reconstructed from observations"
    );
    let projected = crate::live_index::load_map(&db, projection_scope).unwrap();
    assert_eq!(projected["operations"].as_object().unwrap().len(), 1);
    assert_eq!(projected["operations"][&canonical]["id"], canonical);
    assert!(projected["operations"].get(&retired).is_none());
    // Restart receives a new observation; existing source events are replayed.
    writeln!(fs::OpenOptions::new().append(true).open(&path).unwrap(),"{}",json!({"type":"response_item","payload":{"type":"function_call_output","call_id":"call","output":{"exit_code":0}}})).unwrap();
    let restarted =
        incremental::sync_cached(&db, &source, false, &mut incremental::Cache::default())
            .unwrap()
            .unwrap();
    assert_eq!(restarted.collected.operations.len(), 1);
    assert_eq!(restarted.collected.operations[0].id, canonical);
    assert_eq!(
        restarted.collected.operations[0].status.as_ref(),
        "completed"
    );
    assert_eq!(
        restarted.collected.operations,
        collect(root.path()).operations
    );
}

#[test]
fn same_call_and_item_text_without_bridge_remain_independent_including_fork_replay() {
    let root = tempfile::tempdir().unwrap();
    write(
        root.path(),
        "sessions/a.jsonl",
        &[
            meta("parent"),
            context("u", "model", "low"),
            request(Some("same"), None),
        ],
    );
    let mut child = meta("child");
    child["payload"]["forked_from_id"] = json!("parent");
    write(
        root.path(),
        "sessions/b.jsonl",
        &[
            child,
            context("u", "model", "low"),
            request(None, Some("same")),
        ],
    );
    let collected = collect(root.path());
    assert_eq!(collected.operations.len(), 2);
    assert_ne!(
        collected.operations[0].thread_id,
        collected.operations[1].thread_id
    );
    let (replayed, _) = super::event_projection::replay(&collected);
    assert_eq!(replayed.operations, collected.operations);
}

#[test]
fn matching_explicit_fork_alias_keeps_parent_id_and_merges_terminal_evidence() {
    let root = tempfile::tempdir().unwrap();
    write(
        root.path(),
        "sessions/a.jsonl",
        &[
            meta("parent"),
            context("u", "model", "low"),
            request(Some("call"), Some("item")),
        ],
    );
    let parent_only = collect(root.path());
    let parent_id = parent_only.operations[0].id.clone();
    let mut child = meta("child");
    child["payload"]["forked_from_id"] = json!("parent");
    write(
        root.path(),
        "sessions/b.jsonl",
        &[
            child,
            context("u", "model", "low"),
            request(Some("call"), Some("item")),
            json!({"type":"response_item","payload":{"type":"function_call_output","call_id":"call","output":{"exit_code":1}}}),
        ],
    );
    let collected = collect(root.path());
    assert_eq!(collected.operations.len(), 1);
    assert_eq!(collected.operations[0].id, parent_id);
    assert_eq!(collected.operations[0].status.as_ref(), "failed");
    assert_eq!(collected.operations[0].evidence.len(), 3);
    let (replayed, _) = super::event_projection::replay(&collected);
    assert_eq!(replayed.operations, collected.operations);
}

#[test]
fn missing_turn_is_a_separate_known_scope_and_does_not_erase_existing_use() {
    let root = tempfile::tempdir().unwrap();
    write(
        root.path(),
        "sessions/a.jsonl",
        &[
            meta("t"),
            request(Some("call"), None),
            context("u", "model", "low"),
            request(Some("call"), None),
        ],
    );
    let collected = collect(root.path());
    assert_eq!(collected.operations.len(), 2);
    assert_eq!(
        collected
            .operations
            .iter()
            .filter(|op| op.turn_id.is_none())
            .count(),
        1
    );
    assert_eq!(
        collected
            .operations
            .iter()
            .filter(|op| op.turn_id.is_some())
            .count(),
        1
    );
    let (replayed, _) = super::event_projection::replay(&collected);
    assert_eq!(replayed.operations, collected.operations);
}

#[test]
fn explicit_dual_alias_fork_replay_uses_every_alias_and_keeps_parent_canonical_id() {
    for (parent_call, parent_item, child_call, child_item) in [
        (None, Some("item"), Some("call"), Some("item")),
        (Some("call"), Some("item"), None, Some("item")),
    ] {
        let root = tempfile::tempdir().unwrap();
        write(
            root.path(),
            "sessions/a.jsonl",
            &[
                meta("parent"),
                context("u", "model", "low"),
                request(parent_call, parent_item),
            ],
        );
        let parent_id = collect(root.path()).operations[0].id.clone();
        let mut child = meta("child");
        child["payload"]["forked_from_id"] = json!("parent");
        write(
            root.path(),
            "sessions/b.jsonl",
            &[
                child,
                context("u", "model", "low"),
                request(child_call, child_item),
            ],
        );
        let collected = collect(root.path());
        assert_eq!(collected.operations.len(), 1);
        assert_eq!(collected.operations[0].id, parent_id);
        assert_eq!(collected.operations[0].evidence.len(), 2);
        let (replayed, _) = super::event_projection::replay(&collected);
        assert_eq!(replayed.operations, collected.operations);
    }
}

#[test]
fn dual_alias_fork_bridge_unifies_recorded_ancestors_without_collapsing_siblings() {
    let root = tempfile::tempdir().unwrap();
    write(
        root.path(),
        "sessions/a.jsonl",
        &[
            meta("grandparent"),
            context("u", "model", "low"),
            request(Some("call"), None),
        ],
    );
    let oldest = collect(root.path()).operations[0].id.clone();
    let mut parent = meta("parent");
    parent["payload"]["forked_from_id"] = json!("grandparent");
    write(
        root.path(),
        "sessions/b.jsonl",
        &[
            parent,
            context("u", "model", "low"),
            request(None, Some("item")),
        ],
    );
    assert_eq!(collect(root.path()).operations.len(), 2);
    let mut child = meta("child");
    child["payload"]["forked_from_id"] = json!("parent");
    write(
        root.path(),
        "sessions/c.jsonl",
        &[
            child,
            context("u", "model", "low"),
            request(Some("call"), Some("item")),
        ],
    );
    let collected = collect(root.path());
    assert_eq!(collected.operations.len(), 1);
    assert_eq!(collected.operations[0].id, oldest);
    assert_eq!(collected.operations[0].evidence.len(), 3);
    let (replayed, _) = super::event_projection::replay(&collected);
    assert_eq!(replayed.operations, collected.operations);

    let siblings = tempfile::tempdir().unwrap();
    write(
        siblings.path(),
        "sessions/a.jsonl",
        &[meta("parent"), context("u", "model", "low")],
    );
    for name in ["one", "two"] {
        let mut child = meta(name);
        child["payload"]["forked_from_id"] = json!("parent");
        write(
            siblings.path(),
            &format!("sessions/{name}.jsonl"),
            &[
                child,
                context("u", "model", "low"),
                request(Some("call"), Some("item")),
            ],
        );
    }
    assert_eq!(collect(siblings.path()).operations.len(), 2);
}

#[test]
fn cancellation_after_source_read_emits_no_fresh_facts() {
    let root = tempfile::tempdir().unwrap();
    let path = write(
        root.path(),
        "sessions/a.jsonl",
        &[
            meta("t"),
            context("u", "model", "low"),
            request(Some("call"), None),
        ],
    );
    let source = source(root.path());
    let context = RunContext::default();
    let mut report = SourceReport {
        source: source.clone(),
        adapter_version: VERSION.into(),
        source_versions: vec![],
        capabilities: CodexAdapter.descriptor().capabilities,
        status: "complete".into(),
        files_read: 0,
        bytes_read: 0,
        issues: vec![],
    };
    let mut facts = Facts::default();
    read_file(&path, &source, &context, &mut facts, &mut report);
    assert!(!facts.events.is_empty());
    assert!(facts.operations_pending);
    context
        .cancelled
        .store(true, std::sync::atomic::Ordering::Relaxed);
    let mut sink = Collected::default();
    finish_facts(
        facts,
        root.path(),
        &mut report,
        &mut sink,
        &context.cancelled,
    );
    assert_eq!(report.status, "cancelled");
    assert!(sink.threads.is_empty());
    assert!(sink.turns.is_empty());
    assert!(sink.operations.is_empty());
    assert!(sink.measurements.is_empty());
    assert!(sink.events.is_empty());
    assert!(sink.watermarks.is_empty());
}

#[test]
fn cancelled_fork_operation_reduction_preserves_existing_canonical_rows() {
    let root = tempfile::tempdir().unwrap();
    write(
        root.path(),
        "sessions/a.jsonl",
        &[
            meta("parent"),
            context("u", "model", "low"),
            request(Some("call"), Some("item")),
        ],
    );
    let mut child = meta("child");
    child["payload"]["forked_from_id"] = json!("parent");
    write(
        root.path(),
        "sessions/b.jsonl",
        &[
            child,
            context("u", "model", "low"),
            request(Some("call"), Some("item")),
        ],
    );
    let collected = collect(root.path());
    let mut facts = Facts::default();
    let mut report = collected.sources[0].clone();
    for event in &collected.events {
        super::super::event_projection::apply(&mut facts, event, &mut report);
    }
    facts.events = collected
        .events
        .iter()
        .map(|e| (e.id().to_owned(), e.clone()))
        .collect();
    facts
        .resolve_operations(&mut report, &std::sync::atomic::AtomicBool::new(false))
        .unwrap();
    let before = facts.operations.clone();
    assert_eq!(before.len(), 2);
    let parents = facts.parents.clone();
    let forest = super::super::session_relations::SessionRelations::new(&parents);
    let error = facts
        .remove_inherited_operations(
            &forest,
            &mut report,
            &std::sync::atomic::AtomicBool::new(true),
        )
        .unwrap_err();
    assert_eq!(
        error
            .downcast_ref::<crate::dto::OperationError>()
            .unwrap()
            .code,
        "CANCELLED"
    );
    assert_eq!(facts.operations, before);
}

#[test]
fn cancellation_during_fact_emission_stops_before_the_next_fact() {
    struct CancelSink<'a> {
        flag: &'a std::sync::atomic::AtomicBool,
        count: usize,
    }
    impl FactSink for CancelSink<'_> {
        fn push(&mut self, _fact: Fact) {
            self.count += 1;
            self.flag.store(true, std::sync::atomic::Ordering::Relaxed);
        }
    }
    let root = tempfile::tempdir().unwrap();
    let path = write(
        root.path(),
        "sessions/a.jsonl",
        &[
            meta("t"),
            context("u", "model", "low"),
            request(Some("call"), None),
        ],
    );
    let source = source(root.path());
    let context = RunContext::default();
    let mut report = SourceReport {
        source: source.clone(),
        adapter_version: VERSION.into(),
        source_versions: vec![],
        capabilities: CodexAdapter.descriptor().capabilities,
        status: "complete".into(),
        files_read: 0,
        bytes_read: 0,
        issues: vec![],
    };
    let mut facts = Facts::default();
    read_file(&path, &source, &context, &mut facts, &mut report);
    let mut sink = CancelSink {
        flag: &context.cancelled,
        count: 0,
    };
    finish_facts(
        facts,
        root.path(),
        &mut report,
        &mut sink,
        &context.cancelled,
    );
    assert_eq!(sink.count, 1);
    assert_eq!(report.status, "cancelled");
}
