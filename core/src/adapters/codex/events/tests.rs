//! Damaged nested records retain source boundaries without invented accounting.
use super::*;
use crate::session_events::{ContentPresence, Gap, MessageOrigin, Payload as SafePayload};
use crate::timing::analysis::{Analysis, AnalyzeInput, Budget, analyze};
use serde_json::{Value, json};
use std::io::Write;

fn row(kind: &str, at: u8, payload: Value) -> Value {
    json!({"type":kind,"timestamp":format!("2026-10-05T00:00:{at:02}Z"),"payload":payload})
}
fn start() -> Vec<Value> {
    vec![
        row("session_meta", 0, json!({"id":"thread"})),
        row(
            "event_msg",
            0,
            json!({"type":"task_started","turn_id":"turn"}),
        ),
    ]
}
fn message(at: u8, explicit: bool) -> Value {
    let mut payload = json!({"type":"agent_message","message":"SYNTHETIC_PRIVATE_BODY"});
    if explicit {
        payload["turn_id"] = json!("turn");
    }
    row("event_msg", at, payload)
}
fn broken(at: u8, explicit: bool) -> Value {
    let mut payload = json!({"type":"item_completed","item":{"type":"AgentMessage","role":17,"content":[{"type":"Text","text":"SYNTHETIC_PRIVATE_NESTED"}]}});
    if explicit {
        payload["turn_id"] = json!("turn");
    }
    row("event_msg", at, payload)
}
fn complete() -> Value {
    row(
        "event_msg",
        3,
        json!({"type":"task_complete","turn_id":"turn","duration_ms":3000,"time_to_first_token_ms":17}),
    )
}
fn write(root: &Path, rows: &[Value]) -> PathBuf {
    fs::create_dir_all(root.join("sessions")).unwrap();
    let path = root.join("sessions/events.jsonl");
    fs::write(
        &path,
        rows.iter()
            .map(|row| row.to_string() + "\n")
            .collect::<String>(),
    )
    .unwrap();
    path
}
fn source(root: &Path) -> SourceInstance {
    CodexAdapter
        .discover(&DiscoveryRequest {
            roots: vec![root.into()],
        })
        .sources
        .remove(0)
}
fn collect(root: &Path) -> Collected {
    crate::adapters::collect(
        &DiscoveryRequest {
            roots: vec![root.into()],
        },
        &RunContext::default(),
    )
}
fn analysis(facts: &Collected) -> Analysis {
    let thread = facts
        .threads
        .iter()
        .find(|thread| thread.upstream_id == "thread")
        .unwrap();
    let turn = stable_id(&[&thread.id, "turn", "turn"]);
    analyze(AnalyzeInput {
        source: &thread.source_instance_id,
        thread: &thread.id,
        turn: &turn,
        events: &facts.events,
        measurements: &facts.measurements,
        budget: Budget {
            events: 100,
            measurements: 100,
            lifecycle_records: 100,
        },
    })
}
fn gap(facts: &Collected) -> &crate::session_events::Event {
    facts
        .events
        .iter()
        .find(|event| event.gaps().contains(&Gap::SourcePartial))
        .unwrap()
}

#[test]
fn malformed_nested_item_is_a_scoped_safe_boundary_not_a_precise_first_record() {
    let root = tempfile::tempdir().unwrap();
    let prefix = start();
    let offset = prefix
        .iter()
        .map(|row| row.to_string().len() + 1)
        .sum::<usize>();
    let rows = [prefix, vec![broken(1, true), message(2, true), complete()]].concat();
    let path = write(root.path(), &rows);
    let facts = collect(root.path());
    let boundary = gap(&facts);
    let thread = &facts.threads[0].id;
    let turn = stable_id(&[thread, "turn", "turn"]);
    assert_eq!(boundary.position().byte_offset, offset as u64);
    assert_eq!(boundary.position().ordinal, 2);
    assert_eq!(boundary.thread_id(), Some(thread.as_str()));
    assert_eq!(boundary.turn_id(), Some(turn.as_str()));
    assert_eq!(
        boundary.time().timestamp.as_deref(),
        Some("2026-10-05T00:00:01.000000000Z")
    );
    assert!(matches!(
        boundary.payload(),
        SafePayload::Message {
            origin: MessageOrigin::Unknown,
            presence: ContentPresence::Unknown,
            ..
        }
    ));
    let result = analysis(&facts);
    assert_eq!(result.first_content_record_delay_ms, None);
    assert_eq!(result.native_wall_clock_ms, Some(3000));
    assert_eq!(result.native_ttft_ms, Some(17));
    assert_eq!(result.coverage.unknown_content_records, 1);
    assert!(result.coverage.partial);
    assert!(facts.operations.is_empty() && facts.measurements.is_empty());
    assert!(
        facts.sources[0]
            .issues
            .iter()
            .any(|issue| issue.code == "invalidOperation")
    );
    assert!(
        !serde_json::to_string(&facts.events)
            .unwrap()
            .contains("SYNTHETIC_PRIVATE")
    );
    assert_eq!(
        facts.watermarks[0].committed_offset,
        fs::metadata(path).unwrap().len()
    );
}

