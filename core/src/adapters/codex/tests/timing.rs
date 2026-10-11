use super::*;
use crate::session_events::{Gap, Payload as SafePayload, Phase, Precision};

fn event(kind: &str, turn: &str) -> Value {
    json!({"type":"event_msg","timestamp":"2026-10-04T01:00:00.123456Z","payload":{"type":kind,"turn_id":turn}})
}

#[test]
fn lifecycle_retains_native_zero_precision_context_changes_and_safe_metadata() {
    let root = tempfile::tempdir().unwrap();
    let mut start = event("task_started", "a");
    start["payload"]["model_context_window"] = json!(1000);
    let mut end = event("task_complete", "a");
    end["payload"]["duration_ms"] = json!(0);
    end["payload"]["time_to_first_token_ms"] = json!(0);
    end["payload"]["last_agent_message"] = json!("PRIVATE_SYNTHETIC_BODY");
    let mut window = legacy(counts(10, 0, 1), None, "2026-10-04T01:00:00Z");
    window["payload"]["info"]["model_context_window"] = json!(2000);
    write(
        root.path(),
        "sessions/timing.jsonl",
        &[meta("t"), start, window, end],
    );
    let mut facts = collect(root.path());
    facts.events.retain(|e| {
        !matches!(
            e.payload(),
            SafePayload::Thread { .. } | SafePayload::Turn { .. }
        )
    });
    assert_eq!(facts.events.len(), 5);
    assert_eq!(facts.measurements.len(), 1);
    let values = facts
        .events
        .iter()
        .filter_map(|e| match e.payload() {
            SafePayload::ContextWindow { tokens, .. } => Some(*tokens),
            _ => None,
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(values, BTreeSet::from([1000, 2000]));
    let completed = facts
        .events
        .iter()
        .find(|e| {
            matches!(
                e.payload(),
                SafePayload::Lifecycle {
                    phase: Phase::Completed,
                    ..
                }
            )
        })
        .unwrap();
    assert!(matches!(
        completed.payload(),
        SafePayload::Lifecycle {
            duration_ms: Some(0),
            first_token_ms: Some(0),
            ..
        }
    ));
    assert_eq!(completed.time().precision, Precision::Microsecond);
    assert_eq!(completed.turn_id(), Some(facts.turns[0].id.as_str()));
    assert!(
        !serde_json::to_string(&facts.events)
            .unwrap()
            .contains("PRIVATE_SYNTHETIC_BODY")
    );
}

#[test]
fn malformed_native_timing_preserves_lifecycle_and_reports_unknowns() {
    let root = tempfile::tempdir().unwrap();
    let mut end = event("task_complete", "a");
    end["timestamp"] = json!("not-time");
    end["payload"]["duration_ms"] = json!(-1);
    end["payload"]["time_to_first_token_ms"] = json!("bad");
    write(root.path(), "sessions/timing.jsonl", &[meta("t"), end]);
    let mut facts = collect(root.path());
    facts.events.retain(|e| {
        !matches!(
            e.payload(),
            SafePayload::Thread { .. } | SafePayload::Turn { .. }
        )
    });
    assert_eq!(facts.events.len(), 1);
    let observation = &facts.events[0];
    assert!(matches!(
        observation.payload(),
        SafePayload::Lifecycle {
            duration_ms: None,
            first_token_ms: None,
            ..
        }
    ));
    assert!(observation.gaps().contains(&Gap::InvalidTimestamp));
    assert!(observation.gaps().contains(&Gap::InvalidNativeField));
    assert!(facts.issues.iter().any(|i| i.code == "invalidTimingField"));
    assert!(!facts.issues.iter().any(|i| i.code == "invalidRecord"));
}

#[test]
fn event_projection_delta_keeps_occurrences_and_replaces_membership_only_on_replay() {
    use std::io::Write;
    let root = tempfile::tempdir().unwrap();
    let index = tempfile::tempdir().unwrap();
    let initial = [meta("t"), event("task_started", "a")];
    let path = write(root.path(), "sessions/events.jsonl", &initial);
    let prefix_bytes = fs::metadata(&path).unwrap().len();
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
    assert_eq!(first.collected.events.len(), 5);
    writeln!(
        fs::OpenOptions::new().append(true).open(&path).unwrap(),
        "{}",
        event("task_complete", "a")
    )
    .unwrap();
    let next = incremental::sync_cached(&db, &source, false, &mut cache)
        .unwrap()
        .unwrap();
    let delta: Vec<_> = next
        .collected
        .events
        .iter()
        .filter(|e| e.position().byte_offset >= prefix_bytes)
        .collect();
    assert_eq!(delta.len(), 4);
    assert!(
        delta
            .iter()
            .all(|e| e.position().byte_offset >= prefix_bytes)
    );
    assert_eq!(next.collected.events.len(), 9);
    for original in &first.collected.events {
        assert_eq!(
            next.collected
                .events
                .iter()
                .find(|e| e.id() == original.id())
                .unwrap()
                .as_ref(),
            original.as_ref()
        );
    }
    assert!(
        incremental::sync_cached(&db, &source, false, &mut cache)
            .unwrap()
            .is_none()
    );
    let verified = incremental::sync_cached(&db, &source, true, &mut cache)
        .unwrap()
        .unwrap();
    assert_eq!(verified.collected.events, next.collected.events);
    writeln!(
        fs::OpenOptions::new().append(true).open(&path).unwrap(),
        "{}",
        event("task_started", "b")
    )
    .unwrap();
    let restarted =
        incremental::sync_cached(&db, &source, false, &mut incremental::Cache::default())
            .unwrap()
            .unwrap();
    // Source disappearance keeps observations; it is an empty delta, not a deletion.
    // Restart has refreshed parser state, so use another fresh cache to establish it.
    let mut cache = incremental::Cache::default();
    incremental::sync_cached(&db, &source, true, &mut cache)
        .unwrap()
        .unwrap();
    fs::remove_file(&path).unwrap();
    let missing = incremental::sync_cached(&db, &source, false, &mut cache)
        .unwrap()
        .unwrap();
    assert_eq!(missing.collected.events, restarted.collected.events);
    write(root.path(), "sessions/events.jsonl", &initial);
    let replacement = incremental::sync_cached(&db, &source, false, &mut cache)
        .unwrap()
        .unwrap();
    assert_eq!(replacement.collected.events.len(), 5);
    assert!(
        replacement.collected.events.iter().all(|e| !first
            .collected
            .events
            .iter()
            .any(|old| old.id() == e.id()))
    );
}

#[test]
fn append_restart_verify_truncation_and_replacement_obey_event_generations() {
    use std::io::Write;
    let root = tempfile::tempdir().unwrap();
    let index = tempfile::tempdir().unwrap();
    let rows = [meta("t"), event("task_started", "a")];
    let path = write(root.path(), "sessions/timing.jsonl", &rows);
    let source = CodexAdapter
        .discover(&DiscoveryRequest {
            roots: vec![root.path().into()],
        })
        .sources
        .remove(0);
    let sync = |verify| {
        let mut db = crate::live_index::open(&index.path().join("index.sqlite")).unwrap();
        let tx = db.transaction().unwrap();
        let result = incremental::sync(&tx, &source, verify).unwrap();
        tx.commit().unwrap();
        result.unwrap()
    };
    let first = sync(false).events;
    let mut file = fs::OpenOptions::new().append(true).open(&path).unwrap();
    file.write_all((event("task_complete", "a").to_string() + "\n").as_bytes())
        .unwrap();
    drop(file);
    let appended = sync(false).events;
    assert_eq!(first.len(), 5);
    assert_eq!(appended.len(), 9);
    assert!(
        first
            .iter()
            .all(|original| appended.iter().any(|e| e.id() == original.id()))
    );
    assert!(appended.iter().any(|e| e.id() == first[0].id()));
    assert!(
        appended
            .iter()
            .all(|e| e.position().generation == first[0].position().generation)
    );
    let verified = sync(true).events;
    assert_eq!(
        serde_json::to_value(&appended).unwrap(),
        serde_json::to_value(&verified).unwrap()
    );
    // Same session prefix after truncation must not reuse the earlier generation.
    write(root.path(), "sessions/timing.jsonl", &rows);
    let truncated = sync(false).events;
    assert_eq!(truncated.len(), first.len());
    assert_ne!(truncated[0].id(), first[0].id());
    let replacement = write(root.path(), "replacement.jsonl", &rows);
    fs::rename(replacement, &path).unwrap();
    let replaced = sync(false).events;
    assert_ne!(replaced[0].id(), truncated[0].id());
    assert_eq!(sync(true).events[0].id(), replaced[0].id());
    fs::remove_file(path).unwrap();
    let missing = sync(false);
    assert_eq!(missing.events[0].id(), replaced[0].id());
    assert_eq!(missing.threads.len(), 1);
    assert!(missing.issues.iter().any(|i| i.code == "sourceMissing"));
}

#[test]
fn unidentified_start_does_not_inherit_the_previous_turn() {
    let root = tempfile::tempdir().unwrap();
    let mut unknown = event("task_started", "new");
    unknown["payload"]
        .as_object_mut()
        .unwrap()
        .remove("turn_id");
    write(
        root.path(),
        "sessions/timing.jsonl",
        &[meta("t"), context("old", "gpt-5.4", "high"), unknown],
    );
    let mut facts = collect(root.path());
    facts.events.retain(|e| {
        !matches!(
            e.payload(),
            SafePayload::Thread { .. } | SafePayload::Turn { .. }
        )
    });
    assert_eq!(facts.events.len(), 1);
    assert!(facts.events[0].thread_id().is_some());
    assert!(facts.events[0].turn_id().is_none());
    assert!(facts.events[0].gaps().contains(&Gap::MissingIdentity));
}

#[test]
fn partial_tail_and_rollback_never_publish_half_an_event_generation() {
    use std::io::Write;
    let root = tempfile::tempdir().unwrap();
    let index = tempfile::tempdir().unwrap();
    let path = write(
        root.path(),
        "sessions/timing.jsonl",
        &[meta("t"), event("task_started", "a")],
    );
    let source = CodexAdapter
        .discover(&DiscoveryRequest {
            roots: vec![root.path().into()],
        })
        .sources
        .remove(0);
    let mut db = crate::live_index::open(&index.path().join("index.sqlite")).unwrap();
    let tx = db.transaction().unwrap();
    let first = incremental::sync(&tx, &source, false).unwrap().unwrap();
    tx.commit().unwrap();
    let row = event("task_complete", "a").to_string();
    let mut file = fs::OpenOptions::new().append(true).open(&path).unwrap();
    file.write_all(row.as_bytes()).unwrap();
    let tx = db.transaction().unwrap();
    let partial = incremental::sync(&tx, &source, false).unwrap().unwrap();
    assert_eq!(
        serde_json::to_value(&partial.events).unwrap(),
        serde_json::to_value(&first.events).unwrap()
    );
    assert_eq!(partial.events[0].id(), first.events[0].id());
    assert!(partial.issues.iter().any(|i| i.code == "incompleteTail"));
    tx.commit().unwrap();
    file.write_all(b"\n").unwrap();
    let tx = db.transaction().unwrap();
    let uncommitted = incremental::sync(&tx, &source, false).unwrap().unwrap();
    assert_eq!(uncommitted.events.len(), first.events.len() + 4);
    tx.rollback().unwrap();
    let tx = db.transaction().unwrap();
    let retried = incremental::sync(&tx, &source, false).unwrap().unwrap();
    // Rolled-back new observations are collected again. Only committed events
    // retain their collection times; event identity and source semantics match.
    let committed: std::collections::BTreeSet<_> = first.events.iter().map(|e| e.id()).collect();
    let comparable = |events: &[std::sync::Arc<crate::session_events::Event>]| {
        let mut rows = serde_json::to_value(events).unwrap();
        for row in rows.as_array_mut().unwrap() {
            if !committed.contains(row["id"].as_str().unwrap()) {
                row.as_object_mut().unwrap().remove("collectedAt");
            }
        }
        rows
    };
    assert_eq!(comparable(&retried.events), comparable(&uncommitted.events));
    assert_eq!(retried.sources[0].bytes_read, (row.len() + 1) as u64);
    tx.commit().unwrap();
    let tx = db.transaction().unwrap();
    assert!(incremental::sync(&tx, &source, false).unwrap().is_none());
}
