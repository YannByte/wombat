use super::*;
use crate::{
    adapters::contract::{
        CommandSource, OPERATION_MATCH_VERSION, Operation, OperationMatchObservation,
        ParsedCommand, ReadMatchTarget, SourcePathPlatform, WORK_OBSERVATION_VERSION, WorkData,
        WorkObservation, WorkStage,
    },
    operation_association,
    session_events::{
        ContentPhase, ContentPresence, ItemKind, LifecycleKind, MessageOrigin, MessageRecordKind,
        Payload, Phase, Position, Time,
    },
};
use serde_json::json;
fn event(offset: u64, time: Option<i64>, payload: Payload) -> Arc<Event> {
    let text = time.map(|t| {
        chrono::DateTime::from_timestamp_millis(t)
            .unwrap()
            .to_rfc3339()
    });
    Arc::new(
        Event::new(
            Position {
                source_instance_id: "source".into(),
                file_id: "file".into(),
                generation: "g".into(),
                byte_offset: offset,
                ordinal: 0,
            },
            Some("thread".into()),
            Some("turn".into()),
            Time::from_source(text.as_deref()).0,
            vec![],
            payload,
        )
        .unwrap(),
    )
}
#[allow(clippy::too_many_arguments)]
fn op(
    offset: u64,
    time: Option<i64>,
    id: &str,
    phase: Phase,
    status: &str,
    fingerprint: &str,
    targets: &[&str],
    duration: Option<u64>,
) -> Arc<Event> {
    let mut operation: Operation = serde_json::from_value(json!({
        "id": id, "threadId":"thread", "turnId":"turn", "itemId":id, "callId":null, "responseId":null,
        "kind":"command", "name":"command", "sequence":offset, "timestamp":null, "timePrecision":"unknown",
        "status":status, "exitCode":null, "outcomeConflict":false, "durationMs":duration, "path":null,
        "work":null, "server":null, "tool":null, "evidence":[{"file":"/synthetic/session.jsonl","line":offset}]
    })).unwrap();
    operation.work = Some(WorkObservation {
        format_version: WORK_OBSERVATION_VERSION,
        stage: if phase == Phase::Started {
            WorkStage::Proposed
        } else {
            WorkStage::Terminal
        },
        data: WorkData::Command {
            cwd: Some("/historical".into()),
            source: Some(CommandSource::Agent),
            parsed_commands: Some(
                targets
                    .iter()
                    .map(|path| ParsedCommand::Read {
                        path: Some((*path).into()),
                    })
                    .collect(),
            ),
        },
        gaps: vec![],
    });
    operation.matching = Some(OperationMatchObservation {
        format_version: OPERATION_MATCH_VERSION,
        receiver_owner: Some("thread".into()),
        request_fingerprint: Some(crate::hash(fingerprint)),
        function_request_fingerprint: None,
        read_targets: targets
            .iter()
            .map(|path| ReadMatchTarget {
                path: format!("/historical/{path}"),
                platform: SourcePathPlatform::Posix,
            })
            .collect(),
        expected_nonzero: false,
        gaps: vec![],
    });
    event(
        offset,
        time,
        Payload::Operation {
            value: Arc::new(operation),
            phase,
        },
    )
}
fn call(
    offset: u64,
    id: &str,
    start: i64,
    end: i64,
    status: &str,
    fingerprint: &str,
    targets: &[&str],
) -> Vec<Arc<Event>> {
    vec![
        op(
            offset,
            Some(start),
            id,
            Phase::Started,
            "running",
            fingerprint,
            targets,
            None,
        ),
        op(
            offset + 1,
            Some(end),
            id,
            if status == "failed" {
                Phase::Failed
            } else {
                Phase::Completed
            },
            status,
            fingerprint,
            targets,
            None,
        ),
    ]
}
fn project_events(events: &[Arc<Event>]) -> Projection {
    project_budget(
        events,
        &[],
        Some(intervals::Window {
            start_ms: 0,
            end_ms: 30_000,
        }),
        Budget::default(),
    )
    .unwrap()
}
fn project_budget(
    events: &[Arc<Event>],
    controls: &[Arc<Event>],
    window: Option<intervals::Window>,
    budget: Budget,
) -> Result<Projection> {
    let phases =
        operation_association::resolve(events.iter().map(Arc::as_ref), &AtomicBool::new(false))?
            .phases;
    let endpoints = operation_association::endpoints::reduce(&phases, &AtomicBool::new(false))?;
    project(
        events,
        controls,
        &phases,
        &endpoints,
        window,
        budget,
        &AtomicBool::new(false),
    )
}
fn alter(value: &Arc<Event>, change: impl FnOnce(&mut serde_json::Value)) -> Arc<Event> {
    let mut serialized = serde_json::to_value(value).unwrap();
    change(&mut serialized);
    Arc::new(serde_json::from_value(serialized).unwrap())
}

