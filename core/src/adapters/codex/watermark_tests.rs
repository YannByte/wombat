use super::*;
use std::io::Write;
fn setup() -> (
    tempfile::TempDir,
    tempfile::TempDir,
    PathBuf,
    SourceInstance,
    rusqlite::Connection,
) {
    let root = tempfile::tempdir().unwrap();
    let index = tempfile::tempdir().unwrap();
    fs::create_dir(root.path().join("sessions")).unwrap();
    let path = root.path().join("sessions/source.jsonl");
    fs::write(&path, row("initial") + "\n").unwrap();
    let source = CodexAdapter
        .discover(&DiscoveryRequest {
            roots: vec![root.path().into()],
        })
        .sources
        .remove(0);
    let db = crate::live_index::open(&index.path().join("index.sqlite")).unwrap();
    (root, index, path, source, db)
}
fn row(id: &str) -> String {
    serde_json::json!({"type":"session_meta","timestamp":"2026-10-05T00:00:00Z","payload":{"id":id}}).to_string()
}
fn sync(db: &mut rusqlite::Connection, source: &SourceInstance) -> Option<Collected> {
    let tx = db.transaction().unwrap();
    let value = incremental::sync(&tx, source, false).unwrap();
    tx.commit().unwrap();
    value
}
#[test]
fn watermark_append_tail_missing_return_and_restart_track_same_position() {
    let (_root, index, path, source, mut db) = setup();
    let initial = sync(&mut db, &source).unwrap();
    let first = initial.watermarks[0].clone();
    assert_eq!(first.state, WatermarkState::Complete);
    assert_eq!(first.committed_offset, fs::metadata(&path).unwrap().len());
    assert_eq!(first.file_id, initial.events[0].position().file_id.as_ref());
    assert_eq!(
        first.generation.as_deref(),
        Some(initial.events[0].position().generation.as_ref())
    );
    assert!(sync(&mut db, &source).is_none());
    let mut file = fs::OpenOptions::new().append(true).open(&path).unwrap();
    file.write_all(row("tail").as_bytes()).unwrap();
    let half = sync(&mut db, &source).unwrap().watermarks.remove(0);
    assert_eq!(half.state, WatermarkState::Partial);
    assert_eq!(half.committed_offset, first.committed_offset);
    assert!(half.observed_bytes.unwrap() > half.committed_offset);
    drop(db);
    db = crate::live_index::open(&index.path().join("index.sqlite")).unwrap();
    assert!(sync(&mut db, &source).is_none());
    file.write_all(b"\n").unwrap();
    let full = sync(&mut db, &source).unwrap().watermarks.remove(0);
    assert_eq!(full.state, WatermarkState::Complete);
    assert_eq!(full.generation, first.generation);
    assert!(full.committed_offset > first.committed_offset);
    let bytes = fs::read(&path).unwrap();
    fs::remove_file(&path).unwrap();
    let missing = sync(&mut db, &source).unwrap().watermarks.remove(0);
    assert_eq!(missing.state, WatermarkState::Missing);
    assert_eq!(missing.committed_offset, full.committed_offset);
    assert_eq!(missing.generation, full.generation);
    assert_eq!(missing.observed_bytes, None);
    assert!(sync(&mut db, &source).is_none());
    fs::write(&path, bytes).unwrap();
    assert_eq!(
        sync(&mut db, &source).unwrap().watermarks[0].state,
        WatermarkState::Complete
    );
}
#[test]
fn replacement_rollback_preserves_committed_watermark_until_transaction_commit() {
    let (_root, _index, path, source, mut db) = setup();
    let first = sync(&mut db, &source).unwrap().watermarks.remove(0);
    let scope = format!("parser:{}:{VERSION}:1", source.id);
    let old = crate::live_index::load_map(&db, &scope).unwrap();
    fs::write(&path, row("replacement") + "\n").unwrap();
    {
        let tx = db.transaction().unwrap();
        let attempted = incremental::sync(&tx, &source, false).unwrap().unwrap();
        assert_ne!(attempted.watermarks[0].generation, first.generation);
    }
    assert_eq!(crate::live_index::load_map(&db, &scope).unwrap(), old);
    let committed = sync(&mut db, &source).unwrap().watermarks.remove(0);
    assert_ne!(committed.generation, first.generation);
    assert_eq!(
        committed.committed_offset,
        fs::metadata(path).unwrap().len()
    );
}
#[test]
fn direct_reader_retains_valid_unclosed_fact_but_only_covers_newlines() {
    let (root, _index, path, source, _db) = setup();
    let complete = fs::metadata(&path).unwrap().len();
    fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .unwrap()
        .write_all(row("unclosed").as_bytes())
        .unwrap();
    let mut facts = Collected::default();
    let report = CodexAdapter.collect(&source, &ReadPlan, &RunContext::default(), &mut facts);
    assert_eq!(report.status, "partial");
    assert_eq!(
        report
            .issues
            .iter()
            .filter(|issue| issue.code == "incompleteTail")
            .count(),
        1
    );
    assert_eq!(facts.threads.len(), 2);
    assert_eq!(facts.watermarks[0].committed_offset, complete);
    assert_eq!(facts.watermarks[0].state, WatermarkState::Partial);
    assert!(
        facts.watermarks[0]
            .issue_codes
            .contains(&WatermarkIssue::IncompleteTail)
    );
    fs::write(path, b"").unwrap();
    let mut empty = Collected::default();
    CodexAdapter.collect(&source, &ReadPlan, &RunContext::default(), &mut empty);
    assert_eq!(empty.watermarks[0].generation, None);
    assert_eq!(empty.watermarks[0].state, WatermarkState::Complete);
    drop(root);
}
#[test]
fn old_or_unknown_parser_watermark_header_is_rejected_without_mutation() {
    let (_root, _index, _path, source, mut db) = setup();
    sync(&mut db, &source).unwrap();
    let scope = format!("parser:{}:{VERSION}:1", source.id);
    for header in [None, Some(serde_json::json!(2))] {
        let mut old = crate::live_index::load_map(&db, &scope).unwrap();
        old.remove("watermarkVersion");
        if let Some(header) = header {
            old.insert("watermarkVersion".into(), header);
        }
        crate::live_index::save_map(&db, &scope, &old).unwrap();
        let before = crate::live_index::load_map(&db, &scope).unwrap();
        {
            let tx = db.transaction().unwrap();
            assert!(incremental::sync(&tx, &source, false).is_err());
        }
        assert_eq!(crate::live_index::load_map(&db, &scope).unwrap(), before);
    }
}

