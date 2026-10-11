use super::*;
use crate::session_events::{Position, Time};
use std::sync::Arc;

fn operation(row: u64, call: Option<&str>, item: Option<&str>, seed: &str) -> Event {
    let value = serde_json::from_value(serde_json::json!({
        "id":seed,"threadId":"thread","turnId":"turn","callId":call,"itemId":item,
        "kind":"mcpTool","name":"tool","sequence":row,"timestamp":null,"timePrecision":"unknown",
        "status":"running","outcomeConflict":false,"server":"server","tool":"tool",
        "evidence":[{"file":"/synthetic/a.jsonl","line":row}]
    }))
    .unwrap();
    event(
        row,
        1,
        Payload::Operation {
            value: Arc::new(value),
            phase: Phase::Started,
        },
    )
}
fn event(row: u64, ordinal: u32, payload: Payload) -> Event {
    Event::new_at(
        Position {
            source_instance_id: "source".into(),
            file_id: "file".into(),
            generation: "g".into(),
            byte_offset: row,
            ordinal,
        },
        Some("thread".into()),
        Some("turn".into()),
        Time::from_source(None).0,
        vec![],
        payload,
        "2026-10-05T00:00:00Z".into(),
    )
    .unwrap()
}
fn item(row: u64, id: &str) -> Event {
    event(
        row,
        0,
        Payload::Item {
            item_kind: ItemKind::Mcp,
            native_id: Some(id.into()),
            phase: Phase::Completed,
            started_at_ms: Some(10),
            completed_at_ms: Some(30),
            duration: None,
        },
    )
}
fn resolved(events: &[Event]) -> Resolution<'_> {
    resolve(events, &AtomicBool::new(false)).unwrap()
}