#[test]
fn current_implicit_nested_damage_breaks_subsequent_implicit_attribution() {
    let root = tempfile::tempdir().unwrap();
    write(
        root.path(),
        &[
            start(),
            vec![broken(1, false), message(2, false), complete()],
        ]
        .concat(),
    );
    let facts = collect(root.path());
    assert!(gap(&facts).turn_id().is_some());
    let later = facts
        .events
        .iter()
        .find(|event| {
            matches!(
                event.payload(),
                SafePayload::Message {
                    origin: MessageOrigin::AssistantVisible,
                    ..
                }
            )
        })
        .unwrap();
    assert_eq!(later.turn_id(), None);
    assert_eq!(analysis(&facts).first_content_record_delay_ms, None);
}

#[test]
fn explicitly_unrelated_nested_damage_does_not_break_the_current_turn() {
    for other_thread in [false, true] {
        let root = tempfile::tempdir().unwrap();
        let mut damaged = broken(1, true);
        damaged["payload"]["turn_id"] = json!("other-turn");
        if other_thread {
            damaged["payload"]["thread_id"] = json!("other-thread");
        }
        write(
            root.path(),
            &[start(), vec![damaged, message(2, false), complete()]].concat(),
        );
        let facts = collect(root.path());
        let thread = facts
            .threads
            .iter()
            .find(|thread| thread.upstream_id == "thread")
            .unwrap();
        let turn = stable_id(&[&thread.id, "turn", "turn"]);
        assert_ne!(gap(&facts).turn_id(), Some(turn.as_str()));
        assert_eq!(analysis(&facts).first_content_record_delay_ms, Some(2000));
        assert!(facts.operations.is_empty() && facts.measurements.is_empty());
    }
}

#[test]
fn unknown_nested_presence_before_first_content_blocks_it_but_later_unknowns_do_not() {
    for shape in 0..4 {
        for earlier in [false, true] {
            let root = tempfile::tempdir().unwrap();
            let at = if earlier { 1 } else { 2 };
            let unknown = if shape == 0 {
                broken(at, true)
            } else {
                let mut item = json!({"type":"FutureMessage"});
                match shape {
                    1 => {
                        item["content"] =
                            json!([{"type":"FutureText","text":"SYNTHETIC_PRIVATE_FUTURE"}])
                    }
                    2 => {
                        item["role"] = json!("assistant");
                        item["message"] = json!("SYNTHETIC_PRIVATE_FUTURE");
                    }
                    _ => {}
                }
                row(
                    "event_msg",
                    at,
                    json!({"type":"item_completed","turn_id":"turn","item":item}),
                )
            };
            let known = message(if earlier { 2 } else { 1 }, true);
            let records = if earlier {
                vec![unknown, known]
            } else {
                vec![known, unknown]
            };
            write(root.path(), &[start(), records, vec![complete()]].concat());
            let facts = collect(root.path());
            assert_eq!(
                analysis(&facts).first_content_record_delay_ms,
                (!earlier).then_some(1000)
            );
            assert!(
                !serde_json::to_string(&facts.events)
                    .unwrap()
                    .contains("SYNTHETIC_PRIVATE")
            );
        }
    }
}

#[test]
fn known_non_content_nested_items_do_not_become_unknown_content_candidates() {
    for kind in [
        "CommandExecution",
        "FileChange",
        "McpToolCall",
        "FunctionCallOutput",
    ] {
        let root = tempfile::tempdir().unwrap();
        let item = row(
            "event_msg",
            1,
            json!({"type":"item_completed","turn_id":"turn","item":{"type":kind,"id":"known-item"}}),
        );
        write(
            root.path(),
            &[start(), vec![item, message(2, true), complete()]].concat(),
        );
        let facts = collect(root.path());
        let result = analysis(&facts);
        assert_eq!(result.first_content_record_delay_ms, Some(2000));
        assert_eq!(result.coverage.unknown_content_records, 0);
    }
}

