use super::*;
#[test]
fn live_checkpoint_restart_tail_retraction_and_full_replay_agree() {
    use std::io::Write;
    let root = tempfile::tempdir().unwrap();
    let index = tempfile::tempdir().unwrap();
    let path = write(
        root.path(),
        "sessions/live.jsonl",
        &[
            meta("live"),
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
    let sync = |verify| {
        let mut db = crate::live_index::open(&index.path().join("index.sqlite")).unwrap();
        let tx = db.transaction().unwrap();
        let result = incremental::sync(&tx, &source, verify).unwrap();
        tx.commit().unwrap();
        result
    };
    assert_eq!(sync(false).unwrap().measurements[0].tokens.total, Some(110));
    assert!(sync(false).is_none(), "unchanged source must not reparse");
    let row = direct("live", "u", "r", "2026-09-29T00:00:01Z", 100, 60, 10).to_string();
    let mut file = fs::OpenOptions::new().append(true).open(&path).unwrap();
    file.write_all(row.as_bytes()).unwrap();
    let partial = sync(false).unwrap();
    assert_eq!(partial.measurements.len(), 1);
    assert_eq!(partial.sources[0].issues[0].code, "incompleteTail");
    file.write_all(b"\n").unwrap();
    let live = sync(false).unwrap();
    let full = collect(root.path());
    assert_eq!(
        serde_json::to_value(&live.measurements).unwrap(),
        serde_json::to_value(&full.measurements).unwrap()
    );
    assert_eq!(
        live.measurements.len(),
        1,
        "late direct record replaces legacy delta"
    );
    assert_eq!(live.sources[0].bytes_read, row.len() as u64 + 1);
    assert!(
        !live.sources[0]
            .issues
            .iter()
            .any(|i| i.code == "incompleteTail")
    );
    // Parser context survives another process, including cumulative counters and model.
    for i in 2..=6 {
        let row = direct(
            "live",
            "u",
            &format!("r{i}"),
            "2026-09-29T00:00:02Z",
            100,
            60,
            10,
        )
        .to_string()
            + "\n";
        file.write_all(row.as_bytes()).unwrap();
        let live = sync(false).unwrap();
        assert_eq!(live.measurements.len(), i);
        assert_eq!(live.sources[0].bytes_read, row.len() as u64);
        assert_eq!(
            live.measurements
                .iter()
                .map(|m| m.tokens.total.unwrap())
                .sum::<u64>(),
            i as u64 * 110
        );
        assert!(
            live.measurements
                .iter()
                .all(|m| m.model.raw.as_deref() == Some("gpt-5.4"))
        );
    }
    let verified = sync(true).unwrap();
    assert_eq!(
        serde_json::to_value(verified.measurements).unwrap(),
        serde_json::to_value(collect(root.path()).measurements).unwrap()
    );
}

#[test]
fn live_transaction_rollback_truncate_missing_and_body_privacy() {
    let root = tempfile::tempdir().unwrap();
    let index = tempfile::tempdir().unwrap();
    let source = CodexAdapter
        .discover(&DiscoveryRequest {
            roots: vec![root.path().into()],
        })
        .sources
        .remove(0);
    let rows = [
        meta("t"),
        context("u", "gpt-5.4", "high"),
        direct("t", "u", "r", "2026-09-29T00:00:01Z", 100, 60, 10),
        serde_json::json!({"type":"response_item","payload":{"type":"message","content":[{"type":"input_text","text":"PRIVATE_SYNTHETIC_BODY_MUST_NOT_PERSIST"}]}}),
    ];
    let path = write(root.path(), "sessions/a.jsonl", &rows);
    let mut db = crate::live_index::open(&index.path().join("index.sqlite")).unwrap();
    {
        let tx = db.transaction().unwrap();
        incremental::sync(&tx, &source, false).unwrap(); /* Simulated crash: no commit. */
    }
    assert_eq!(
        incremental::sync(&db, &source, false)
            .unwrap()
            .unwrap()
            .measurements
            .len(),
        1
    );
    let text: String = db
        .prepare("SELECT group_concat(json(payload)) FROM entries")
        .unwrap()
        .query_row([], |r| r.get(0))
        .unwrap();
    assert!(!text.contains("PRIVATE_SYNTHETIC_BODY_MUST_NOT_PERSIST"));
    write(
        root.path(),
        "sessions/a.jsonl",
        &[
            meta("t"),
            context("u", "gpt-5.4", "high"),
            direct("t", "u", "new", "2026-09-29T00:00:01Z", 200, 60, 20),
        ],
    );
    let replaced = incremental::sync(&db, &source, false).unwrap().unwrap();
    assert_eq!(replaced.measurements.len(), 1);
    assert_eq!(replaced.measurements[0].tokens.total, Some(220));
    fs::remove_file(path).unwrap();
    let missing = incremental::sync(&db, &source, false).unwrap().unwrap();
    assert_eq!(missing.measurements[0].tokens.total, Some(220));
    assert!(missing.issues.iter().any(|i| i.code == "sourceMissing"));
}

#[test]
fn missing_source_rebuilds_facts_from_events_without_derived_cache() {
    use rusqlite::params;

    let root = tempfile::tempdir().unwrap();
    let index = tempfile::tempdir().unwrap();
    let path = write(
        root.path(),
        "sessions/events-only.jsonl",
        &[
            meta("events-only"),
            context("turn-1", "gpt-5.4", "high"),
            direct(
                "events-only",
                "turn-1",
                "response-1",
                "2026-09-29T00:00:01Z",
                100,
                60,
                10,
            ),
            json!({"type":"response_item","timestamp":"2026-09-29T00:00:02Z","payload":{"type":"function_call","call_id":"call-1","name":"read_file","arguments":"{\"path\":\"/synthetic/project/AGENTS.md\"}"}}),
            json!({"type":"response_item","timestamp":"2026-09-29T00:00:03Z","payload":{"type":"function_call_output","call_id":"call-1","output":"synthetic result"}}),
        ],
    );
    let source = CodexAdapter
        .discover(&DiscoveryRequest {
            roots: vec![root.path().into()],
        })
        .sources
        .remove(0);
    let db = crate::live_index::open(&index.path().join("index.sqlite")).unwrap();
    let initial = incremental::sync(&db, &source, false).unwrap().unwrap();
    assert_eq!(initial.threads.len(), 1);
    assert_eq!(initial.turns.len(), 1);
    assert_eq!(initial.measurements.len(), 1);
    assert_eq!(initial.operations.len(), 1);

    let fact_scope = format!("parser:{}:%:1:facts", source.id);
    let event_count: i64 = db
        .query_row(
            "SELECT SUM(e.count) FROM event_blocks e JOIN buckets b ON b.id=e.bucket WHERE b.scope LIKE ?1 AND b.field='events'",
            [&fact_scope],
            |row| row.get(0),
        )
        .unwrap();
    assert!(event_count > 0, "the authoritative event rows must remain");
    let checkpoint_count: i64 = db
        .query_row(
            "SELECT COUNT(*) FROM entries e JOIN buckets b ON b.id=e.bucket WHERE b.scope LIKE ?1 AND b.field='checkpoints'",
            [format!("parser:{}:%:1", source.id)],
            |row| row.get(0),
        )
        .unwrap();
    assert_eq!(checkpoint_count, 1, "the parser checkpoint must remain");

    db.execute(
        "DELETE FROM entries WHERE bucket IN (SELECT id FROM buckets WHERE scope LIKE ?1 AND field <> 'events')",
        params![fact_scope],
    )
    .unwrap();
    fs::remove_file(path).unwrap();

    let restored = incremental::sync(&db, &source, false).unwrap().unwrap();
    assert_eq!(restored.threads.len(), 1);
    assert_eq!(restored.turns.len(), 1);
    assert_eq!(restored.measurements.len(), 1);
    assert_eq!(restored.operations.len(), 1);
    assert_eq!(restored.measurements[0].tokens.total, Some(110));
    assert_eq!(
        serde_json::to_value(&restored.threads).unwrap(),
        serde_json::to_value(&initial.threads).unwrap()
    );
    assert_eq!(
        serde_json::to_value(&restored.turns).unwrap(),
        serde_json::to_value(&initial.turns).unwrap()
    );
    assert_eq!(
        serde_json::to_value(&restored.measurements).unwrap(),
        serde_json::to_value(&initial.measurements).unwrap()
    );
    assert_eq!(
        serde_json::to_value(&restored.operations).unwrap(),
        serde_json::to_value(&initial.operations).unwrap()
    );
    assert!(
        restored
            .issues
            .iter()
            .any(|issue| issue.code == "sourceMissing")
    );
    assert!(restored.sources.iter().any(|source| {
        source
            .issues
            .iter()
            .any(|issue| issue.code == "sourceMissing")
    }));
    for field in ["measurements", "operations"] {
        let count: i64 = db
            .query_row(
                "SELECT COUNT(*) FROM entries e JOIN buckets b ON b.id=e.bucket WHERE b.scope LIKE ?1 AND b.field=?2",
                params![fact_scope, field],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(
            count, 0,
            "the parser must not duplicate canonical {field} after replay"
        );
    }
}

#[test]
fn persisted_events_reject_other_source_without_projection_fallback() {
    let root = tempfile::tempdir().unwrap();
    let index = tempfile::tempdir().unwrap();
    write(
        root.path(),
        "sessions/source.jsonl",
        &[
            meta("source"),
            context("u", "gpt-5.4", "high"),
            direct("source", "u", "r", "2026-09-29T00:00:01Z", 100, 60, 10),
        ],
    );
    let source = CodexAdapter
        .discover(&DiscoveryRequest {
            roots: vec![root.path().into()],
        })
        .sources
        .remove(0);
    let db = crate::live_index::open(&index.path().join("index.sqlite")).unwrap();
    incremental::sync(&db, &source, false).unwrap().unwrap();
    let scope = format!("parser:{}:{}:1:facts", source.id, VERSION);
    let foreign = crate::session_events::Event::new(
        crate::session_events::Position {
            source_instance_id: "other-source".into(),
            file_id: "foreign-file".into(),
            generation: "foreign-generation".into(),
            byte_offset: 0,
            ordinal: 0,
        },
        None,
        None,
        crate::session_events::Time {
            timestamp: None,
            precision: crate::session_events::Precision::Unknown,
        },
        vec![],
        crate::session_events::Payload::ContextWindow {
            model: None,
            tokens: 1,
        },
    )
    .unwrap();
    crate::live_index::save_events(
        &db,
        &scope,
        &[Arc::new(foreign)],
        false,
        &std::sync::atomic::AtomicBool::new(false),
    )
    .unwrap();
    let error = incremental::sync(&db, &source, true)
        .expect_err("foreign events must fail even with valid cached projections");
    assert!(
        error
            .to_string()
            .contains("stored event source identity mismatch")
    );
}

#[test]
fn rebuild_orders_missing_and_present_file_events_together() {
    let root = tempfile::tempdir().unwrap();
    let index = tempfile::tempdir().unwrap();
    write(
        root.path(),
        "sessions/a.jsonl",
        &[
            meta("shared"),
            context("u", "gpt-5.4", "high"),
            json!({"type":"event_msg","timestamp":"2026-09-29T00:00:01Z","payload":{"type":"task_complete","turn_id":"u"}}),
        ],
    );
    let missing = write(
        root.path(),
        "sessions/b.jsonl",
        &[
            meta("shared"),
            context("u", "gpt-5.4", "high"),
            json!({"type":"event_msg","timestamp":"2026-09-29T00:00:02Z","payload":{"type":"turn_aborted","turn_id":"u"}}),
        ],
    );
    let source = CodexAdapter
        .discover(&DiscoveryRequest {
            roots: vec![root.path().into()],
        })
        .sources
        .remove(0);
    let db = crate::live_index::open(&index.path().join("index.sqlite")).unwrap();
    let initial = incremental::sync(&db, &source, false).unwrap().unwrap();
    assert_eq!(initial.turns.len(), 1);
    assert_eq!(initial.turns[0].status, "interrupted");
    fs::remove_file(missing).unwrap();
    let verified = incremental::sync(&db, &source, true).unwrap().unwrap();
    assert_eq!(
        serde_json::to_value(&verified.turns).unwrap(),
        serde_json::to_value(&initial.turns).unwrap()
    );
    assert_eq!(
        verified.sources[0]
            .issues
            .iter()
            .filter(|issue| issue.code == "sourceMissing")
            .count(),
        1
    );
}

#[test]
fn shared_mcp_matching_survives_checkpoint_restart_append_and_verified_replay() {
    use std::io::Write;
    let root = tempfile::tempdir().unwrap();
    let index = tempfile::tempdir().unwrap();
    let row = |phase: &str, args: Value| json!({"type":"event_msg","timestamp":"2026-10-03T00:00:02Z","payload":{"type":phase,"call_id":"call","turn_id":"u","invocation":{"server":"synthetic","tool":"search","arguments":args},"result":{"Ok":{"content":[]}}}});
    let path = write(
        root.path(),
        "sessions/mcp.jsonl",
        &[
            meta("t"),
            context("u", "model", "low"),
            row(
                "mcp_tool_call_begin",
                json!({"body":"PRIVATE_MCP","n":9007199254740993_u64}),
            ),
        ],
    );
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
        result
    };
    let before = sync(false).unwrap();
    let first = before.operations[0].matching.clone().unwrap();
    assert!(first.request_fingerprint.is_some());
    assert!(sync(false).is_none());
    let mut file = fs::OpenOptions::new().append(true).open(path).unwrap();
    writeln!(
        file,
        "{}",
        row(
            "mcp_tool_call_end",
            json!({"n":9007199254740993_u64,"body":"PRIVATE_MCP"})
        )
    )
    .unwrap();
    let after = sync(false).unwrap();
    assert_eq!(after.operations.len(), 1);
    assert_eq!(after.operations[0].matching.as_ref(), Some(&first));
    assert_eq!(after.operations[0].status.as_ref(), "completed");
    let verified = sync(true).unwrap();
    let full = collect(root.path());
    assert_eq!(
        serde_json::to_value(&verified.operations).unwrap(),
        serde_json::to_value(&full.operations).unwrap()
    );
    assert!(
        !serde_json::to_string(&verified)
            .unwrap()
            .contains("PRIVATE_MCP")
    );
}

#[test]
fn callable_matching_survives_append_restart_and_full_replay_without_private_arguments() {
    use std::io::Write;
    let root = tempfile::tempdir().unwrap();
    let index = tempfile::tempdir().unwrap();
    let row = json!({"type":"response_item","timestamp":"2026-10-03T00:00:01Z","payload":{"type":"function_call","call_id":"call","name":"mcp__docs__search","arguments":"{\"private\":\"PRIVATE_CALLABLE\"}"}});
    let path = write(
        root.path(),
        "sessions/callable.jsonl",
        &[meta("t"), context("u", "model", "low"), row],
    );
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
        result
    };
    let first = sync(false).unwrap().operations[0].matching.clone().unwrap();
    assert!(first.function_request_fingerprint.is_some());
    let output = json!({"type":"response_item","timestamp":"2026-10-03T00:00:02Z","payload":{"type":"function_call_output","call_id":"call","output":{"isError":true}}});
    writeln!(
        std::fs::OpenOptions::new().append(true).open(path).unwrap(),
        "{output}"
    )
    .unwrap();
    let after = sync(false).unwrap();
    assert_eq!(after.operations[0].matching.as_ref(), Some(&first));
    assert_eq!(after.operations[0].status.as_ref(), "failed");
    let verified = sync(true).unwrap();
    assert_eq!(
        serde_json::to_value(&verified.operations).unwrap(),
        serde_json::to_value(&collect(root.path()).operations).unwrap()
    );
    assert!(
        !serde_json::to_string(&verified)
            .unwrap()
            .contains("PRIVATE_CALLABLE")
    );
}