#[test]
fn failed_reader_does_not_advance_a_committed_cursor() {
    let (_root, _index, path, source, _db) = setup();
    fs::remove_file(&path).unwrap();
    let mut checkpoint = incremental::Checkpoint::default();
    checkpoint.offset = 40;
    checkpoint.state.event_generation = Some("committed-generation".into());
    let mut facts = Facts::default();
    let descriptor = CodexAdapter.descriptor();
    let mut report = SourceReport {
        source: source.clone(),
        adapter_version: descriptor.adapter_version,
        source_versions: vec![],
        capabilities: descriptor.capabilities,
        status: "complete".into(),
        files_read: 0,
        bytes_read: 0,
        issues: vec![],
    };
    read_file_from(
        &path,
        &source,
        &RunContext::default(),
        &mut facts,
        &mut report,
        Some(&mut checkpoint),
        None,
    );
    assert_eq!(checkpoint.offset, 40);
    assert_eq!(
        checkpoint.state.event_generation.as_deref(),
        Some("committed-generation")
    );
    let watermark = facts.watermarks.values().next().unwrap();
    assert_eq!(watermark.state, WatermarkState::Failed);
    assert_eq!(watermark.committed_offset, 40);
    assert_eq!(
        watermark.generation.as_deref(),
        Some("committed-generation")
    );
    assert_eq!(watermark.observed_bytes, None);
    assert_eq!(
        watermark.issue_codes,
        vec![WatermarkIssue::SourceUnreadable]
    );
}

#[test]
fn established_generation_cannot_be_erased_in_watermark_or_reset_parser_data() {
    let (_root, _index, path, source, mut db) = setup();
    sync(&mut db, &source).unwrap();
    let scope = format!("parser:{}:{VERSION}:1", source.id);
    let mut stored = crate::live_index::load_map(&db, &scope).unwrap();
    let path = dunce::canonicalize(path).unwrap();
    let path = path.to_str().unwrap();
    assert!(stored["checkpoints"][path]["state"]["event_generation"].is_string());
    stored["checkpoints"][path]["watermark"]["generation"] = serde_json::Value::Null;
    crate::live_index::save_map(&db, &scope, &stored).unwrap();
    {
        let tx = db.transaction().unwrap();
        assert!(incremental::sync(&tx, &source, false).is_err());
    }
    assert_eq!(crate::live_index::load_map(&db, &scope).unwrap(), stored);
}

#[test]
fn empty_replacement_has_unknown_generation_until_first_valid_record() {
    let (_root, _index, path, source, mut db) = setup();
    let first = sync(&mut db, &source).unwrap().watermarks.remove(0);
    fs::write(&path, b"").unwrap();
    let empty = sync(&mut db, &source).unwrap().watermarks.remove(0);
    assert_eq!(empty.generation, None);
    assert_eq!(empty.committed_offset, 0);
    assert_eq!(empty.state, WatermarkState::Complete);
    assert!(sync(&mut db, &source).is_none());
    fs::write(&path, row("initial") + "\n").unwrap();
    let observed = sync(&mut db, &source).unwrap().watermarks.remove(0);
    assert!(observed.generation.is_some());
    assert_ne!(observed.generation, first.generation);
}