#[test]
fn late_bridge_merges_two_existing_groups_and_keeps_oldest_operation_id() {
    let a = operation(10, Some("call"), None, "stable-call");
    let b = operation(20, None, Some("item"), "stable-item");
    let bridge = operation(30, Some("call"), Some("item"), "bridge-seed");
    let before = [a.clone(), b.clone()];
    assert_eq!(resolved(&before).groups.len(), 2);
    let forward = [a.clone(), b.clone(), bridge.clone()];
    let reversed = [bridge, b, a];
    for events in [&forward, &reversed] {
        let result = resolved(events);
        assert_eq!(result.groups.len(), 1);
        assert_eq!(result.groups[0].id, "stable-call");
        assert_eq!(result.groups[0].operation_events.len(), 3);
        assert!(
            result
                .phases
                .iter()
                .all(|p| p.identity.as_deref() == Some("stable-call"))
        );
        let identity = result.phases[0].identity.as_ref().unwrap();
        assert!(
            result
                .phases
                .iter()
                .all(|phase| Arc::ptr_eq(identity, phase.identity.as_ref().unwrap()))
        );
    }
}
#[test]
fn same_text_call_and_item_stay_separate_and_disambiguate_legacy_seed_collision() {
    let events = [
        operation(10, Some("same"), None, "same-seed"),
        operation(20, None, Some("same"), "same-seed"),
    ];
    let result = resolved(&events);
    assert_eq!(result.groups.len(), 2);
    assert_eq!(result.groups[0].id, "same-seed");
    assert_ne!(result.groups[0].id, result.groups[1].id);
    let reverse = [events[1].clone(), events[0].clone()];
    assert_eq!(
        result.groups.iter().map(|g| &g.id).collect::<Vec<_>>(),
        resolved(&reverse)
            .groups
            .iter()
            .map(|g| &g.id)
            .collect::<Vec<_>>()
    );
}
#[test]
fn operation_anchor_wins_over_earlier_native_item_and_same_row_can_bridge_call_fallback() {
    let events = [
        item(1, "native"),
        item(10, "native"),
        operation(10, Some("native"), None, "operation-seed"),
    ];
    let result = resolved(&events);
    assert_eq!(result.groups.len(), 1);
    assert_eq!(result.groups[0].id, "operation-seed");
    assert_eq!(result.phases.len(), 3);
    assert!(
        result
            .phases
            .iter()
            .all(|p| p.identity.as_deref() == Some("operation-seed"))
    );
}
#[test]
fn cross_row_matching_text_does_not_bridge_native_item_and_call() {
    let events = [
        item(1, "native"),
        operation(2, Some("native"), None, "call-seed"),
    ];
    assert_eq!(resolved(&events).groups.len(), 2);
}
#[test]
fn source_scope_and_unknown_turn_are_not_guessed_from_alias_text() {
    let known = operation(1, Some("call"), None, "seed");
    let mut unknown = known.clone();
    let Payload::Operation { value, phase } = unknown.payload().clone() else {
        unreachable!()
    };
    let mut value = value.as_ref().clone();
    value.turn_id = None;
    let mut position = known.position().clone();
    position.byte_offset = 2;
    unknown = Event::new_at(
        position,
        Some("thread".into()),
        None,
        Time::from_source(None).0,
        vec![Gap::MissingIdentity],
        Payload::Operation {
            value: Arc::new(value),
            phase,
        },
        "2026-10-05T00:00:00Z".into(),
    )
    .unwrap();
    let mut position = known.position().clone();
    position.source_instance_id = "other-source".into();
    let other = Event::new_at(
        position,
        Some("thread".into()),
        Some("turn".into()),
        Time::from_source(None).0,
        vec![],
        known.payload().clone(),
        "2026-10-05T00:00:00Z".into(),
    )
    .unwrap();
    let events = [known, unknown, other];
    let result = resolved(&events);
    assert_eq!(result.groups.len(), 3);
    assert_eq!(
        result
            .groups
            .iter()
            .filter(|g| g.scope.turn.is_none())
            .count(),
        1
    );
}
#[test]
fn partitioned_scopes_preserve_global_anchor_order_and_seed_collisions() {
    let mut events = Vec::new();
    for (row, turn) in [(1, "z"), (2, "a"), (3, "z"), (4, "a")] {
        let original = operation(row, Some("call"), Some("item"), "shared-seed");
        let Payload::Operation { value, phase } = original.payload().clone() else {
            unreachable!()
        };
        let mut value = value.as_ref().clone();
        value.turn_id = Some(turn.into());
        events.push(
            Event::new_at(
                original.position().clone(),
                Some("thread".into()),
                Some(turn.into()),
                Time::from_source(None).0,
                vec![],
                Payload::Operation {
                    value: Arc::new(value),
                    phase,
                },
                "2026-10-05T00:00:00Z".into(),
            )
            .unwrap(),
        );
    }
    let expected_second = fallback_id(Scope::event(&events[1]), events[1].position());
    events.reverse();
    events.push(events[0].clone());
    let result = resolved(&events);
    assert_eq!(result.groups.len(), 2);
    assert_eq!(result.groups[0].id, "shared-seed");
    assert_eq!(result.groups[0].scope.turn, Some("z"));
    assert_eq!(result.groups[1].id, expected_second);
    assert_eq!(result.groups[1].scope.turn, Some("a"));
    assert_eq!(
        result
            .phases
            .iter()
            .map(|p| p.event.position().byte_offset)
            .collect::<Vec<_>>(),
        [1, 3, 2, 4]
    );
    let groups = resolve_groups(&events, &AtomicBool::new(false)).unwrap();
    for (group, full) in groups.iter().zip(&result.groups) {
        assert_eq!(group.id, full.id);
        assert_eq!(group.aliases, full.aliases);
        assert_eq!(group.operation_events, full.operation_events);
    }
}
#[test]
fn duplicate_observations_do_not_duplicate_phases() {
    let a = operation(1, Some("call"), Some("item"), "seed");
    let events = [a.clone(), a];
    let result = resolved(&events);
    assert_eq!(result.phases.len(), 1);
    assert_eq!(result.groups[0].operation_events.len(), 1);
}
#[test]
fn target_conflict_propagates_to_linked_native_phase_without_hiding_other_group() {
    let mut wrong = operation(20, Some("call"), Some("native"), "seed");
    let Payload::Operation { value, phase } = wrong.payload().clone() else {
        unreachable!()
    };
    let mut value = value.as_ref().clone();
    value.server = Some("other".into());
    wrong = event(
        20,
        1,
        Payload::Operation {
            value: Arc::new(value),
            phase,
        },
    );
    let events = [
        item(10, "native"),
        operation(10, Some("call"), Some("native"), "seed"),
        wrong,
        operation(30, Some("independent"), None, "other-seed"),
    ];
    let result = resolved(&events);
    assert_eq!(
        result.phases.iter().filter(|p| p.target_conflict).count(),
        3
    );
    assert_eq!(
        result.groups.iter().filter(|g| g.target_conflict).count(),
        1
    );
}
#[test]
fn generic_mcp_closure_and_unknown_result_do_not_establish_success() {
    let events = [
        item(10, "native"),
        operation(10, Some("call"), Some("native"), "seed"),
    ];
    let result = resolved(&events);
    assert!(result.phases.iter().all(|p| p.terminal_outcome.is_none()));
    assert!(!result.groups[0].outcome_conflict);
}
#[test]
fn missing_mcp_target_fields_do_not_conflict_with_known_fields() {
    let a = operation(1, Some("call"), None, "seed");
    let Payload::Operation { value, .. } = a.payload() else {
        unreachable!()
    };
    let mut missing = value.as_ref().clone();
    missing.server = None;
    missing.tool = None;
    assert!(!mcp_target_conflict(value, &missing));
    let b = event(
        2,
        1,
        Payload::Operation {
            value: Arc::new(missing),
            phase: Phase::Progress,
        },
    );
    let events = [a, b];
    assert!(!resolved(&events).groups[0].target_conflict);
}
#[test]
fn cancellation_stops_complete_association() {
    let events = [operation(1, Some("call"), None, "seed")];
    assert!(resolve(&events, &AtomicBool::new(true)).is_err());
}