#[test]
fn unified_startup_uses_the_same_verified_chain_as_agent_but_interaction_does_not() {
    let mut events = call(1, "a", 2000, 5000, "failed", "same", &[]);
    events.extend(call(3, "b", 8000, 12000, "completed", "same", &[]));
    for (source, eligible) in [
        ("unified_exec_startup", true),
        ("unified_exec_interaction", false),
        ("user_shell", false),
    ] {
        let changed = events
            .iter()
            .map(|event| {
                alter(event, |value| {
                    value["payload"]["value"]["work"]["data"]["source"] = json!(source);
                    if !eligible {
                        value["payload"]["value"]["matching"]["receiverOwner"] =
                            serde_json::Value::Null;
                    }
                })
            })
            .collect::<Vec<_>>();
        let result = project_events(&changed);
        assert_eq!(
            result.coverage.eligible_commands,
            if eligible { 2 } else { 0 }
        );
        assert_eq!(result.after_failure.count, Some(usize::from(eligible)));
    }
}
fn matching(value: &mut serde_json::Value) -> &mut serde_json::Value {
    &mut value["payload"]["value"]["matching"]
}

#[test]
fn failed_then_success_then_success_marks_only_the_second_call() {
    let mut events = call(1, "first", 2000, 5000, "failed", "request", &[]);
    events.extend(call(3, "second", 8000, 12000, "completed", "request", &[]));
    events.extend(call(5, "third", 15000, 16000, "completed", "request", &[]));
    let result = project_events(&events);
    assert_eq!(result.after_failure.count, Some(1));
    assert_eq!(result.after_failure.duration.sum_ms, Some(4000));
    assert_eq!(result.after_failure.duration.calculated_count, 1);
    assert_eq!(result.recovery_span_sum_ms, Some(7000));
    assert_eq!(result.combined_union_ms, Some(4000));
    assert_eq!(result.details[0].operation_id, "second");
    assert_eq!(
        result.details[0].after_failure_predecessor.as_deref(),
        Some("first")
    );
}
#[test]
fn failed_then_failed_then_success_links_each_immediate_predecessor() {
    let mut events = call(1, "a", 1, 2, "failed", "same", &[]);
    events.extend(call(3, "b", 3, 4, "failed", "same", &[]));
    events.extend(call(5, "c", 5, 6, "completed", "same", &[]));
    let result = project_events(&events);
    assert_eq!(result.after_failure.count, Some(2));
    assert_eq!(
        result.details[0].after_failure_predecessor.as_deref(),
        Some("a")
    );
    assert_eq!(
        result.details[1].after_failure_predecessor.as_deref(),
        Some("b")
    );
    assert_eq!(result.after_failure.duration.sum_ms, Some(2));
}
#[test]
fn indeterminate_or_unfinished_nearest_request_breaks_failure_link() {
    for completed in [false, true] {
        let mut events = call(1, "a", 1, 2, "failed", "same", &[]);
        events.push(op(
            3,
            Some(3),
            "b",
            Phase::Started,
            "running",
            "same",
            &[],
            None,
        ));
        if completed {
            events.push(op(
                4,
                Some(4),
                "b",
                Phase::Completed,
                "unknown",
                "same",
                &[],
                None,
            ));
        }
        events.extend(call(5, "c", 5, 6, "completed", "same", &[]));
        let result = project_events(&events);
        assert_eq!(result.after_failure.count, Some(1));
        assert_eq!(result.details[0].operation_id, "b");
        assert_eq!(result.coverage.indeterminate_outcomes, 1);
    }
}
#[test]
fn completion_without_start_can_prove_one_prior_call_but_not_ambiguous_order() {
    let mut events = vec![op(
        1,
        Some(5000),
        "a",
        Phase::Failed,
        "failed",
        "same",
        &[],
        Some(3000),
    )];
    events.extend(call(2, "b", 8000, 12000, "completed", "same", &[]));
    let result = project_events(&events);
    assert_eq!(result.after_failure.count, Some(1));
    assert_eq!(result.coverage.missing_start, 1);
    assert_eq!(result.recovery_span_sum_ms, Some(7000));
    events.extend(call(4, "c", 15000, 16000, "completed", "same", &[]));
    assert_eq!(project_events(&events).after_failure.count, Some(1));
    let mut ambiguous = call(1, "older", 2000, 4000, "failed", "same", &[]);
    ambiguous.push(op(
        3,
        Some(7000),
        "untimed",
        Phase::Failed,
        "failed",
        "same",
        &[],
        None,
    ));
    ambiguous.extend(call(4, "later", 8000, 12000, "completed", "same", &[]));
    let result = project_events(&ambiguous);
    assert_eq!(result.after_failure.count, Some(0));
    assert!(result.coverage.order_gaps > 0);
}
#[test]
fn missing_later_completion_retains_the_count_and_missing_duration() {
    let mut events = call(1, "a", 2000, 5000, "failed", "same", &[]);
    events.push(op(
        3,
        Some(8000),
        "b",
        Phase::Started,
        "running",
        "same",
        &[],
        None,
    ));
    let result = project_events(&events);
    assert_eq!(result.after_failure.count, Some(1));
    assert_eq!(result.after_failure.duration.sum_ms, None);
    assert_eq!(result.after_failure.duration.missing_count, 1);
    assert_eq!(result.recovery_span_sum_ms, None);
    assert_eq!(result.missing_recovery_span_count, 1);
    assert_eq!(result.combined_missing_interval_count, 1);
    assert_eq!(result.combined_union_ms, Some(0));
}
#[test]
fn successful_read_requires_confirmed_targets_and_prior_success() {
    for status in ["completed", "failed", "unknown", "cancelled"] {
        let mut events = call(1, "a", 1000, 2000, status, "read-first", &["file"]);
        events.extend(call(
            3,
            "b",
            3000,
            4000,
            "completed",
            "different-range-request",
            &["file"],
        ));
        let result = project_events(&events);
        assert_eq!(
            result.repeated_read.count,
            Some(usize::from(status == "completed"))
        );
        assert_eq!(result.repeated_read_request_count, Some(1));
        if status == "completed" {
            assert_eq!(
                result.details[0].read_layer,
                Some(ReadLayer::SamePathRangeUnconfirmed)
            );
        }
    }
    let mut events = call(1, "search", 1, 2, "completed", "search", &[]);
    events.extend(call(3, "read", 3, 4, "completed", "read", &["file"]));
    assert_eq!(project_events(&events).repeated_read.count, Some(0));
}
#[test]
fn multiple_targets_count_the_later_operation_once_and_keep_predecessors() {
    let mut events = call(1, "a", 1, 2, "completed", "one", &["a"]);
    events.extend(call(3, "b", 3, 4, "completed", "two", &["b"]));
    events.extend(call(5, "both", 5, 6, "completed", "both", &["a", "b"]));
    let result = project_events(&events);
    assert_eq!(result.repeated_read.count, Some(1));
    assert_eq!(result.repeated_read.duration.sum_ms, Some(1));
    assert_eq!(result.details[0].repeated_read_targets, 2);
    assert_eq!(result.details[0].successful_read_predecessors, ["a", "b"]);
}
#[test]
fn two_labels_and_overlapping_operations_share_one_union() {
    let mut events = call(1, "read-old", 0, 1000, "completed", "head-file", &["file"]);
    events.extend(call(3, "failed", 2000, 5000, "failed", "cat-file", &[]));
    events.extend(call(
        5,
        "later",
        8000,
        12000,
        "completed",
        "cat-file",
        &["file"],
    ));
    events.extend(call(
        7,
        "overlap",
        9000,
        14000,
        "completed",
        "third-read",
        &["file"],
    ));
    let result = project_events(&events);
    assert_eq!(result.after_failure.count, Some(1));
    assert_eq!(result.repeated_read.count, Some(2)); // Both requests follow the earlier confirmed successful read.
    assert_eq!(result.combined_operation_count, Some(2));
    assert_eq!(result.combined_union_ms, Some(6000));
    assert!(result.details[0].after_failure_predecessor.is_some());
    assert_eq!(result.details[0].successful_read_predecessors, ["read-old"]);
}
#[test]
fn independent_native_duration_and_clipped_union_remain_separate() {
    let mut events = call(1, "a", 1, 2, "failed", "same", &[]);
    events.extend(call(3, "b", 3, 10, "completed", "same", &[]));
    events[3] = alter(&events[3], |v| {
        v["payload"]["value"]["durationMs"] = json!(11)
    });
    let result = project_budget(
        &events,
        &[],
        Some(intervals::Window {
            start_ms: 0,
            end_ms: 8,
        }),
        Budget::default(),
    )
    .unwrap();
    assert_eq!(result.after_failure.duration.sum_ms, Some(11));
    assert_eq!(result.after_failure.duration.recorded_count, 1);
    assert_eq!(result.combined_union_ms, Some(5));
    let result = project_budget(&events, &[], None, Budget::default()).unwrap();
    assert_eq!(result.after_failure.count, Some(1));
    assert_eq!(result.after_failure.duration.sum_ms, Some(11));
    assert_eq!(result.combined_union_ms, None);
}
#[test]
fn explicit_zero_is_retained_and_conflicting_native_durations_are_not_repaired() {
    let mut events = call(1, "a", 0, 1, "failed", "same", &[]);
    events.extend(call(3, "b", 2, 2, "completed", "same", &[]));
    events[3] = alter(&events[3], |v| {
        v["payload"]["value"]["durationMs"] = json!(0)
    });
    assert_eq!(
        project_events(&events).after_failure.duration.sum_ms,
        Some(0)
    );
    let extra = alter(&events[3], |v| {
        v["position"]["byteOffset"] = json!(5);
        v["id"] = json!(
            Position {
                source_instance_id: "source".into(),
                file_id: "file".into(),
                generation: "g".into(),
                byte_offset: 5,
                ordinal: 0
            }
            .event_id()
            .unwrap()
        );
        v["payload"]["value"]["durationMs"] = json!(1);
    });
    events.push(extra);
    let result = project_events(&events);
    assert_eq!(result.after_failure.count, Some(1));
    assert_eq!(result.after_failure.duration.sum_ms, None);
    assert_eq!(result.coverage.duration_conflicts, 1);
    assert_eq!(result.combined_union_ms, Some(0));
}
#[test]
fn expected_no_match_and_cancellation_do_not_establish_failure() {
    for (status, code, expected_count) in [
        ("failed", 1, 0),
        ("failed", 2, 1),
        ("failed", 0, 1),
        ("cancelled", 2, 0),
    ] {
        let mut events = call(1, "a", 1, 2, status, "same", &[]);
        for value in &mut events {
            *value = alter(value, |v| {
                matching(v)["expectedNonzero"] = json!(true);
                v["payload"]["value"]["exitCode"] = json!(code);
            });
        }
        events.extend(call(3, "b", 3, 4, "completed", "same", &[]));
        assert_eq!(
            project_events(&events).after_failure.count,
            Some(expected_count)
        );
    }
}
#[test]
fn reliable_dispatch_times_override_completion_order_and_equal_starts_are_ambiguous() {
    let mut events = call(1, "earlier-dispatch", 1, 8, "failed", "same", &[]);
    events.extend(call(3, "later-dispatch", 2, 4, "completed", "same", &[]));
    events.extend(call(5, "last", 9, 10, "completed", "same", &[]));
    assert_eq!(project_events(&events).after_failure.count, Some(0)); // The nearest call succeeded.
    let mut equal = call(1, "a", 1, 2, "failed", "same", &[]);
    equal.extend(call(3, "b", 1, 3, "failed", "same", &[]));
    equal.extend(call(5, "c", 4, 5, "completed", "same", &[]));
    let result = project_events(&equal);
    assert_eq!(result.after_failure.count, Some(0));
    assert!(result.coverage.order_gaps >= 2);
}
fn user_boundary(offset: u64, time: i64) -> Arc<Event> {
    event(
        offset,
        Some(time),
        Payload::Message {
            origin: MessageOrigin::UserInput,
            presence: ContentPresence::Unknown,
            native_id: None,
            record_kind: MessageRecordKind::NativeSnapshot,
            record_phase: Phase::Completed,
            content_phase: ContentPhase::Unknown,
        },
    )
}
#[test]
fn user_input_compaction_and_source_damage_reset_both_chains() {
    let boundaries = [
        user_boundary(3, 3),
        event(
            3,
            Some(3),
            Payload::Lifecycle {
                lifecycle: LifecycleKind::Compaction,
                phase: Phase::Completed,
                native_id: Some("compact".into()),
                duration_ms: None,
                first_token_ms: None,
            },
        ),
        alter(&user_boundary(3, 3), |v| {
            v["gaps"] = json!(["source_partial"]);
            v["payload"]["origin"] = json!("assistant_visible");
        }),
    ];
    for boundary in boundaries {
        let mut events = call(1, "failed", 1, 2, "failed", "same", &[]);
        events.push(boundary.clone());
        events.extend(call(4, "later", 4, 5, "completed", "same", &[]));
        assert_eq!(project_events(&events).after_failure.count, Some(0));
        let mut events = call(1, "read", 1, 2, "completed", "read", &["a"]);
        events.push(boundary);
        events.extend(call(4, "later", 4, 5, "completed", "read", &["a"]));
        assert_eq!(project_events(&events).repeated_read.count, Some(0));
    }
}
#[test]
fn unassigned_source_damage_is_a_control_boundary_without_becoming_a_turn_fact() {
    let control = alter(&user_boundary(3, 3), |v| {
        v["turnId"] = serde_json::Value::Null;
        v["gaps"] = json!(["source_partial"]);
        v["payload"]["origin"] = json!("assistant_visible");
    });
    let mut events = call(1, "a", 1, 2, "failed", "same", &[]);
    events.extend(call(4, "b", 4, 5, "completed", "same", &[]));
    let result = project_budget(&events, &[control], None, Budget::default()).unwrap();
    assert_eq!(result.after_failure.count, Some(0));
    assert!(result.coverage.partial);
}
#[test]
fn malformed_request_or_unknown_receiver_cannot_bridge_supported_calls() {
    for receiver_gap in [false, true] {
        let mut events = call(1, "a", 1, 2, "failed", "same", &[]);
        let mut unsafe_call = call(3, "unsafe", 3, 4, "failed", "same", &[]);
        for value in &mut unsafe_call {
            *value = alter(value, |v| {
                if receiver_gap {
                    matching(v)["receiverOwner"] = serde_json::Value::Null;
                    matching(v)["gaps"] = json!(["missing_receiver"]);
                } else {
                    matching(v)["requestFingerprint"] = serde_json::Value::Null;
                    matching(v)["gaps"] = json!(["unsupported_parameters"]);
                }
            });
        }
        events.extend(unsafe_call);
        events.extend(call(5, "c", 5, 6, "completed", "same", &[]));
        let result = project_events(&events);
        assert_eq!(result.after_failure.count, Some(0));
        assert!(result.coverage.partial);
    }
}
#[test]
fn replay_and_alias_bridges_keep_one_operation_identity() {
    let mut events = call(1, "a", 1, 2, "failed", "same", &[]);
    events.extend(call(3, "b", 3, 4, "completed", "same", &[]));
    let duplicate = events.clone();
    events.extend(duplicate);
    events.reverse();
    assert_eq!(project_events(&events).after_failure.count, Some(1));
    let first = alter(
        &op(
            1,
            Some(1),
            "a",
            Phase::Started,
            "running",
            "same",
            &[],
            None,
        ),
        |v| {
            v["payload"]["value"]["itemId"] = serde_json::Value::Null;
            v["payload"]["value"]["callId"] = json!("call");
        },
    );
    let bridge = alter(
        &op(
            2,
            Some(2),
            "bridge",
            Phase::Failed,
            "failed",
            "same",
            &[],
            None,
        ),
        |v| {
            v["payload"]["value"]["itemId"] = json!("item");
            v["payload"]["value"]["callId"] = json!("call");
        },
    );
    let end = alter(
        &op(
            3,
            Some(2),
            "native-end",
            Phase::Failed,
            "failed",
            "same",
            &[],
            None,
        ),
        |v| v["payload"]["value"]["itemId"] = json!("item"),
    );
    let result = project_events(&[first, bridge, end]);
    assert_eq!(result.coverage.candidates, 1);
    assert_eq!(result.after_failure.count, Some(0));
}
#[test]
fn file_generation_and_turn_ownership_do_not_cross() {
    for field in ["file", "generation", "turn", "thread", "source"] {
        let mut events = call(1, "a", 1, 2, "failed", "same", &["a"]);
        let mut later = call(3, "b", 3, 4, "completed", "same", &["a"]);
        for value in &mut later {
            *value = alter(value, |v| {
                match field {
                    "file" => v["position"]["fileId"] = json!("other"),
                    "generation" => v["position"]["generation"] = json!("other"),
                    "source" => v["position"]["sourceInstanceId"] = json!("other"),
                    "turn" => {
                        v["turnId"] = json!("other");
                        v["payload"]["value"]["turnId"] = json!("other");
                    }
                    "thread" => {
                        v["threadId"] = json!("other");
                        v["payload"]["value"]["threadId"] = json!("other");
                        matching(v)["receiverOwner"] = json!("other");
                    }
                    _ => unreachable!(),
                }
                let position: Position = serde_json::from_value(v["position"].clone()).unwrap();
                v["id"] = json!(position.event_id().unwrap());
            });
        }
        events.extend(later);
        assert_eq!(
            project_events(&events).after_failure.count,
            Some(0),
            "{field}"
        );
    }
}
#[test]
fn complete_totals_survive_detail_overflow_and_limits_never_return_a_prefix() {
    let mut events = Vec::new();
    for index in 0..205 {
        events.extend(call(
            index * 2,
            &format!("op{index}"),
            index as i64 * 10,
            index as i64 * 10 + 1,
            "failed",
            "same",
            &[],
        ));
    }
    let result = project_events(&events);
    assert_eq!(result.after_failure.count, Some(204));
    assert_eq!(result.after_failure.duration.sum_ms, Some(204));
    assert_eq!(result.combined_union_ms, Some(204));
    assert!(result.detail_limited);
    assert!(result.details.is_empty());
    for budget in [
        Budget {
            operations: 1,
            ..Budget::default()
        },
        Budget {
            metadata: 0,
            ..Budget::default()
        },
        Budget {
            string_bytes: 1,
            ..Budget::default()
        },
    ] {
        let result = project_budget(&events, &[], None, budget).unwrap();
        assert!(!result.computed);
        assert_eq!(result.after_failure.count, None);
        assert_eq!(result.combined_union_ms, None);
        assert!(result.coverage.budget_exceeded);
    }
}
#[test]
fn cancellation_returns_an_error_and_analysis_budget_keeps_unavailable_counts() {
    let events = call(1, "a", 1, 2, "failed", "same", &[]);
    let phases =
        operation_association::resolve(events.iter().map(Arc::as_ref), &AtomicBool::new(false))
            .unwrap()
            .phases;
    let endpoints =
        operation_association::endpoints::reduce(&phases, &AtomicBool::new(false)).unwrap();
    assert!(
        project(
            &events,
            &[],
            &phases,
            &endpoints,
            None,
            Budget::default(),
            &AtomicBool::new(true)
        )
        .is_err()
    );
    let result = super::super::analysis::analyze(super::super::analysis::AnalyzeInput {
        source: "source",
        thread: "thread",
        turn: "turn",
        measurements: &[],
        events: &events,
        budget: super::super::analysis::Budget {
            events: 1,
            measurements: 1,
            lifecycle_records: 1,
        },
    });
    assert_eq!(result.repeated_behavior.after_failure.count, None);
    assert!(result.repeated_behavior.coverage.budget_exceeded);
}
#[test]
fn request_observations_remain_available_without_timestamps_or_known_success() {
    let events = [
        op(
            1,
            None,
            "a",
            Phase::Completed,
            "unknown",
            "same",
            &["a"],
            None,
        ),
        op(
            2,
            None,
            "b",
            Phase::Completed,
            "unknown",
            "same",
            &["a"],
            None,
        ),
        op(
            3,
            None,
            "c",
            Phase::Completed,
            "unknown",
            "same",
            &["a"],
            None,
        ),
    ];
    let result = project_events(&events);
    assert_eq!(result.same_request_observation_count, Some(2));
    assert_eq!(result.repeated_read_request_count, Some(2));
    assert_eq!(result.after_failure.count, Some(0));
    assert_eq!(result.repeated_read.count, Some(0));
    assert!(result.coverage.partial);
    let damaged_time: Vec<_> = events
        .iter()
        .map(|e| alter(e, |v| v["gaps"] = json!(["invalid_timestamp"])))
        .collect();
    assert_eq!(
        project_events(&damaged_time).same_request_observation_count,
        Some(2)
    );
    assert_eq!(
        project_events(&damaged_time).repeated_read_request_count,
        Some(2)
    );
}
#[test]
fn matching_overlapping_operations_count_combined_time_once() {
    let mut events = call(1, "a-failed", 2000, 5000, "failed", "a", &[]);
    events.extend(call(3, "b-failed", 3000, 6000, "failed", "b", &[]));
    events.extend(call(5, "a-later", 8000, 12000, "completed", "a", &[]));
    events.extend(call(7, "b-later", 9000, 14000, "completed", "b", &[]));
    let result = project_events(&events);
    assert_eq!(result.after_failure.count, Some(2));
    assert_eq!(result.after_failure.duration.sum_ms, Some(9000));
    assert_eq!(result.combined_union_ms, Some(6000));
}
#[test]
fn crossed_context_and_reversed_endpoints_cannot_form_links() {
    let mut events = call(1, "a", 1, 2, "failed", "same", &[]);
    events.push(user_boundary(3, 3));
    events.extend(call(4, "b", 2, 5, "failed", "same", &[])); // Native dispatch precedes the observed new context.
    events.extend(call(6, "c", 6, 7, "completed", "same", &[]));
    let result = project_events(&events);
    assert_eq!(result.coverage.crossed_context, 1);
    assert_eq!(result.after_failure.count, Some(0));
    let mut events = call(1, "a", 4, 2, "failed", "same", &[]);
    events.extend(call(3, "b", 5, 6, "completed", "same", &[]));
    let result = project_events(&events);
    assert_eq!(result.after_failure.count, Some(0));
    assert!(result.coverage.conflicting > 0);
}
#[test]
fn same_instant_read_receipts_never_link_to_themselves_or_create_a_cycle() {
    let mut events = call(1, "a", 0, 0, "completed", "same", &["a"]);
    events.extend(call(3, "b", 0, 0, "completed", "same", &["a"]));
    let result = project_events(&events);
    assert_eq!(result.repeated_read.count, Some(1));
    assert_eq!(result.repeated_read.duration.sum_ms, Some(0));
    assert_eq!(result.details[0].operation_id, "b");
    assert_eq!(result.details[0].successful_read_predecessors, ["a"]);
}
#[test]
fn independent_native_completion_anchors_have_priority_without_inventing_a_dispatch() {
    let native = event(
        2,
        None,
        Payload::Item {
            item_kind: ItemKind::Command,
            native_id: Some("a".into()),
            phase: Phase::Completed,
            started_at_ms: None,
            completed_at_ms: Some(5000),
            duration: None,
        },
    );
    let mut events = vec![
        op(
            1,
            Some(100_000),
            "a",
            Phase::Failed,
            "failed",
            "same",
            &[],
            Some(3000),
        ),
        native,
    ];
    events.extend(call(3, "b", 8000, 12000, "completed", "same", &[]));
    let result = project_events(&events);
    assert_eq!(result.after_failure.count, Some(1));
    assert_eq!(result.recovery_span_sum_ms, Some(7000));
    assert_eq!(result.coverage.missing_start, 1);
    let phases =
        operation_association::resolve(events.iter().map(Arc::as_ref), &AtomicBool::new(false))
            .unwrap()
            .phases;
    let endpoints =
        operation_association::endpoints::reduce(&phases, &AtomicBool::new(false)).unwrap();
    let prior = endpoints
        .groups
        .iter()
        .find(|group| group.identity == "a")
        .unwrap();
    assert_eq!(prior.completion_ms, Some(5000));
    assert_eq!(prior.end_ms, None);
    assert_eq!(prior.start_ms, None);
}
#[test]
fn native_duration_and_cross_generation_halves_never_supply_completion_time() {
    let native = event(
        2,
        None,
        Payload::Item {
            item_kind: ItemKind::Command,
            native_id: Some("a".into()),
            phase: Phase::Completed,
            started_at_ms: None,
            completed_at_ms: None,
            duration: Some(crate::session_events::NativeDuration { secs: 3, nanos: 0 }),
        },
    );
    let mut events = vec![
        op(1, None, "a", Phase::Failed, "failed", "same", &[], None),
        native,
    ];
    events.extend(call(3, "b", 8000, 12000, "completed", "same", &[]));
    assert_eq!(project_events(&events).after_failure.count, Some(0));
    events[1] = alter(&events[1], |v| {
        v["position"]["generation"] = json!("foreign");
        v["payload"]["completed_at_ms"] = json!(5000);
        let position: Position = serde_json::from_value(v["position"].clone()).unwrap();
        v["id"] = json!(position.event_id().unwrap());
    });
    let result = project_events(&events);
    assert_eq!(result.after_failure.count, Some(0));
    assert!(result.coverage.missing_clock_domain > 0);
}
#[test]
fn conflicting_native_work_metadata_does_not_bypass_canonical_command_policy() {
    let mut events = call(1, "a", 1, 2, "failed", "same", &["a"]);
    events[1] = alter(&events[1], |v| {
        v["payload"]["value"]["work"]["data"]["parsed_commands"] = json!([{"kind":"unknown"}])
    });
    events.extend(call(3, "b", 3, 4, "completed", "same", &["a"]));
    let result = project_events(&events);
    assert_eq!(result.coverage.conflicting, 1);
    assert_eq!(result.after_failure.count, Some(0));
    assert_eq!(result.repeated_read.count, Some(0));
}

