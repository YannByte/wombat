use super::*;
use crate::session_events::{
    ContentPhase, ContentPresence as C, MessageOrigin as O, MessageRecordKind as R,
    Payload as SafePayload, Phase,
};

fn response(role: &str, body: Value) -> Value {
    json!({"type":"response_item","timestamp":"2026-10-04T01:00:00.150Z","payload":{"type":"message","role":role,"id":"native-message","content":body,"internal_chat_message_metadata_passthrough":{"turn_id":"turn"}}})
}
fn text() -> Value {
    json!([{"type":"output_text","text":"PRIVATE_SYNTHETIC_TEXT https://private.invalid"}])
}

#[test]
fn interface_provenance_uses_complete_declared_kinds_not_text_or_prefixes() {
    for (kinds, role, expected) in [
        (
            json!(["additional_content.codex_apps_open_page"]),
            "user",
            O::InjectedContext,
        ),
        (
            json!([
                "agents_md.instructions",
                "additional_content.codex_apps_open_page"
            ]),
            "user",
            O::InjectedContext,
        ),
        (
            json!(["additional_content.codex_apps_open_page", "user.text"]),
            "user",
            O::UserUnclassified,
        ),
        (
            json!(["additional_content.future"]),
            "user",
            O::UserUnclassified,
        ),
        (json!([]), "user", O::UserUnclassified),
        (Value::Null, "user", O::UserUnclassified),
        (
            json!(["additional_content.codex_apps_open_page"]),
            "assistant",
            O::AssistantVisible,
        ),
    ] {
        let root = tempfile::tempdir().unwrap();
        let mut row = response(
            role,
            json!([{"type":"input_text","text":"<external_codex_apps_open_page>{\"page_id\":null}</external_codex_apps_open_page>"}]),
        );
        if !kinds.is_null() {
            row["payload"]["internal_chat_message_metadata_passthrough"]["content_item_kinds"] =
                kinds;
        }
        write(
            root.path(),
            "sessions/ui.jsonl",
            &[meta("thread"), context("turn", "model", "medium"), row],
        );
        let facts = collect(root.path());
        let origins: Vec<_> = facts
            .events
            .iter()
            .filter_map(|e| match e.payload() {
                SafePayload::Message { origin, .. } => Some(*origin),
                _ => None,
            })
            .collect();
        assert_eq!(origins, vec![expected]);
        assert!(facts.measurements.is_empty());
        assert!(facts.operations.is_empty());
    }
}

