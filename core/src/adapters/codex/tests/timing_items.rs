use super::*;
use crate::session_events::{ActivityKind, Gap, ItemKind, Payload as SafePayload, Phase};

fn item(phase: &str, kind: &str, id: &str) -> Value {
    json!({"type":"event_msg","timestamp":"2026-10-04T01:00:00Z","payload":{"type":phase,"turn_id":"a","item":{"type":kind,"id":id}}})
}

#[test]
fn diagnostic_requests_and_results_keep_only_source_locators_without_storing_bodies() {
    let root = tempfile::tempdir().unwrap();
    let mut failed = item("item_completed", "CommandExecution", "failed");
    failed["payload"]["item"]["status"] = json!("failed");
    failed["payload"]["item"]["exit_code"] = json!(1);
    failed["payload"]["item"]["aggregated_output"] = json!("PRIVATE_SYNTHETIC_TOOL_ERROR");
    let mut success = item("item_completed", "CommandExecution", "success");
    success["payload"]["item"]["aggregated_output"] = json!("PRIVATE_SYNTHETIC_TOOL_OK");
    write(
        root.path(),
        "sessions/tool.jsonl",
        &[
            meta("t"),
            context("a", "gpt-5.4", "high"),
            failed,
            success,
            json!({"type":"response_item","timestamp":"2026-10-04T01:00:01Z","payload":{"type":"function_call","status":"completed","call_id":"opaque","name":"synthetic_tool","arguments":"{}"}}),
            json!({"type":"response_item","timestamp":"2026-10-04T01:00:02Z","payload":{"type":"function_call_output","call_id":"opaque","output":"PRIVATE_SYNTHETIC_TOOL_RESULT_WITHOUT_OUTCOME"}}),
        ],
    );
    let facts = collect(root.path());
    assert!(
        !serde_json::to_string(&facts.events)
            .unwrap()
            .contains("PRIVATE_SYNTHETIC_TOOL")
    );
    let located = facts
        .events
        .iter()
        .filter(|event| event.original_content().is_some())
        .collect::<Vec<_>>();
    assert_eq!(located.len(), 4);
    for status in ["failed", "unknown", "running", "completed"] {
        let event = located.iter().find(|event| matches!(event.payload(), SafePayload::Operation {value,..} if value.status.as_ref() == status)).unwrap();
        assert!(!event.original_content().unwrap().chunks.is_empty());
    }
    let opaque = facts
        .operations
        .iter()
        .find(|operation| operation.call_id.as_deref() == Some("opaque"))
        .unwrap();
    assert_eq!(opaque.status.as_ref(), "unknown");
    assert!(opaque.text_result.is_some());
    assert!(!opaque.outcome_conflict);
}

#[test]
fn generated_request_completion_never_replaces_execution_dispatch_or_conflicts_with_result() {
    use std::sync::atomic::AtomicBool;
    for kind in ["function_call", "custom_tool_call"] {
        let root = tempfile::tempdir().unwrap();
        let output_kind = if kind == "function_call" {
            "function_call_output"
        } else {
            "custom_tool_call_output"
        };
        write(
            root.path(),
            "sessions/request.jsonl",
            &[
                meta("t"),
                context("u", "gpt-5.4", "high"),
                json!({"type":"response_item","timestamp":"2026-10-04T01:00:00Z","payload":{"type":kind,"status":"completed","call_id":"c","name":"synthetic_tool","arguments":"{}","input":"synthetic"}}),
                json!({"type":"response_item","timestamp":"2026-10-04T01:00:02Z","payload":{"type":output_kind,"call_id":"c","output":{"exit_code":1}}}),
            ],
        );
        let facts = collect(root.path());
        let phases = facts
            .events
            .iter()
            .filter_map(|event| match event.payload() {
                SafePayload::Operation { value, phase } => Some((value, phase)),
                _ => None,
            })
            .collect::<Vec<_>>();
        assert!(
            phases
                .iter()
                .any(|(op, phase)| op.status.as_ref() == "running" && **phase == Phase::Started)
        );
        assert!(
            phases
                .iter()
                .any(|(op, phase)| op.status.as_ref() == "failed" && **phase == Phase::Failed)
        );
        assert_eq!(facts.operations.len(), 1);
        assert_eq!(facts.operations[0].status.as_ref(), "failed");
        assert!(!facts.operations[0].outcome_conflict);
        let cancel = AtomicBool::new(false);
        let resolution = crate::operation_association::resolve(
            facts.events.iter().map(std::sync::Arc::as_ref),
            &cancel,
        )
        .unwrap();
        let endpoints =
            crate::operation_association::endpoints::reduce(&resolution.phases, &cancel).unwrap();
        assert_eq!(endpoints.groups.len(), 1);
        assert!(!endpoints.groups[0].time_conflict);
        assert!(endpoints.groups[0].start_ms < endpoints.groups[0].end_ms);
    }
}