fn mcp_events(events: &[Arc<Event>]) -> Vec<Arc<Event>> {
    events
        .iter()
        .map(|e| {
            alter(e, |v| {
                let op = &mut v["payload"]["value"];
                op["kind"] = json!("mcpTool");
                op["server"] = json!("synthetic");
                op["tool"] = json!("search");
                op["work"] = serde_json::Value::Null;
            })
        })
        .collect()
}
#[test]
fn native_mcp_requests_reuse_predecessor_duration_and_observation_analysis() {
    let mut events = call(1, "a", 2000, 5000, "failed", "same", &[]);
    events.extend(call(3, "b", 8000, 12000, "completed", "same", &[]));
    let events = mcp_events(&events);
    let result = project_events(&events);
    assert_eq!(result.after_failure.count, Some(1));
    assert_eq!(result.after_failure.duration.sum_ms, Some(4000));
    assert_eq!(result.same_request_observation_count, Some(1));
    assert_eq!(result.repeated_read.count, Some(0));
    assert_eq!(result.coverage.eligible_commands, 0);
    assert_eq!(result.recovery_span_sum_ms, Some(7000));
    let no_times: Vec<_> = events
        .iter()
        .map(|e| {
            alter(e, |v| {
                v["time"]["timestamp"] = serde_json::Value::Null;
                v["time"]["precision"] = json!("unknown");
            })
        })
        .collect();
    let observations = project_events(&no_times);
    assert_eq!(observations.same_request_observation_count, Some(1));
    assert_eq!(observations.after_failure.count, Some(0));
}
#[test]
fn unsupported_mcp_request_breaks_matching_chain() {
    let mut events = call(1, "a", 1, 2, "failed", "same", &[]);
    events.extend(call(3, "b", 3, 4, "completed", "same", &[]));
    events.extend(call(5, "c", 5, 6, "completed", "same", &[]));
    let mut events = mcp_events(&events);
    for e in &mut events[2..4] {
        *e = alter(e, |v| {
            matching(v)["requestFingerprint"] = serde_json::Value::Null;
            matching(v)["gaps"] = json!(["unsupported_parameters"]);
        });
    }
    let result = project_events(&events);
    assert_eq!(result.after_failure.count, Some(0));
    assert_eq!(result.same_request_observation_count, Some(0));
    assert!(result.coverage.context_resets > 0);
    assert!(result.coverage.missing_matching > 0);
    assert_eq!(result.coverage.conflicting, 0);
}

