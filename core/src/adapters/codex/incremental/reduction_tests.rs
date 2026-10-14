use super::*;
use crate::session_events::{Event, Payload, Phase, Position, Time};

fn operation(scope: &str, offset: u64, native: &str, id: &str, status: &str) -> Arc<Event> {
    let value: Operation = serde_json::from_value(serde_json::json!({
        "id":id,"threadId":scope,"turnId":scope,"callId":native,
        "kind":"command","name":"synthetic","sequence":offset,"timestamp":null,
        "timePrecision":"unknown","status":status,"outcomeConflict":false,
        "evidence":[{"file":"/synthetic/source.jsonl","line":1}]
    }))
    .unwrap();
    Arc::new(
        Event::new_at(
            Position {
                source_instance_id: "source".into(),
                file_id: scope.into(),
                generation: "g".into(),
                byte_offset: offset,
                ordinal: 0,
            },
            Some(scope.into()),
            Some(scope.into()),
            Time::from_source(None).0,
            vec![],
            Payload::Operation {
                value: Arc::new(value),
                phase: if status == "failed" {
                    Phase::Failed
                } else {
                    Phase::Started
                },
            },
            "2026-10-05T00:00:00Z".into(),
        )
        .unwrap(),
    )
}
fn report() -> SourceReport {
    SourceReport {
        source: SourceInstance {
            id: "source".into(),
            agent_kind: "codex".into(),
            root: "/synthetic".into(),
        },
        adapter_version: VERSION.into(),
        source_versions: vec![],
        capabilities: CodexAdapter.descriptor().capabilities,
        status: "complete".into(),
        files_read: 0,
        bytes_read: 0,
        issues: vec![],
    }
}

#[test]
fn archive_scopes_preserve_global_collisions_late_outcomes_and_unaffected_allocations() {
    let directory = tempfile::tempdir().unwrap();
    let db = crate::live_index::open(&directory.path().join("index.sqlite")).unwrap();
    let cancelled = std::sync::atomic::AtomicBool::new(false);
    let mut rows = vec![
        operation("z", 1, "first", "duplicate", "running"),
        operation("a", 2, "second", "duplicate", "running"),
        operation("z", 3, "third", "duplicate", "running"),
    ];
    for n in 0..1000 {
        rows.push(operation(
            "untouched",
            100 + n,
            &format!("call-{n}"),
            &format!("id-{n}"),
            "running",
        ));
    }
    crate::live_index::begin_event_changes(&db).unwrap();
    crate::live_index::save_events(&db, "facts", &rows, true, &cancelled).unwrap();
    let report = report();
    let expected = replay_events(
        rows.iter()
            .map(|e| (e.shared_id(), Arc::clone(e)))
            .collect(),
        &report,
        &cancelled,
    )
    .unwrap();
    let mut fresh = Facts {
        operations_pending: true,
        ..Facts::default()
    };
    reduction::resolve_streamed(&db, "facts", &mut fresh, &mut report.clone(), &cancelled).unwrap();
    assert_eq!(
        fresh.operations, expected.operations,
        "Encoded write inputs preserve complete canonical outcomes"
    );
    assert_eq!(fresh.operation_aliases, expected.operation_aliases);
    let mut actual = reduction::load_streamed(&db, "facts", &report, &cancelled).unwrap();
    assert_eq!(actual.operations, expected.operations);
    assert_eq!(actual.operation_aliases, expected.operation_aliases);
    assert_eq!(
        actual.operations["duplicate"].call_id.as_deref(),
        Some("first")
    );
    assert!(actual.events.is_empty());
    assert!(actual.associations_share_canonical_allocations());
    assert_eq!(
        db.query_row("SELECT count(*) FROM codex_replay", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        0
    );
    let untouched = Arc::clone(&actual.operations["id-0"]);
    let completion = operation("a", 50, "second", "new-observation", "failed");
    crate::live_index::begin_event_changes(&db).unwrap();
    crate::live_index::save_events(&db, "facts", [&completion], false, &cancelled).unwrap();
    let before = crate::live_index::decoded_event_count();
    actual.operations_pending = true;
    reduction::resolve_streamed(&db, "facts", &mut actual, &mut report.clone(), &cancelled)
        .unwrap();
    assert_eq!(
        crate::live_index::decoded_event_count() - before,
        2,
        "Only the changed target is expanded"
    );
    assert!(Arc::ptr_eq(&untouched, &actual.operations["id-0"]));
    assert!(actual.associations_share_canonical_allocations());
    rows.push(completion);
    let expected = replay_events(
        rows.iter()
            .map(|e| (e.shared_id(), Arc::clone(e)))
            .collect(),
        &report,
        &cancelled,
    )
    .unwrap();
    assert_eq!(actual.operations, expected.operations);
    assert_eq!(actual.operation_aliases, expected.operation_aliases);
    assert!(
        actual
            .operations
            .values()
            .any(|op| op.call_id.as_deref() == Some("second") && op.status.as_ref() == "failed")
    );
    assert!(untouched.status.as_ref() == "running");
}

#[test]
fn incomplete_pass_local_association_inputs_fail_without_changing_committed_observations() {
    let directory = tempfile::tempdir().unwrap();
    let db = crate::live_index::open(&directory.path().join("index.sqlite")).unwrap();
    let cancelled = std::sync::atomic::AtomicBool::new(false);
    let event = operation("a", 1, "call", "id", "running");
    crate::live_index::begin_event_changes(&db).unwrap();
    crate::live_index::save_events(&db, "facts", [&event], true, &cancelled).unwrap();
    {
        let tx = db.unchecked_transaction().unwrap();
        tx.execute("UPDATE written_association_blocks SET payload='[]'", [])
            .unwrap();
        let mut facts = Facts {
            operations_pending: true,
            ..Facts::default()
        };
        let error =
            reduction::resolve_streamed(&tx, "facts", &mut facts, &mut report(), &cancelled)
                .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("written association count mismatch")
        );
    }
    let mut facts = Facts {
        operations_pending: true,
        ..Facts::default()
    };
    reduction::resolve_streamed(&db, "facts", &mut facts, &mut report(), &cancelled).unwrap();
    assert_eq!(facts.operations.len(), 1);
    assert_eq!(facts.operations["id"].status.as_ref(), "running");
}