#[test]
fn command_call_only_native_and_operation_share_only_matching_same_row_kind() {
    let native = event(
        10,
        0,
        Payload::Item {
            item_kind: ItemKind::Command,
            native_id: Some("command".into()),
            phase: Phase::Completed,
            started_at_ms: Some(10),
            completed_at_ms: Some(30),
            duration: None,
        },
    );
    let operation = operation(10, Some("command"), None, "command-seed");
    let Payload::Operation { value, phase } = operation.payload().clone() else {
        unreachable!()
    };
    let mut value = value.as_ref().clone();
    value.kind = "command".into();
    value.server = None;
    value.tool = None;
    let command = event(
        10,
        1,
        Payload::Operation {
            value: Arc::new(value),
            phase,
        },
    );
    let events = [native.clone(), command];
    let result = resolved(&events);
    assert_eq!(result.groups.len(), 1);
    assert_eq!(result.groups[0].id, "command-seed");
    let different = [native, operation];
    assert_eq!(resolved(&different).groups.len(), 2);
}

#[test]
fn operation_anchors_follow_existing_file_traversal_instead_of_file_hash_order() {
    let first = operation(100, Some("call"), None, "existing-first-file-id");
    let mut later = operation(1, Some("call"), Some("item"), "other-file-seed");
    let Payload::Operation { value, phase } = later.payload().clone() else {
        unreachable!()
    };
    let mut value = value.as_ref().clone();
    value.evidence[0].file = "/synthetic/z.jsonl".into();
    let mut position = later.position().clone();
    position.file_id = "000-sorts-before-file".into();
    later = Event::new_at(
        position,
        Some("thread".into()),
        Some("turn".into()),
        Time::from_source(None).0,
        vec![],
        Payload::Operation {
            value: Arc::new(value),
            phase,
        },
        "2026-10-05T00:00:00Z".into(),
    )
    .unwrap();
    let events = [later, first];
    assert_eq!(resolved(&events).groups[0].id, "existing-first-file-id");
}

#[test]
fn determinate_terminal_conflicts_survive_generic_closure_and_unknown_records() {
    let start = operation(1, Some("call"), Some("item"), "seed");
    let Payload::Operation { value, .. } = start.payload() else {
        unreachable!()
    };
    let mut success = value.as_ref().clone();
    success.status = "completed".into();
    let mut failure = success.clone();
    failure.status = "failed".into();
    let events = [
        start,
        event(
            2,
            1,
            Payload::Operation {
                value: Arc::new(success),
                phase: Phase::Completed,
            },
        ),
        event(
            3,
            1,
            Payload::Operation {
                value: Arc::new(failure),
                phase: Phase::Failed,
            },
        ),
        item(4, "item"),
    ];
    let result = resolved(&events);
    assert!(result.groups[0].outcome_conflict);
    assert!(result.phases.iter().all(|p| p.outcome_conflict));
}