#[test]
fn common_matching_validation_rejects_foreign_owner_and_fabricated_mcp_reads() {
    let source = mcp_events(&call(1, "a", 1, 2, "failed", "same", &[]));
    for (field, value) in [
        ("receiverOwner", json!("another-thread")),
        (
            "readTargets",
            json!([{"path":"/synthetic/a","platform":"posix"}]),
        ),
        ("expectedNonzero", json!(true)),
        ("formatVersion", json!(0)),
    ] {
        let mut encoded = serde_json::to_value(&source[0]).unwrap();
        matching(&mut encoded)[field] = value;
        assert!(serde_json::from_value::<Event>(encoded).is_err(), "{field}");
    }
    for invalid in ["", "bad\nserver", &"s".repeat(4097)] {
        let mut encoded = serde_json::to_value(&source[0]).unwrap();
        encoded["payload"]["value"]["server"] = json!(invalid);
        assert!(serde_json::from_value::<Event>(encoded).is_err());
    }
}

#[test]
fn many_predecessors_omit_all_proofs_without_losing_full_repeat_totals() {
    let targets: Vec<_> = (0..210).map(|n| format!("p{n}")).collect();
    let mut events = vec![];
    for (n, target) in targets.iter().enumerate() {
        events.extend(call(
            (n * 2 + 1) as u64,
            &format!("prior-{n}"),
            (n * 2) as i64,
            (n * 2 + 1) as i64,
            "completed",
            target,
            &[target],
        ));
    }
    let refs: Vec<_> = targets.iter().map(String::as_str).collect();
    for n in 0..3 {
        events.extend(call(
            421 + n * 2,
            &format!("later-{n}"),
            500 + n as i64 * 2,
            501 + n as i64 * 2,
            if n == 2 { "completed" } else { "failed" },
            "same",
            &refs,
        ));
    }
    let result = project_events(&events);
    assert!(result.detail_limited);
    assert!(result.details.is_empty());
    assert_eq!(result.repeated_read.count, Some(3));
    assert_eq!(result.after_failure.count, Some(2));
    assert_eq!(result.repeated_read.duration.sum_ms, Some(3));
}