#[test]
fn operation_events_keep_both_observations_while_the_projection_merges_one_call() {
    let root = tempfile::tempdir().unwrap();
    let mut start = item("item_started", "CommandExecution", "cmd");
    start["payload"]["started_at_ms"] = json!(1000);
    start["payload"]["item"]["command"] = json!(["PRIVATE_SYNTHETIC_COMMAND"]);
    let mut end = item("item_completed", "CommandExecution", "cmd");
    end["payload"]["started_at_ms"] = json!(1000);
    end["payload"]["completed_at_ms"] = json!(4000);
    end["payload"]["item"]["duration"] = json!({"secs":2,"nanos":500000123});
    end["payload"]["item"]["aggregated_output"] = json!("PRIVATE_SYNTHETIC_OUTPUT");
    end["payload"]["item"]["exit_code"] = json!(1);
    write(
        root.path(),
        "sessions/items.jsonl",
        &[meta("t"), start, end],
    );
    let facts = collect(root.path());
    assert_eq!(facts.operations.len(), 1);
    assert_eq!(facts.operations[0].duration_ms, Some(2500));
    let observations: Vec<_> = facts
        .events
        .iter()
        .filter_map(|e| match e.payload() {
            SafePayload::Operation { value, phase } => Some((value, phase)),
            _ => None,
        })
        .collect();
    assert_eq!(observations.len(), 2);
    assert_eq!(observations[0].0.id, observations[1].0.id);
    assert!(observations.iter().any(|(_, p)| **p == Phase::Started));
    assert!(observations.iter().any(|(_, p)| **p == Phase::Failed));
    let completed = facts
        .events
        .iter()
        .find(|e| {
            matches!(
                e.payload(),
                SafePayload::Item {
                    phase: Phase::Completed,
                    ..
                }
            )
        })
        .unwrap();
    assert!(matches!(
        completed.payload(),
        SafePayload::Item {
            item_kind: ItemKind::Command,
            started_at_ms: Some(1000),
            completed_at_ms: Some(4000),
            ..
        }
    ));
    assert!(
        matches!(completed.payload(), SafePayload::Item { duration: Some(d), .. } if d.secs == 2 && d.nanos == 500000123)
    );
    assert_eq!(
        facts
            .events
            .iter()
            .map(|e| e.id())
            .collect::<BTreeSet<_>>()
            .len(),
        facts.events.len()
    );
    let serialized = serde_json::to_string(&facts.events).unwrap();
    assert!(!serialized.contains("PRIVATE_SYNTHETIC"));
    assert_ne!(
        completed.time().timestamp.as_deref(),
        Some("1970-01-01T00:00:04.000000000Z")
    );
}