#[test]
fn supported_message_styles_keep_only_safe_presence_phase_identity_and_record_time() {
    let root = tempfile::tempdir().unwrap();
    let legacy = json!({"type":"event_msg","timestamp":"2026-10-04T01:00:00.125Z","payload":{"type":"agent_message","turn_id":"turn","message":"PRIVATE_LEGACY","phase":"commentary"}});
    let mut response = response("assistant", text());
    response["payload"]["phase"] = json!("final_answer");
    let native = json!({"type":"event_msg","timestamp":"2026-10-04T01:00:00.175Z","payload":{"type":"item_completed","turn_id":"turn","started_at_ms":1,"completed_at_ms":2,"item":{"type":"AgentMessage","id":"native-item","content":[{"type":"Text","text":"PRIVATE_NATIVE"}],"phase":"commentary"}}});
    let delta = json!({"type":"event_msg","timestamp":"2026-10-04T01:00:00.180Z","payload":{"type":"agent_message_content_delta","turn_id":"turn","item_id":"native-item","delta":"PRIVATE_DELTA"}});
    write(
        root.path(),
        "sessions/messages.jsonl",
        &[
            meta("thread"),
            context("turn", "model", "medium"),
            legacy,
            response,
            native,
            delta,
        ],
    );
    let facts = collect(root.path());
    let messages: Vec<_> = facts
        .events
        .iter()
        .filter(|event| matches!(event.payload(), SafePayload::Message { .. }))
        .collect();
    assert_eq!(messages.len(), 4);
    let legacy = messages
        .iter()
        .find(|event| {
            matches!(
                event.payload(),
                SafePayload::Message {
                    record_kind: R::LegacySnapshot,
                    ..
                }
            )
        })
        .unwrap();
    assert!(matches!(
        legacy.payload(),
        SafePayload::Message {
            origin: O::AssistantVisible,
            presence: C::NonEmpty,
            native_id: None,
            content_phase: ContentPhase::Commentary,
            ..
        }
    ));
    assert!(messages.iter().any(|event| matches!(event.payload(), SafePayload::Message { record_kind: R::ResponseSnapshot, native_id: Some(id), content_phase: ContentPhase::FinalAnswer, .. } if id == "native-message")));
    let native = messages
        .iter()
        .find(|event| {
            matches!(
                event.payload(),
                SafePayload::Message {
                    record_kind: R::NativeSnapshot,
                    ..
                }
            )
        })
        .unwrap();
    assert!(
        matches!(native.payload(), SafePayload::Message { native_id: Some(id), presence: C::NonEmpty, record_phase: Phase::Completed, .. } if id == "native-item")
    );
    assert_eq!(
        native.time().timestamp.as_deref(),
        Some("2026-10-04T01:00:00.175000000Z")
    );
    assert!(messages.iter().any(|event| matches!(event.payload(), SafePayload::Message { record_kind: R::Delta, native_id: Some(id), presence: C::NonEmpty, .. } if id == "native-item")));
    assert!(
        messages
            .iter()
            .all(|event| event.turn_id() == Some(facts.turns[0].id.as_str()))
    );
    let stored = serde_json::to_string(&facts.events).unwrap();
    assert!(!stored.contains("PRIVATE"));
    assert!(!stored.contains("private.invalid"));
    assert!(facts.measurements.is_empty());
    assert!(facts.operations.is_empty());
}

#[test]
fn provenance_excludes_user_injection_inheritance_interagent_reasoning_and_compaction() {
    let root = tempfile::tempdir().unwrap();
    let mut injected = response("user", text());
    injected["payload"]["internal_chat_message_metadata_passthrough"]["content_item_kinds"] =
        json!(["agents_md.instructions"]);
    let mut inherited = response("user", text());
    inherited["metadata"] = json!({"inherited_user_message":true});
    let mut interagent = response(
        "assistant",
        json!([{"type":"encrypted_content","encrypted_content":"PRIVATE_CIPHER"}]),
    );
    interagent["payload"]["type"] = json!("agent_message");
    let mut reasoning = response("assistant", text());
    reasoning["payload"]["type"] = json!("reasoning");
    let compaction = json!({"type":"compacted","timestamp":"2026-10-04T01:00:00.180Z","payload":{"message":"PRIVATE_COMPACTION"}});
    write(
        root.path(),
        "sessions/messages.jsonl",
        &[
            meta("thread"),
            context("turn", "model", "medium"),
            response("user", text()),
            injected,
            inherited,
            interagent,
            reasoning,
            compaction,
        ],
    );
    let facts = collect(root.path());
    let origins: Vec<_> = facts
        .events
        .iter()
        .filter_map(|event| match event.payload() {
            SafePayload::Message { origin, .. } => Some(*origin),
            _ => None,
        })
        .collect();
    for expected in [
        O::UserUnclassified,
        O::InjectedContext,
        O::Inherited,
        O::InterAgent,
        O::Reasoning,
        O::Compaction,
    ] {
        assert!(origins.contains(&expected), "{expected:?}");
    }
    assert!(!origins.contains(&O::UserInput));
    assert!(!origins.contains(&O::AssistantVisible));
    assert!(facts.events.iter().any(|event| matches!(
        event.payload(),
        SafePayload::Message {
            origin: O::InterAgent,
            presence: C::Unknown,
            ..
        }
    )));
    assert!(
        !serde_json::to_string(&facts.events)
            .unwrap()
            .contains("PRIVATE")
    );
}

