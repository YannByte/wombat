use super::*;
use crate::adapters::{
    self,
    contract::{DiscoveryRequest, RunContext},
};
use serde_json::json;
use std::fs;
fn rows(text: &str) -> String {
    [json!({"type":"session_meta","timestamp":"2026-10-14T00:00:00Z","payload":{"id":"thread","cwd":"/synthetic/project"}}), json!({"type":"turn_context","timestamp":"2026-10-14T00:00:01Z","payload":{"turn_id":"turn","cwd":"/synthetic/project"}}), json!({"type":"event_msg","timestamp":"2026-10-14T00:00:02Z","payload":{"type":"agent_message","phase":"final_answer","turn_id":"turn","message":text}})].iter().map(|r| r.to_string()+"\n").collect()
}
fn collect(root: &std::path::Path, id: &str) -> Snapshot {
    let data = adapters::collect(
        &DiscoveryRequest {
            roots: vec![root.into()],
        },
        &RunContext::default(),
    );
    assert!(data.measurements.is_empty());
    assert!(
        !serde_json::to_string(&data.events)
            .unwrap()
            .contains("PRIVATE_SYNTHETIC")
    );
    crate::usage_store::memory(
        data,
        id.into(),
        crate::pricing_sync::current_at(&root.join("prices")).unwrap(),
        None,
    )
    .unwrap()
}
fn fixture(text: &str) -> (tempfile::TempDir, Snapshot, std::path::PathBuf) {
    let root = tempfile::tempdir().unwrap();
    fs::create_dir(root.path().join("sessions")).unwrap();
    let path = root.path().join("sessions/source.jsonl");
    fs::write(&path, rows(text)).unwrap();
    let snapshot = collect(root.path(), "view-one");
    (root, snapshot, path)
}
fn request(snapshot: &Snapshot, action: Action) -> Request {
    serde_json::from_value(
        json!({"action":action,"snapshotId":snapshot.manifest.snapshot_ref.snapshot_id}),
    )
    .unwrap()
}
fn locate(snapshot: &Snapshot) -> Evidence {
    let r = query(
        snapshot,
        None,
        &request(snapshot, Action::Locate),
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(r.items.len(), 1);
    assert!(r.items[0].has_text_locator);
    r.items[0].evidence.clone()
}

#[test]
fn expired_scan_slice_still_advances_through_messages_and_control_events() {
    let (_root, snapshot, _path) = fixture("PRIVATE_SYNTHETIC_PROGRESS");
    let cancelled = AtomicBool::new(false);
    for action in [Action::Activity, Action::Locate, Action::Search] {
        let mut r = request(&snapshot, action);
        if action == Action::Search {
            r.query = Some("PRIVATE_SYNTHETIC_PROGRESS".into());
            r.selection.projects = vec!["/synthetic/project".into()];
        }
        let expected = query(&snapshot, None, &r, &cancelled).unwrap();
        let mut items = vec![];
        let mut complete = false;
        for _ in 0..snapshot.events().unwrap().len() + 1 {
            let page = query_with_scan_budget(&snapshot, None, &r, &cancelled, 0).unwrap();
            items.extend(page.items);
            if page.scan_complete {
                complete = true;
                break;
            }
            assert_ne!(page.next_cursor, r.cursor, "expired slice must advance");
            r.cursor = page.next_cursor;
        }
        assert!(complete, "continuation did not finish");
        assert_eq!(
            serde_json::to_value(items).unwrap(),
            serde_json::to_value(expected.items).unwrap()
        );
    }
}
fn read_request(snapshot: &Snapshot, evidence: Evidence, max: usize) -> Request {
    let mut r = request(snapshot, Action::Read);
    r.targets.push(ReadTarget {
        evidence,
        cursor: None,
        max_bytes: Some(max),
        before: 0,
        after: 0,
    });
    r
}
#[test]
fn activity_covers_no_usage_and_calendar_filters_do_not_guess_dates() {
    let (_root, snapshot, _) = fixture("PRIVATE_SYNTHETIC answer");
    let mut r = request(&snapshot, Action::Activity);
    r.selection.projects = vec!["/synthetic/project".into()];
    r.selection.timezone = Some("Asia/Shanghai".into());
    r.selection.since = Some("2026-10-14".into());
    r.selection.until = Some("2026-10-15".into());
    let result = query(&snapshot, None, &r, &AtomicBool::new(false)).unwrap();
    assert!(result.scan_complete);
    assert_eq!(result.source_bytes_read, 0);
    assert!(result.items.iter().any(|i| i.kind == "message"));
    r.selection.until = Some("2026-10-14".into());
    assert!(query(&snapshot, None, &r, &AtomicBool::new(false)).is_err());
}
#[test]
fn unicode_fragment_continuation_and_large_message_read_amplification() {
    for text in ["中文🙂\n\\\"".repeat(20), "x".repeat(2 * 1024 * 1024)] {
        let (_root, snapshot, _) = fixture(&text);
        let evidence = locate(&snapshot);
        let mut r = read_request(
            &snapshot,
            evidence,
            if text.len() > 1024 { 256 * 1024 } else { 13 },
        );
        let mut all = String::new();
        let mut bytes = 0;
        loop {
            let result = query(&snapshot, None, &r, &AtomicBool::new(false)).unwrap();
            bytes += result.source_bytes_read;
            let f = &result.fragments[0];
            assert!(f.text.len() <= r.targets[0].max_bytes.unwrap());
            all.push_str(&f.text);
            if f.next_cursor.is_none() {
                break;
            }
            r.targets[0].cursor = f.next_cursor.clone();
        }
        assert_eq!(all, text);
        if text.len() > 1024 {
            assert_eq!(bytes, text.len() as u64);
        }
    }
}
#[test]
fn search_crosses_chunk_boundaries_and_resumes_with_adjustable_budget() {
    let text = "x".repeat(8190) + "needle_cross中文";
    let (_root, snapshot, _) = fixture(&text);
    let evidence = locate(&snapshot);
    let mut r = request(&snapshot, Action::Search);
    r.selection.threads = vec![evidence.thread_id.clone().unwrap()];
    r.query = Some("needle_cross中文".into());
    r.max_scan_bytes = Some(8192);
    let first = query(&snapshot, None, &r, &AtomicBool::new(false)).unwrap();
    assert!(!first.scan_complete);
    assert!(first.items.is_empty());
    assert!(first.next_cursor.is_some());
    r.cursor = first.next_cursor;
    r.max_scan_bytes = Some(32768);
    let second = query(&snapshot, None, &r, &AtomicBool::new(false)).unwrap();
    assert_eq!(second.items.len(), 1);
    assert!(second.scan_complete);
    r.query = Some("different".into());
    assert!(query(&snapshot, None, &r, &AtomicBool::new(false)).is_err());
}
#[test]
fn reads_fail_on_replacement_rewrite_truncation_and_missing_source() {
    let (_root, snapshot, path) = fixture("PRIVATE_SYNTHETIC");
    let r = read_request(&snapshot, locate(&snapshot), 8192);
    let before = fs::metadata(&path).unwrap().modified().unwrap();
    fs::write(&path, rows("CHANGED_SYNTHETIC")).unwrap();
    fs::OpenOptions::new()
        .write(true)
        .open(&path)
        .unwrap()
        .set_times(fs::FileTimes::new().set_modified(before))
        .unwrap();
    assert!(query(&snapshot, None, &r, &AtomicBool::new(false)).is_err());
    fs::write(&path, rows("PRIVATE_SYNTHETIC")).unwrap();
    let replacement = path.with_extension("replacement");
    fs::write(&replacement, rows("PRIVATE_SYNTHETIC")).unwrap();
    fs::rename(&replacement, &path).unwrap();
    assert!(query(&snapshot, None, &r, &AtomicBool::new(false)).is_err());
    fs::write(&path, "").unwrap();
    assert!(query(&snapshot, None, &r, &AtomicBool::new(false)).is_err());
    fs::remove_file(&path).unwrap();
    assert!(query(&snapshot, None, &r, &AtomicBool::new(false)).is_err());
}
#[test]
fn incremental_activity_uses_fixed_baseline_file_boundaries() {
    let (root, old, path) = fixture("old");
    use std::io::Write;
    let appended = json!({"type":"event_msg","timestamp":"2026-10-14T01:00:00Z","payload":{"type":"user_message","turn_id":"turn","message":"new"}});
    writeln!(
        fs::OpenOptions::new().append(true).open(path).unwrap(),
        "{appended}"
    )
    .unwrap();
    let current = collect(root.path(), "view-two");
    assert!(old.manifest.watermarks[0].committed_offset > 0);
    assert_eq!(
        old.manifest.watermarks[0].generation,
        current.manifest.watermarks[0].generation
    );
    let mut r = request(&current, Action::Changes);
    r.baseline_snapshot_id = Some(old.manifest.snapshot_ref.snapshot_id.clone());
    let response = query(&current, Some(&old), &r, &AtomicBool::new(false)).unwrap();
    assert!(
        response
            .items
            .iter()
            .all(|i| matches!(i.change, Some(Change::Appended))),
        "{:?}",
        response
            .items
            .iter()
            .map(|i| (&i.change, &i.evidence.source_instance_id))
            .collect::<Vec<_>>()
    );
    assert_eq!(
        response
            .items
            .iter()
            .filter(|i| i.kind == "message")
            .count(),
        1
    );
    assert!(
        response
            .items
            .iter()
            .all(|i| matches!(i.change, Some(Change::Appended)))
    );
}
#[test]
fn empty_content_still_checks_source_and_read_targets_cannot_bypass_scope() {
    let (_root, snapshot, path) = fixture("");
    let mut r = read_request(&snapshot, locate(&snapshot), 1);
    assert_eq!(
        query(&snapshot, None, &r, &AtomicBool::new(false))
            .unwrap()
            .fragments[0]
            .text,
        ""
    );
    r.selection.projects = vec!["/unrelated".into()];
    assert!(query(&snapshot, None, &r, &AtomicBool::new(false)).is_err());
    r.selection.projects.clear();
    r.targets[0].evidence.event_offset += 1;
    assert!(query(&snapshot, None, &r, &AtomicBool::new(false)).is_err());
    r.targets[0].evidence.event_offset -= 1;
    assert!(query(&snapshot, None, &r, &AtomicBool::new(true)).is_err());
    fs::remove_file(path).unwrap();
    assert!(query(&snapshot, None, &r, &AtomicBool::new(false)).is_err());
}

#[test]
fn message_kinds_anchor_windows_and_empty_arrays_keep_exact_evidence() {
    let (root, _, path) = fixture("before");
    use std::io::Write;
    let extra = json!({"type":"response_item","timestamp":"2026-10-14T00:00:03Z","payload":{"type":"message","role":"assistant","id":"native","phase":"final_answer","internal_chat_message_metadata_passthrough":{"turn_id":"turn"},"content":[]}});
    writeln!(
        fs::OpenOptions::new().append(true).open(path).unwrap(),
        "{extra}"
    )
    .unwrap();
    let snapshot = collect(root.path(), "anchored");
    let mut r = request(&snapshot, Action::Locate);
    r.selection.record_kinds = vec![crate::session_events::MessageRecordKind::ResponseSnapshot];
    let located = query(&snapshot, None, &r, &AtomicBool::new(false)).unwrap();
    assert_eq!(located.items.len(), 1);
    let evidence = located.items[0].evidence.clone();
    let read = query(
        &snapshot,
        None,
        &read_request(&snapshot, evidence.clone(), 8192),
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(read.fragments[0].text, "");
    assert!(read.source_bytes_read > 0);
    r.selection.record_kinds.clear();
    r.anchor = Some(evidence);
    r.radius = Some(20);
    let around = query(&snapshot, None, &r, &AtomicBool::new(false)).unwrap();
    assert_eq!(around.items.len(), 2);
    assert!(around.window.is_some());
    assert_eq!(around.source_bytes_read, 0);
}