#[test]
fn predecessor_proof_retains_later_exit_code_fill_and_conflict_witnesses() {
    let mut events = call(1, "prior", 10, 20, "completed", "same", &[]);
    let code = alter(
        &op(
            3,
            Some(20),
            "prior",
            Phase::Completed,
            "completed",
            "same",
            &[],
            None,
        ),
        |v| {
            v["payload"]["value"]["exitCode"] = json!(2);
        },
    );
    events.push(code.clone());
    events.extend(call(4, "later", 30, 40, "completed", "same", &[]));
    let result = project_events(&events);
    assert_eq!(result.after_failure.count, Some(1));
    assert!(
        result.details[0]
            .after_failure_evidence
            .as_ref()
            .unwrap()
            .contains(&code.id().into())
    );
    let conflict = alter(
        &op(
            6,
            Some(40),
            "later",
            Phase::Completed,
            "completed",
            "same",
            &[],
            None,
        ),
        |v| {
            v["payload"]["value"]["exitCode"] = json!(0);
        },
    );
    let conflict2 = alter(
        &op(
            7,
            Some(40),
            "later",
            Phase::Completed,
            "completed",
            "same",
            &[],
            None,
        ),
        |v| {
            v["payload"]["value"]["exitCode"] = json!(2);
        },
    );
    events.extend([conflict.clone(), conflict2.clone()]);
    let result = project_events(&events);
    assert_eq!(result.after_failure.count, Some(1));
    assert!(
        result.details[0]
            .later_evidence
            .contains(&conflict.id().into())
    );
    assert!(
        result.details[0]
            .later_evidence
            .contains(&conflict2.id().into())
    );
    assert!(result.details[0].later_evidence.len() <= PROOF_REF_LIMIT);
}

