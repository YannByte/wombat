use super::*;
use crate::session_events::Payload as SafePayload;

pub(super) fn replay(collected: &Collected) -> (Collected, SourceReport) {
    let mut facts = Facts::default();
    let mut report = collected.sources[0].clone();
    report.issues.clear();
    let mut observations: Vec<_> = collected
        .events
        .iter()
        .filter_map(|e| {
            let evidence = match e.payload() {
                SafePayload::Ancestry { evidence, .. }
                | SafePayload::Thread { evidence, .. }
                | SafePayload::Turn { evidence, .. } => Some(evidence),
                SafePayload::Measurement { value, .. } => value.evidence.first(),
                SafePayload::Operation { value, .. } => value.evidence.first(),
                _ => None,
            }?;
            Some((
                evidence.file.clone(),
                e.position().byte_offset,
                e.position().ordinal,
                e,
            ))
        })
        .collect();
    observations.sort_by(|a, b| (&a.0, a.1, a.2).cmp(&(&b.0, b.1, b.2)));
    for (_, _, _, event) in observations {
        super::super::event_projection::apply(&mut facts, event, &mut report);
    }
    facts.events = collected
        .events
        .iter()
        .map(|event| (event.shared_id(), event.clone()))
        .collect();
    let mut replayed = Collected::default();
    emit_facts(
        finish_projection(
            facts,
            &mut report,
            &std::sync::atomic::AtomicBool::new(false),
        )
        .unwrap(),
        &mut replayed,
        &std::sync::atomic::AtomicBool::new(false),
    )
    .unwrap();
    (replayed, report)
}

#[test]
fn safe_event_replay_reproduces_late_direct_retraction_and_operation_aliases() {
    let root = tempfile::tempdir().unwrap();
    write(
        root.path(),
        "sessions/a.jsonl",
        &[
            meta("t"),
            context("a", "gpt-5.4", "high"),
            legacy(
                counts(100, 40, 10),
                Some(counts(100, 40, 10)),
                "2026-10-04T01:00:00Z",
            ),
            json!({"type":"response_item","timestamp":"2026-10-04T01:00:01Z","payload":{"type":"function_call","id":"item","call_id":"call","name":"read_file","arguments":"{\"path\":\"/synthetic/SKILL.md\"}"}}),
            json!({"type":"response_item","timestamp":"2026-10-04T01:00:02Z","payload":{"type":"function_call_output","call_id":"call","output":"PRIVATE_BODY"}}),
            direct("t", "a", "response", "2026-10-04T01:00:00Z", 100, 40, 10),
        ],
    );
    let collected = collect(root.path());
    // Delete source files before replay: no prebuilt scope or ledger is supplied.
    fs::remove_dir_all(root.path()).unwrap();
    let (replayed, _) = replay(&collected);
    assert_eq!(
        serde_json::to_value(&replayed.threads).unwrap(),
        serde_json::to_value(&collected.threads).unwrap()
    );
    assert_eq!(
        serde_json::to_value(&replayed.turns).unwrap(),
        serde_json::to_value(&collected.turns).unwrap()
    );
    assert_eq!(replayed.measurements.len(), 1);
    assert_eq!(replayed.measurements[0].tokens.total, Some(110));
    assert_eq!(replayed.operations.len(), 1);
    assert_eq!(replayed.operations[0].kind.as_ref(), "skillRead");
    assert_eq!(replayed.operations[0].status.as_ref(), "unknown");
    assert_eq!(
        serde_json::to_value(&replayed.measurements).unwrap(),
        serde_json::to_value(&collected.measurements).unwrap()
    );
    assert_eq!(
        serde_json::to_value(&replayed.operations).unwrap(),
        serde_json::to_value(&collected.operations).unwrap()
    );
}

#[test]
fn fork_replay_uses_native_candidates_and_explicit_ancestry_without_rebilling() {
    let root = tempfile::tempdir().unwrap();
    let rows = [
        meta("parent"),
        context("a", "gpt-5.4", "high"),
        legacy(
            counts(100, 40, 10),
            Some(counts(100, 40, 10)),
            "2026-10-04T01:00:00Z",
        ),
    ];
    write(root.path(), "sessions/a.jsonl", &rows);
    let mut child = meta("child");
    child["payload"]["forked_from_id"] = json!("parent");
    write(
        root.path(),
        "sessions/b.jsonl",
        &[
            child,
            rows[1].clone(),
            rows[2].clone(),
            legacy(
                counts(120, 40, 12),
                Some(counts(20, 0, 2)),
                "2026-10-04T01:00:01Z",
            ),
        ],
    );
    let collected = collect(root.path());
    fs::remove_dir_all(root.path()).unwrap();
    let (replayed, _) = replay(&collected);
    assert_eq!(
        serde_json::to_value(&replayed.threads).unwrap(),
        serde_json::to_value(&collected.threads).unwrap()
    );
    assert_eq!(
        serde_json::to_value(&replayed.turns).unwrap(),
        serde_json::to_value(&collected.turns).unwrap()
    );
    assert_eq!(
        replayed
            .measurements
            .iter()
            .map(|m| m.tokens.total.unwrap())
            .sum::<u64>(),
        132
    );
    assert_eq!(
        serde_json::to_value(&replayed.measurements).unwrap(),
        serde_json::to_value(&collected.measurements).unwrap()
    );
}

#[test]
fn scope_replay_preserves_unknown_time_status_and_conflicting_projects() {
    let root = tempfile::tempdir().unwrap();
    let mut first = meta("scope");
    first.as_object_mut().unwrap().remove("timestamp");
    let mut second = context("a", "gpt-5.4", "high");
    second["payload"]["cwd"] = json!("/synthetic/other");
    second["timestamp"] = json!("2026-10-04T01:00:00Z");
    write(
        root.path(),
        "sessions/scope.jsonl",
        &[
            first,
            second,
            json!({"type":"event_msg","timestamp":"2026-10-04T01:00:03Z","payload":{"type":"task_complete","turn_id":"a"}}),
            json!({"type":"event_msg","timestamp":"2026-10-04T01:00:02Z","payload":{"type":"task_started","turn_id":"a"}}),
            json!({"type":"turn_context","payload":{"turn_id":"unknown"}}),
        ],
    );
    let collected = collect(root.path());
    fs::remove_dir_all(root.path()).unwrap();
    let (replayed, report) = replay(&collected);
    assert_eq!(replayed.threads.len(), 1);
    assert!(replayed.threads[0].project.is_none());
    assert!(report.issues.iter().any(|i| i.code == "projectConflict"));
    let completed = replayed
        .turns
        .iter()
        .find(|t| t.upstream_id == "a")
        .unwrap();
    assert_eq!(completed.status, "completed");
    assert_eq!(
        completed.started_at.as_deref(),
        Some("2026-10-04T01:00:00.000000000Z")
    );
    assert_eq!(
        completed.ended_at.as_deref(),
        Some("2026-10-04T01:00:03.000000000Z")
    );
    let unknown = replayed
        .turns
        .iter()
        .find(|t| t.upstream_id == "unknown")
        .unwrap();
    assert!(unknown.started_at.is_none());
    assert!(unknown.ended_at.is_none());
    assert_eq!(unknown.status, "unknown");
    assert_eq!(
        serde_json::to_value(&replayed.threads).unwrap(),
        serde_json::to_value(&collected.threads).unwrap()
    );
    assert_eq!(
        serde_json::to_value(&replayed.turns).unwrap(),
        serde_json::to_value(&collected.turns).unwrap()
    );
}
