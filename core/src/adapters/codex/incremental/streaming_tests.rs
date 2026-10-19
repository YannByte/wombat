use super::*;

#[test]
fn first_scan_streams_multiple_batches_and_preserves_every_observation_on_append_and_rollback() {
    let source_root = tempfile::tempdir().unwrap();
    let index = tempfile::tempdir().unwrap();
    fs::create_dir(source_root.path().join("sessions")).unwrap();
    let path = source_root.path().join("sessions/large.jsonl");
    let mut rows =
        vec![serde_json::json!({"type":"session_meta","payload":{"id":"large"}}).to_string()];
    rows.push(serde_json::json!({"timestamp":"2026-10-01T00:00:00Z","type":"event_msg","payload":{"type":"task_started","turn_id":"turn"}}).to_string());
    for i in 0..1700 {
        rows.push(serde_json::json!({"type":"event_msg","payload":{"type":"agent_message","message":"synthetic","turn_id":"turn","id":format!("message-{i}")}}).to_string());
    }
    rows.push(serde_json::json!({"type":"event_msg","payload":{"type":"token_usage_record","thread_id":"large","turn_id":"turn","response_id":"response","usage":{"input_tokens":100,"output_tokens":10,"total_tokens":110}}}).to_string());
    rows.push(serde_json::json!({"timestamp":"2026-10-01T00:00:02Z","type":"event_msg","payload":{"type":"task_complete","turn_id":"turn"}}).to_string());
    fs::write(&path, rows.join("\n") + "\n").unwrap();
    let source = CodexAdapter
        .discover(&DiscoveryRequest {
            roots: vec![source_root.path().into()],
        })
        .sources
        .remove(0);
    let mut db = crate::live_index::open(&index.path().join("index.sqlite")).unwrap();
    let mut cache = Cache::default();
    let original;
    {
        let tx = db.transaction().unwrap();
        let result = sync_cached(&tx, &source, false, &mut cache)
            .unwrap()
            .unwrap();
        assert_eq!(result.collected.measurements[0].tokens.total, Some(110));
        assert!(result.collected.events.is_empty());
        assert_eq!(cache.encoded_bytes, 0);
        original = crate::live_index::load_map(&tx, &fact_scope(&source)).unwrap();
        assert!(original["events"].as_object().unwrap().len() > 1700);
        tx.commit().unwrap();
    }
    use std::io::Write;
    writeln!(fs::OpenOptions::new().append(true).open(&path).unwrap(), "{}", serde_json::json!({"type":"event_msg","payload":{"type":"agent_message","message":"next","turn_id":"turn"}})).unwrap();
    {
        let tx = db.transaction().unwrap();
        assert!(
            sync_cached(&tx, &source, false, &mut cache)
                .unwrap()
                .is_some()
        );
        assert!(
            crate::live_index::load_map(&tx, &fact_scope(&source)).unwrap()["events"]
                .as_object()
                .unwrap()
                .len()
                > original["events"].as_object().unwrap().len()
        );
        // Dropping the source transaction must leave every old frame intact.
    }
    assert_eq!(
        crate::live_index::load_map(&db, &fact_scope(&source)).unwrap(),
        original
    );
    let tx = db.transaction().unwrap();
    let mut restarted = Cache::default();
    let result = sync_cached(&tx, &source, false, &mut restarted)
        .unwrap()
        .unwrap();
    assert_eq!(result.collected.measurements[0].tokens.total, Some(110));
    assert!(result.collected.events.is_empty());
    assert_eq!(restarted.encoded_bytes, 0);
    tx.commit().unwrap();
}