#[test]
fn unknown_content_and_metadata_never_become_empty_or_user_authorization() {
    let root = tempfile::tempdir().unwrap();
    let unknown = response(
        "assistant",
        json!([{"type":"future","text":"PRIVATE_FUTURE"}]),
    );
    let mut missing = response("assistant", text());
    missing["payload"]
        .as_object_mut()
        .unwrap()
        .remove("content");
    let mut bad_metadata = response("user", text());
    bad_metadata["metadata"] = json!({"inherited_user_message":"unknown"});
    let mut conflict = response("assistant", text());
    conflict["payload"]["turn_id"] = json!("different-turn");
    write(
        root.path(),
        "sessions/messages.jsonl",
        &[
            meta("thread"),
            context("turn", "model", "medium"),
            unknown,
            missing,
            bad_metadata,
            conflict,
        ],
    );
    let future = json!({"type":"response_item","timestamp":"2026-10-04T01:00:00.151Z","payload":{"type":"future_message","role":"assistant","content":[{"type":"output_text","text":"PRIVATE_FUTURE_VARIANT"}],"internal_chat_message_metadata_passthrough":{"turn_id":"turn"}}});
    let native_future = json!({"type":"event_msg","timestamp":"2026-10-04T01:00:00.152Z","payload":{"type":"item_completed","turn_id":"turn","item":{"type":"FutureMessage","content":[{"type":"Text","text":"PRIVATE_FUTURE_NATIVE"}]}}});
    let path = root.path().join("sessions/messages.jsonl");
    let mut rows = fs::read_to_string(&path).unwrap();
    rows.push_str(&format!("{future}\n{native_future}\n"));
    fs::write(path, rows).unwrap();
    let facts = collect(root.path());
    assert_eq!(
        facts
            .events
            .iter()
            .filter(|event| matches!(
                event.payload(),
                SafePayload::Message {
                    origin: O::Unknown,
                    presence: C::Unknown,
                    ..
                }
            ))
            .count(),
        2
    );
    assert_eq!(
        facts
            .events
            .iter()
            .filter(|event| matches!(
                event.payload(),
                SafePayload::Message {
                    presence: C::Unknown,
                    origin: O::AssistantVisible,
                    ..
                }
            ))
            .count(),
        2
    );
    assert!(facts.events.iter().any(|event| matches!(
        event.payload(),
        SafePayload::Message {
            origin: O::UserUnclassified,
            ..
        }
    )));
    assert!(facts.events.iter().any(|event| {
        matches!(
            event.payload(),
            SafePayload::Message {
                origin: O::AssistantVisible,
                ..
            }
        ) && event
            .gaps()
            .contains(&crate::session_events::Gap::ConflictingIdentity)
    }));
    assert!(
        !serde_json::to_string(&facts.events)
            .unwrap()
            .contains("PRIVATE")
    );
}