#[test]
fn callable_fallback_and_native_precedence_share_counting_without_cross_form_matches() {
    let mut events = call(1, "a", 1, 2, "failed", "native", &[]);
    events.extend(call(3, "b", 3, 4, "completed", "native", &[]));
    let native = mcp_events(&events);
    let generic: Vec<_> = native
        .iter()
        .map(|e| {
            alter(e, |v| {
                let op = &mut v["payload"]["value"];
                op["kind"] = json!("mcp");
                op["server"] = serde_json::Value::Null;
                op["tool"] = serde_json::Value::Null;
                let m = matching(v);
                m["requestFingerprint"] = serde_json::Value::Null;
                m["functionRequestFingerprint"] = json!(crate::hash("callable"));
            })
        })
        .collect();
    let result = project_events(&generic);
    assert_eq!(result.same_request_observation_count, Some(1));
    assert_eq!(result.after_failure.count, Some(1));
    assert_eq!(result.coverage.eligible_commands, 0);
    assert_eq!(result.repeated_read.count, Some(0));
    let with_native: Vec<_> = native
        .iter()
        .map(|e| {
            alter(e, |v| {
                matching(v)["functionRequestFingerprint"] =
                    json!(crate::hash(if v["payload"]["value"]["id"] == "a" {
                        "caller-a"
                    } else {
                        "caller-b"
                    }));
            })
        })
        .collect();
    assert_eq!(project_events(&with_native).after_failure.count, Some(1));
    let mut mixed = generic[..2].to_vec();
    mixed.extend_from_slice(&native[2..]);
    assert_eq!(
        project_events(&mixed).same_request_observation_count,
        Some(0)
    );
    let mut conflicting = generic.clone();
    conflicting[1] = alter(&conflicting[1], |v| {
        matching(v)["functionRequestFingerprint"] = json!(crate::hash("changed"));
    });
    assert_eq!(project_events(&conflicting).after_failure.count, Some(0));
    assert!(project_events(&conflicting).coverage.conflicting > 0);
}