#[test]
fn compaction_lifecycle_and_message_activity_do_not_retain_bodies_or_invent_endpoints() {
    let root = tempfile::tempdir().unwrap();
    let mut reasoning = item("item_completed", "Reasoning", "r");
    reasoning["payload"]["item"]["content"] = json!(["PRIVATE_REASONING"]);
    let mut assistant = json!({"type":"response_item","payload":{"type":"message","role":"assistant","content":[{"text":"PRIVATE_MESSAGE"}]}});
    assistant["timestamp"] = json!("2026-10-04T01:00:01Z");
    write(
        root.path(),
        "sessions/items.jsonl",
        &[
            meta("t"),
            context("a", "gpt-5.4", "high"),
            item("item_started", "ContextCompaction", "c"),
            item("item_completed", "ContextCompaction", "c"),
            reasoning,
            assistant,
            json!({"type":"compacted","payload":{"message":"PRIVATE_COMPACTION_BODY"}}),
        ],
    );
    let facts = collect(root.path());
    let compactions: Vec<_> = facts
        .events
        .iter()
        .filter(|e| {
            matches!(
                e.payload(),
                SafePayload::Item {
                    item_kind: ItemKind::Compaction,
                    ..
                }
            )
        })
        .collect();
    assert_eq!(compactions.len(), 2);
    assert!(compactions.iter().all(|e| matches!(
        e.payload(),
        SafePayload::Item {
            started_at_ms: None,
            completed_at_ms: None,
            ..
        }
    )));
    assert!(facts.events.iter().any(|e| matches!(
        e.payload(),
        SafePayload::Activity {
            activity: ActivityKind::Assistant
        }
    )));
    assert!(facts.events.iter().any(|e| matches!(e.payload(), SafePayload::Operation { value, phase: Phase::Unknown } if value.kind.as_ref() == "compaction")));
    assert!(
        !serde_json::to_string(&facts.events)
            .unwrap()
            .contains("PRIVATE_")
    );
}

#[test]
fn malformed_and_reversed_native_endpoints_are_retained_as_gaps() {
    let root = tempfile::tempdir().unwrap();
    let mut malformed = item("item_completed", "AgentMessage", "m");
    malformed["payload"]["started_at_ms"] = json!("bad");
    let mut reversed = item("item_completed", "Reasoning", "r");
    reversed["payload"]["started_at_ms"] = json!(20);
    reversed["payload"]["completed_at_ms"] = json!(10);
    write(
        root.path(),
        "sessions/items.jsonl",
        &[meta("t"), malformed, reversed],
    );
    let facts = collect(root.path());
    let items: Vec<_> = facts
        .events
        .iter()
        .filter(|e| matches!(e.payload(), SafePayload::Item { .. }))
        .collect();
    assert_eq!(items.len(), 2);
    assert!(
        items
            .iter()
            .all(|e| e.gaps().contains(&Gap::InvalidNativeField))
    );
    assert_eq!(
        facts
            .issues
            .iter()
            .filter(|i| i.code == "invalidItemTiming")
            .count(),
        2
    );
}

#[test]
fn measurement_events_preserve_conflicting_source_candidates_before_reconciliation() {
    let root = tempfile::tempdir().unwrap();
    write(
        root.path(),
        "sessions/items.jsonl",
        &[
            meta("t"),
            direct("t", "a", "r", "2026-10-04T01:00:00Z", 10, 0, 1),
            direct("t", "a", "r", "2026-10-04T01:00:00Z", 20, 0, 1),
        ],
    );
    let facts = collect(root.path());
    assert_eq!(facts.measurements.len(), 1);
    assert_eq!(facts.measurements[0].tokens.input, None);
    let candidates: Vec<_> = facts
        .events
        .iter()
        .filter_map(|e| match e.payload() {
            SafePayload::Measurement {
                context_conflicts: _,
                value,
                fingerprint,
                ..
            } => Some((value, fingerprint)),
            _ => None,
        })
        .collect();
    assert_eq!(candidates.len(), 2);
    assert_ne!(candidates[0].1, candidates[1].1);
    assert_eq!(
        candidates
            .iter()
            .map(|(v, _)| v.tokens.input.unwrap())
            .collect::<BTreeSet<_>>(),
        BTreeSet::from([10, 20])
    );
    assert!(facts.issues.iter().any(|i| i.code == "measurementConflict"));
}