#[test]
fn incomplete_nested_tail_is_not_committed_and_restart_replays_one_safe_boundary() {
    let root = tempfile::tempdir().unwrap();
    let index = tempfile::tempdir().unwrap();
    let path = write(root.path(), &start());
    let source = source(root.path());
    let mut db = crate::live_index::open(&index.path().join("index.sqlite")).unwrap();
    let sync = |db: &mut rusqlite::Connection, verify| {
        let tx = db.transaction().unwrap();
        let facts = incremental::sync(&tx, &source, verify).unwrap().unwrap();
        tx.commit().unwrap();
        facts
    };
    let first = sync(&mut db, false);
    let prefix = fs::metadata(&path).unwrap().len();
    let mut damaged = broken(1, true);
    damaged["payload"]["item"]["content"][0]["text"] =
        json!("SYNTHETIC_PRIVATE_LARGE".repeat(50_000));
    fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .unwrap()
        .write_all(damaged.to_string().as_bytes())
        .unwrap();
    let tail = sync(&mut db, false);
    assert_eq!(
        serde_json::to_value(&tail.events).unwrap(),
        serde_json::to_value(&first.events).unwrap()
    );
    assert_eq!(tail.watermarks[0].committed_offset, prefix);
    drop(db);
    db = crate::live_index::open(&index.path().join("index.sqlite")).unwrap();
    fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .unwrap()
        .write_all(format!("\n{}\n{}\n", message(2, true), complete()).as_bytes())
        .unwrap();
    let appended = sync(&mut db, false);
    assert_eq!(gap(&appended).position().byte_offset, prefix);
    assert_eq!(
        appended
            .events
            .iter()
            .filter(|event| event.gaps().contains(&Gap::SourcePartial))
            .count(),
        1
    );
    assert_eq!(analysis(&appended).first_content_record_delay_ms, None);
    let safe = serde_json::to_string(&appended.events).unwrap();
    assert!(!safe.contains("SYNTHETIC_PRIVATE"));
    // Safe facts stay small; bounded source locators scale with text blocks.
    let mut facts_only = serde_json::to_value(&appended.events).unwrap();
    for event in facts_only.as_array_mut().unwrap() {
        event.as_object_mut().unwrap().remove("originalContent");
    }
    assert!(serde_json::to_string(&facts_only).unwrap().len() < 16 * 1024);
    let verified = sync(&mut db, true);
    assert_eq!(
        serde_json::to_value(&verified.events).unwrap(),
        serde_json::to_value(&appended.events).unwrap()
    );
    assert!(verified.measurements.is_empty() && verified.operations.is_empty());
}

#[test]
fn prior_message_mapping_cannot_resume_cached_append_or_verified_parser_state() {
    assert_eq!(incremental::MESSAGE_OBSERVATION_VERSION, 4);
    for mode in [0, 1, 2] {
        let root = tempfile::tempdir().unwrap();
        let index = tempfile::tempdir().unwrap();
        let path = write(root.path(), &start());
        let source = source(root.path());
        let mut db = crate::live_index::open(&index.path().join("index.sqlite")).unwrap();
        let mut cache = incremental::Cache::default();
        let scope = format!("parser:{}:{VERSION}:1", source.id);
        {
            let tx = db.transaction().unwrap();
            incremental::sync_cached(&tx, &source, false, &mut cache).unwrap();
            tx.commit().unwrap();
        }
        let original = crate::live_index::load_map(&db, &scope).unwrap();
        let facts = crate::live_index::load_map(&db, &format!("{scope}:facts")).unwrap();
        if mode == 1 {
            fs::OpenOptions::new()
                .append(true)
                .open(&path)
                .unwrap()
                .write_all(format!("{}\n", message(2, true)).as_bytes())
                .unwrap();
        }
        for version in [None, Some(1), Some(3), Some(99)] {
            let mut old = original.clone();
            old.remove("messageObservationVersion");
            if let Some(version) = version {
                old.insert("messageObservationVersion".into(), json!(version));
            }
            old.insert("checkpoints".into(), json!("future parser shape"));
            crate::live_index::save_map(&db, &scope, &old).unwrap();
            let error = incremental::sync_cached(&db, &source, mode == 2, &mut cache)
                .err()
                .unwrap();
            assert_eq!(
                crate::live_index::failure_code(&error),
                "UNSUPPORTED_VERSION"
            );
            assert_eq!(crate::live_index::load_map(&db, &scope).unwrap(), old);
            assert_eq!(
                crate::live_index::load_map(&db, &format!("{scope}:facts")).unwrap(),
                facts
            );
        }
    }
}
