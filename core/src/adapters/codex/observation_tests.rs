//! Synthetic source observations; no source body or external title becomes usage.
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
    serde_json::json!({"type":"session_meta","payload":{"id":id}}).to_string()
}
fn sync(db: &mut rusqlite::Connection, source: &SourceInstance, verify: bool) -> Collected {
    let tx = db.transaction().unwrap();
    let result = incremental::sync(&tx, source, verify).unwrap().unwrap();
    tx.commit().unwrap();
    result
}
fn title(root: &Path, name: &str, updated: &str) {
    fs::write(
        root.join("session_index.jsonl"),
        serde_json::json!({"id":"initial","thread_name":name,"updated_at":updated}).to_string()
            + "\n",
    )
    .unwrap();
}
#[test]
fn collected_time_survives_append_restart_verify_and_rollback_without_filling_event_time() {
    let (_root, index, path, source, mut db) = setup();
    let first = sync(&mut db, &source, false);
    let times: BTreeMap<_, _> = first
        .events
        .iter()
        .map(|e| (e.id().to_owned(), e.collected_at().to_owned()))
        .collect();
    assert!(first.events.iter().all(|e| e.time().timestamp.is_none()));
    fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .unwrap()
        .write_all((row("next") + "\n").as_bytes())
        .unwrap();
    drop(db);
    db = crate::live_index::open(&index.path().join("index.sqlite")).unwrap();
    db.execute_batch("CREATE TEMP TABLE observation_rewrites(id TEXT); CREATE TEMP TRIGGER observe_event_update AFTER UPDATE ON event_blocks BEGIN INSERT INTO observation_rewrites VALUES(new.origin); END; CREATE TEMP TRIGGER observe_event_delete AFTER DELETE ON event_blocks BEGIN INSERT INTO observation_rewrites VALUES(old.origin); END;").unwrap();
    let appended = sync(&mut db, &source, false);
    assert!(
        db.query_row("SELECT count(*) FROM observation_rewrites", [], |r| r
            .get::<_, i64>(0))
            .unwrap()
            > 0,
        "append rewrites the bounded tail window"
    );
    db.execute("DELETE FROM observation_rewrites", []).unwrap();
    assert!(appended.events.len() > first.events.len());
    for e in &appended.events {
        if let Some(at) = times.get(e.id()) {
            assert_eq!(e.collected_at(), at);
        }
    }
    let serialized: BTreeMap<_, _> = appended
        .events
        .iter()
        .map(|e| (e.id().to_owned(), serde_json::to_value(e).unwrap()))
        .collect();
    let verified = sync(&mut db, &source, true);
    assert_eq!(
        db.query_row("SELECT count(*) FROM observation_rewrites", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        0
    );
    assert_eq!(
        verified
            .events
            .iter()
            .map(|e| (e.id().to_owned(), serde_json::to_value(e).unwrap()))
            .collect::<BTreeMap<_, _>>(),
        serialized
    );
    let scope = format!("parser:{}:{VERSION}:1:facts", source.id);
    let before = crate::live_index::load_map(&db, &scope).unwrap();
    fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .unwrap()
        .write_all((row("rolled-back") + "\n").as_bytes())
        .unwrap();
    {
        let tx = db.transaction().unwrap();
        assert!(incremental::sync(&tx, &source, false).unwrap().is_some());
    }
    assert_eq!(crate::live_index::load_map(&db, &scope).unwrap(), before);
}
#[test]
fn titles_restore_from_facts_noop_reuses_observation_and_complete_replacement_can_revoke() {
    let (root, index, path, source, mut db) = setup();
    title(root.path(), "safe title", "2026-10-05T00:00:00Z");
    let first = sync(&mut db, &source, false);
    let observation = first.title_observations[0].clone();
    assert_eq!(first.threads[0].title.as_deref(), Some("safe title"));
    drop(db);
    db = crate::live_index::open(&index.path().join("index.sqlite")).unwrap();
    fs::OpenOptions::new()
        .append(true)
        .open(&path)
        .unwrap()
        .write_all((row("next") + "\n").as_bytes())
        .unwrap();
    let restarted = sync(&mut db, &source, false);
    assert_eq!(restarted.title_observations, vec![observation.clone()]);
    let verified = sync(&mut db, &source, true);
    assert_eq!(verified.title_observations, vec![observation.clone()]);
    fs::remove_file(root.path().join("session_index.jsonl")).unwrap();
    let missing = sync(&mut db, &source, false);
    assert_eq!(missing.title_observations, vec![observation.clone()]);
    assert!(
        missing.sources[0]
            .issues
            .iter()
            .any(|i| i.code == "titleMissing")
    );
    fs::create_dir(root.path().join("session_index.jsonl")).unwrap();
    let unreadable = sync(&mut db, &source, false);
    assert_eq!(unreadable.title_observations, vec![observation]);
    assert!(
        unreadable.sources[0]
            .issues
            .iter()
            .any(|i| i.code == "titleUnreadable")
    );
    fs::remove_dir(root.path().join("session_index.jsonl")).unwrap();
    title(root.path(), "rolled back", "2026-10-04T00:00:00Z");
    let changed = sync(&mut db, &source, false);
    assert_eq!(
        changed
            .threads
            .iter()
            .find(|t| t.upstream_id == "initial")
            .unwrap()
            .title
            .as_deref(),
        Some("rolled back")
    );
    assert!(changed.measurements.is_empty() && changed.operations.is_empty());
    fs::write(root.path().join("session_index.jsonl"), "").unwrap();
    let empty = sync(&mut db, &source, false);
    assert!(empty.title_observations.is_empty());
    assert!(empty.threads.iter().all(|t| t.title.is_none()));
}
#[test]
fn title_scope_and_latest_row_are_source_local_and_invalid_source_time_cannot_be_fabricated() {
    let (root, _index, _path, source, mut db) = setup();
    fs::write(root.path().join("session_index.jsonl"), [
        serde_json::json!({"id":"initial","thread_name":"latest","updated_at":"2026-10-05T00:00:00Z"}),
        serde_json::json!({"id":"initial","thread_name":"older","updated_at":"2026-10-04T00:00:00Z"}),
    ].iter().map(|v|v.to_string()+"\n").collect::<String>()).unwrap();
    let first = sync(&mut db, &source, false);
    assert_eq!(first.title_observations[0].title, "latest");
    let (other, _index2, _path2, source2, mut db2) = setup();
    title(other.path(), "other source", "2026-10-05T00:00:00Z");
    let second = sync(&mut db2, &source2, false);
    assert_ne!(
        first.title_observations[0].thread_id,
        second.title_observations[0].thread_id
    );
    assert_ne!(
        first.title_observations[0].source_instance_id,
        second.title_observations[0].source_instance_id
    );
    title(root.path(), "invalid", "not-time");
    let invalid = sync(&mut db, &source, false);
    assert_eq!(invalid.title_observations, first.title_observations);
    assert!(
        invalid.sources[0]
            .issues
            .iter()
            .any(|i| i.code == "invalidTitle")
    );
}
#[test]
fn required_parser_headers_reject_old_and_future_shapes_before_loading_facts() {
    let (_root, _index, _path, source, mut db) = setup();
    sync(&mut db, &source, false);
    let scope = format!("parser:{}:{VERSION}:1", source.id);
    let original = crate::live_index::load_map(&db, &scope).unwrap();
    for field in [
        "eventObservationVersion",
        "titleObservationVersion",
        "operationAssociationVersion",
    ] {
        for future in [None, Some(serde_json::json!(99))] {
            let mut changed = original.clone();
            changed.remove(field);
            if let Some(value) = future {
                changed.insert(field.into(), value);
            }
            crate::live_index::save_map(&db, &scope, &changed).unwrap();
            let error = incremental::sync(&db, &source, false).unwrap_err();
            assert_eq!(
                error
                    .downcast_ref::<crate::dto::OperationError>()
                    .unwrap()
                    .code,
                "UNSUPPORTED_VERSION"
            );
            assert_eq!(crate::live_index::load_map(&db, &scope).unwrap(), changed);
        }
    }
}

#[test]
fn replaced_or_truncated_threads_revoke_titles_even_when_title_index_cannot_be_read() {
    for replacement in [false, true] {
        for missing in [false, true] {
            let (root, index, path, source, mut db) = setup();
            fs::write(
                root.path().join("sessions/survivor.jsonl"),
                row("survivor") + "\n",
            )
            .unwrap();
            fs::write(
                root.path().join("session_index.jsonl"),
                ["initial", "survivor"]
                    .map(|id| {
                        serde_json::json!({"id":id,"thread_name":id,"updated_at":"2026-10-05T00:00:00Z"})
                            .to_string() + "\n"
                    })
                    .concat(),
            )
            .unwrap();
            let first = sync(&mut db, &source, false);
            let survivor = first
                .title_observations
                .iter()
                .find(|observation| observation.title == "survivor")
                .unwrap()
                .clone();
            let fixed_root = tempfile::tempdir().unwrap();
            if replacement {
                let replaced = root.path().join("replacement");
                fs::write(&replaced, row("replacement") + "\n").unwrap();
                fs::rename(replaced, &path).unwrap();
            } else {
                fs::write(&path, "").unwrap();
            }
            if missing {
                fs::remove_file(root.path().join("session_index.jsonl")).unwrap();
            } else {
                fs::write(root.path().join("session_index.jsonl"), "invalid\n").unwrap();
            }
            let rebuilt = sync(&mut db, &source, false);
            assert!(
                rebuilt
                    .threads
                    .iter()
                    .all(|thread| thread.upstream_id != "initial")
            );
            assert_eq!(rebuilt.title_observations, vec![survivor.clone()]);
            assert!(rebuilt.sources[0].issues.iter().any(|issue| issue.code
                == if missing {
                    "titleMissing"
                } else {
                    "invalidTitle"
                }));
            let prices = crate::pricing_sync::current_at(fixed_root.path()).unwrap();
            crate::usage_store::memory(rebuilt, "live:rebuilt-titles".into(), prices, None)
                .unwrap();
            drop(db);
            db = crate::live_index::open(&index.path().join("index.sqlite")).unwrap();
            let restarted = sync(&mut db, &source, true);
            assert_eq!(restarted.title_observations, vec![survivor]);
            assert_eq!(first.title_observations.len(), 2);
            assert!(
                first
                    .title_observations
                    .iter()
                    .any(|observation| observation.title == "initial")
            );
        }
    }
    // Revoking the last target still reports failure of a previously observed index.
    let (root, _index, path, source, mut db) = setup();
    title(root.path(), "old", "2026-10-05T00:00:00Z");
    sync(&mut db, &source, false);
    fs::write(path, "").unwrap();
    fs::remove_file(root.path().join("session_index.jsonl")).unwrap();
    let empty = sync(&mut db, &source, false);
    assert!(empty.title_observations.is_empty());
    assert!(
        empty.sources[0]
            .issues
            .iter()
            .any(|issue| issue.code == "titleMissing")
    );
}