#[test]
fn terminal_exit_code_conflicts_are_shared_and_start_zero_is_not_terminal() {
    let start = operation(1, Some("call"), None, "seed");
    let Payload::Operation { value, .. } = start.payload() else {
        unreachable!()
    };
    let mut running = value.as_ref().clone();
    running.exit_code = Some(0);
    let mut failed = value.as_ref().clone();
    failed.status = "failed".into();
    failed.exit_code = Some(1);
    let mut conflicting = failed.clone();
    conflicting.exit_code = Some(2);
    let start = event(
        1,
        1,
        Payload::Operation {
            value: Arc::new(running),
            phase: Phase::Started,
        },
    );
    let end = event(
        2,
        1,
        Payload::Operation {
            value: Arc::new(failed),
            phase: Phase::Failed,
        },
    );
    let events = [start.clone(), end.clone()];
    let resolution = resolved(&events);
    assert!(!resolution.groups[0].outcome_conflict);
    let endpoints = endpoints::reduce(&resolution.phases, &AtomicBool::new(false)).unwrap();
    assert_eq!(
        endpoints.groups[0].terminal.outcome(),
        Some(TerminalOutcome::Failed)
    );
    assert_eq!(endpoints.groups[0].terminal.exit_code, Some(1));
    assert_eq!(endpoints.groups[0].terminal_evidence_ids, [end.id()]);
    let events = [
        start,
        end,
        event(
            3,
            1,
            Payload::Operation {
                value: Arc::new(conflicting),
                phase: Phase::Failed,
            },
        ),
    ];
    let result = resolved(&events);
    assert!(result.groups[0].outcome_conflict);
    assert!(result.phases.iter().all(|p| p.outcome_conflict));
    let endpoints = endpoints::reduce(&result.phases, &AtomicBool::new(false)).unwrap();
    assert_eq!(
        endpoints.groups[0].terminal.state,
        lifecycle::TerminalState::Conflicting
    );
    assert_eq!(endpoints.groups[0].terminal.exit_code, None);
    assert_eq!(
        endpoints.groups[0].terminal_evidence_ids,
        [events[1].id(), events[2].id()]
    );
}

#[test]
fn native_command_completion_is_closure_without_success_claim() {
    let events = [event(
        10,
        0,
        Payload::Item {
            item_kind: ItemKind::Command,
            native_id: Some("command".into()),
            phase: Phase::Completed,
            started_at_ms: Some(10),
            completed_at_ms: Some(30),
            duration: None,
        },
    )];
    let resolution = resolved(&events);
    assert!(resolution.phases[0].terminal_outcome.is_none());
    let endpoints = endpoints::reduce(&resolution.phases, &AtomicBool::new(false)).unwrap();
    assert_eq!(endpoints.groups[0].end_ms, Some(30));
    assert_eq!(
        endpoints.groups[0].terminal.state,
        lifecycle::TerminalState::Unobserved
    );
    assert!(endpoints.groups[0].terminal_evidence_ids.is_empty());
}

#[test]
fn repeated_high_degree_bridges_keep_exact_groups_without_phase_projection() {
    let mut events = Vec::new();
    for n in 0..1500 {
        let alias = format!("item-{n}");
        let seed = format!("seed-{n}");
        events.push(operation(n * 2, Some("root-call"), Some(&alias), &seed));
        events.push(operation(n * 2 + 1, Some("root-call"), Some(&alias), &seed));
    }
    events.reverse();
    let full = resolved(&events);
    let groups = resolve_groups(&events, &AtomicBool::new(false)).unwrap();
    assert_eq!(groups.len(), 1);
    let group = &groups[0];
    assert_eq!(group.id, "seed-0");
    assert_eq!(group.operation_events.len(), 3000);
    assert_eq!(group.aliases.len(), 1501);
    assert!(!group.target_conflict && !group.outcome_conflict);
    assert_eq!(full.phases.len(), 3000);
    assert_eq!(group.id, full.groups[0].id);
    assert_eq!(group.scope, full.groups[0].scope);
    assert_eq!(group.aliases, full.groups[0].aliases);
    assert_eq!(
        group
            .operation_events
            .iter()
            .map(|e| e.id())
            .collect::<Vec<_>>(),
        full.groups[0]
            .operation_events
            .iter()
            .map(|e| e.id())
            .collect::<Vec<_>>()
    );
    assert!(resolve_groups(&events, &AtomicBool::new(true)).is_err());
}