#[test]
fn missing_or_future_message_mapping_refuses_append_and_warm_parser_without_mutation() {
    let (_root, _index, path, source, mut db) = setup();
    fs::OpenOptions::new().append(true).open(&path).unwrap().write_all(b"{\"type\":\"event_msg\",\"timestamp\":\"2026-10-05T00:00:00Z\",\"payload\":{\"type\":\"task_started\",\"turn_id\":\"turn\"}}\n{\"type\":\"event_msg\",\"timestamp\":\"2026-10-05T00:00:00.010Z\",\"payload\":{\"type\":\"agent_message\",\"message\":\"PRIVATE_EARLIER\"}}\n").unwrap();
    let mut cache = incremental::Cache::default();
    {
        let tx = db.transaction().unwrap();
        assert!(
            incremental::sync_cached(&tx, &source, false, &mut cache)
                .unwrap()
                .is_some()
        );
        tx.commit().unwrap();
    }
    assert!(!cache.needs_seed());
    let scope = format!("parser:{}:{VERSION}:1", source.id);
    let facts_scope = format!("{scope}:facts");
    let original = crate::live_index::load_map(&db, &scope).unwrap();
    assert_eq!(
        original["messageObservationVersion"],
        serde_json::json!(incremental::MESSAGE_OBSERVATION_VERSION)
    );
    {
        let tx = db.transaction().unwrap();
        assert!(
            incremental::sync_cached(&tx, &source, false, &mut cache)
                .unwrap()
                .is_none()
        );
    }
    let mut facts = crate::live_index::load_map(&db, &facts_scope).unwrap();
    // Model the old intermediate shape: explicit start/activity, no Message facts.
    let events = facts["events"].as_object_mut().unwrap();
    assert!(
        events
            .values()
            .any(|event| event["payload"]["kind"] == "message")
    );
    events.retain(|_, event| event["payload"]["kind"] != "message");
    let retained: Vec<Arc<crate::session_events::Event>> = facts["events"]
        .as_object()
        .unwrap()
        .values()
        .map(|value| Arc::new(serde_json::from_value(value.clone()).unwrap()))
        .collect();
    crate::live_index::save_events(
        &db,
        &facts_scope,
        &retained,
        true,
        &std::sync::atomic::AtomicBool::new(false),
    )
    .unwrap();
    // A later visible record cannot fill the old mapping's unobserved history.
    fs::OpenOptions::new().append(true).open(&path).unwrap().write_all(b"{\"type\":\"event_msg\",\"timestamp\":\"2026-10-05T00:00:01Z\",\"payload\":{\"type\":\"agent_message\",\"turn_id\":\"turn\",\"message\":\"PRIVATE_LATER\"}}\n").unwrap();
    for header in [
        None,
        Some(serde_json::json!(1)),
        Some(serde_json::json!(
            incremental::MESSAGE_OBSERVATION_VERSION + 1
        )),
    ] {
        let mut stored = original.clone();
        stored.remove("messageObservationVersion");
        if let Some(header) = header {
            stored.insert("messageObservationVersion".into(), header);
        }
        crate::live_index::save_map(&db, &scope, &stored).unwrap();
        for verify in [false, true] {
            let tx = db.transaction().unwrap();
            let error = incremental::sync_cached(&tx, &source, verify, &mut cache)
                .err()
                .unwrap();
            assert_eq!(
                crate::live_index::failure_code(&error),
                "UNSUPPORTED_VERSION"
            );
        }
        assert_eq!(crate::live_index::load_map(&db, &scope).unwrap(), stored);
        assert_eq!(
            crate::live_index::load_map(&db, &facts_scope).unwrap(),
            facts
        );
    }
}

#[test]
fn mapping_header_precedes_future_parser_payload_decoding() {
    let (_root, _index, _path, source, mut db) = setup();
    sync(&mut db, &source).unwrap();
    let scope = format!("parser:{}:{VERSION}:1", source.id);
    let original = crate::live_index::load_map(&db, &scope).unwrap();
    let nested = format!("{}0{}", "[".repeat(200), "]".repeat(200));
    for header in [
        None,
        Some(serde_json::json!(1)),
        Some(serde_json::json!(
            incremental::MESSAGE_OBSERVATION_VERSION + 1
        )),
    ] {
        let mut stored = original.clone();
        stored.remove("messageObservationVersion");
        if let Some(header) = header {
            stored.insert("messageObservationVersion".into(), header);
        }
        crate::live_index::save_map(&db, &scope, &stored).unwrap();
        // A future payload can be valid JSON beyond serde Value's current depth.
        db.execute("UPDATE entries SET payload=jsonb(?1) WHERE bucket IN (SELECT id FROM buckets WHERE scope=?2 AND field='checkpoints')", rusqlite::params![nested, scope]).unwrap();
        assert!(crate::live_index::load_map(&db, &scope).is_err());
        let before: String = db.query_row("SELECT json(e.payload) FROM buckets b JOIN entries e ON e.bucket=b.id WHERE b.scope=?1 AND b.field='checkpoints'", [&scope], |row| row.get(0)).unwrap();
        let tx = db.transaction().unwrap();
        let error = incremental::sync(&tx, &source, false).err().unwrap();
        assert_eq!(
            crate::live_index::failure_code(&error),
            "UNSUPPORTED_VERSION"
        );
        drop(tx);
        let after: String = db.query_row("SELECT json(e.payload) FROM buckets b JOIN entries e ON e.bucket=b.id WHERE b.scope=?1 AND b.field='checkpoints'", [&scope], |row| row.get(0)).unwrap();
        assert_eq!(after, before);
    }
}