#[test]
fn corrupt_source_rows_are_controls_and_cannot_create_precise_first_record_delay() {
    use crate::timing::analysis::{AnalyzeInput, Budget, analyze};
    for broken in [
        "PRIVATE_INVALID_JSON\n",
        "{\"type\":\"event_msg\",\"timestamp\":\"2026-10-04T01:00:00.110Z\",\"payload\":{\"type\":42,\"message\":\"PRIVATE_INVALID_PAYLOAD\"}}\n",
    ] {
        let root = tempfile::tempdir().unwrap();
        let start = json!({"type":"event_msg","timestamp":"2026-10-04T01:00:00.100Z","payload":{"type":"task_started","turn_id":"turn"}});
        let path = write(
            root.path(),
            "sessions/messages.jsonl",
            &[meta("thread"), start],
        );
        let prefix = fs::read_to_string(&path).unwrap();
        let content = response("assistant", text());
        fs::write(&path, format!("{prefix}{broken}{content}\n")).unwrap();
        let facts = collect(root.path());
        let control = facts
            .events
            .iter()
            .find(|event| {
                event
                    .gaps()
                    .contains(&crate::session_events::Gap::SourcePartial)
            })
            .unwrap();
        assert_eq!(control.position().byte_offset, prefix.len() as u64);
        assert_eq!(control.turn_id(), None);
        assert_eq!(control.time().timestamp, None);
        assert_eq!(control.thread_id(), Some(facts.threads[0].id.as_str()));
        assert!(matches!(
            control.payload(),
            SafePayload::Message {
                origin: O::Unknown,
                presence: C::Unknown,
                record_kind: R::Unknown,
                ..
            }
        ));
        let result = analyze(AnalyzeInput {
            source: &facts.sources[0].source.id,
            thread: &facts.threads[0].id,
            turn: &facts.turns[0].id,
            events: &facts.events,
            measurements: &facts.measurements,
            budget: Budget {
                events: 100,
                measurements: 100,
                lifecycle_records: 100,
            },
        });
        assert_eq!(result.first_content_record_delay_ms, None);
        assert!(result.coverage.partial);
        assert_eq!(result.coverage.content_candidates, 1);
        assert!(result.coverage.unassigned_events > 0);
        assert!(facts.measurements.is_empty());
        assert!(facts.operations.is_empty());
        assert!(
            facts.sources[0]
                .issues
                .iter()
                .any(|issue| issue.code == "invalidRecord")
        );
        assert!(
            !serde_json::to_string(&facts.events)
                .unwrap()
                .contains("PRIVATE")
        );
        let watermark = &facts.watermarks[0];
        assert_eq!(
            watermark.committed_offset,
            fs::metadata(path).unwrap().len()
        );
    }
}

#[test]
fn source_message_styles_establish_record_delay_and_earlier_unknowns_block_it() {
    use crate::timing::analysis::{AnalyzeInput, Budget, analyze};
    let legacy = json!({"type":"event_msg","timestamp":"2026-10-04T01:00:00.150Z","payload":{"type":"agent_message","message":"PRIVATE_LEGACY"}});
    let response = response("assistant", text());
    let native = json!({"type":"event_msg","timestamp":"2026-10-04T01:00:00.150Z","payload":{"type":"item_completed","turn_id":"turn","started_at_ms":1,"completed_at_ms":2,"item":{"type":"AgentMessage","id":"native-item","content":[{"type":"Text","text":"PRIVATE_NATIVE"}]}}});
    for message in [legacy, response, native] {
        for earlier in [
            None,
            Some(
                json!({"type":"event_msg","timestamp":"2026-10-04T01:00:00.110Z","payload":{"type":"agent_message"}}),
            ),
            Some(
                json!({"type":"event_msg","payload":{"type":"agent_message","message":"PRIVATE_UNTIMED"}}),
            ),
        ] {
            let root = tempfile::tempdir().unwrap();
            let start = json!({"type":"event_msg","timestamp":"2026-10-04T01:00:00.100Z","payload":{"type":"task_started","turn_id":"turn"}});
            let mut rows = vec![meta("thread"), start];
            if let Some(earlier) = &earlier {
                rows.push(earlier.clone());
            }
            rows.push(message.clone());
            write(root.path(), "sessions/messages.jsonl", &rows);
            let facts = collect(root.path());
            let result = analyze(AnalyzeInput {
                source: &facts.sources[0].source.id,
                thread: &facts.threads[0].id,
                turn: &facts.turns[0].id,
                events: &facts.events,
                measurements: &facts.measurements,
                budget: Budget {
                    events: 100,
                    measurements: 0,
                    lifecycle_records: 0,
                },
            });
            assert_eq!(
                result.first_content_record_delay_ms,
                earlier.is_none().then_some(50)
            );
            assert!(
                !serde_json::to_string(&facts.events)
                    .unwrap()
                    .contains("PRIVATE")
            );
        }
    }
}