#[test]
fn ordered_activity_replay_omits_detail_and_preserves_turn_creation_order() {
    let mut facts = Facts::default();
    let turn = turn_identity("thread", "turn");
    let mut value: Operation = serde_json::from_value(serde_json::json!({
        "id":"call","threadId":"thread","turnId":turn,"kind":"command",
        "name":"synthetic","sequence":1,"timestamp":"2026-10-05T00:00:02Z",
        "timePrecision":"record","status":"running","outcomeConflict":false,
        "path":"/synthetic/".repeat(1024),"evidence":[]
    }))
    .unwrap();
    value.turn_id = Some(turn.clone().into());
    let event = Event::new_at(
        Position {
            source_instance_id: "source".into(),
            file_id: "file".into(),
            generation: "generation".into(),
            byte_offset: 1,
            ordinal: 0,
        },
        Some("thread".into()),
        Some(turn.clone()),
        Time::from_source(None).0,
        vec![],
        Payload::Operation {
            value: Arc::new(value),
            phase: Phase::Started,
        },
        "2026-10-05T00:00:00Z".into(),
    )
    .unwrap();
    let raw = serde_json::to_string(&event).unwrap();
    let input =
        serde_json::to_string(&event_projection::Replay::from_event(&event).unwrap()).unwrap();
    assert!(input.len() * 20 < raw.len());
    assert!(!input.contains("path"));
    let apply = |facts: &mut Facts| {
        let record: event_projection::Replay<'_> = serde_json::from_str(&input).unwrap();
        record.apply(facts, &mut report());
    };
    apply(&mut facts);
    assert!(facts.turns.is_empty(), "Activity cannot create a turn");
    facts.project_turn(
        "thread",
        "turn",
        Some("2026-10-05T00:00:01Z"),
        Some("running"),
    );
    assert_eq!(
        facts.turns[&turn].last_activity_at.as_deref(),
        Some("2026-10-05T00:00:01Z")
    );
    apply(&mut facts);
    assert_eq!(
        facts.turns[&turn].last_activity_at.as_deref(),
        Some("2026-10-05T00:00:02Z")
    );
}
