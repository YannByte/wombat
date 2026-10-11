use super::*;
use std::collections::BTreeSet;
use std::path::Path;

#[test]
fn event_projection_appends_skip_old_membership_and_failed_publication_rolls_back() {
    use std::io::Write;
    let root = tempfile::tempdir().unwrap();
    let index = tempfile::tempdir().unwrap();
    fs::create_dir(root.path().join("sessions")).unwrap();
    let path = root.path().join("sessions/events.jsonl");
    let initial = [
        json!({"type":"session_meta","timestamp":"2026-10-04T01:00:00Z","payload":{"id":"task"}}),
        json!({"type":"event_msg","timestamp":"2026-10-04T01:00:01Z","payload":{"type":"task_started","turn_id":"a"}}),
    ]
    .iter()
    .map(|row| row.to_string() + "\n")
    .collect::<String>();
    fs::write(&path, &initial).unwrap();
    let roots = vec![root.path().to_string_lossy().into_owned()];
    let key = source_key(&roots);
    let mut db = crate::live_index::open(&index.path().join("index.sqlite")).unwrap();
    let mut caches = BTreeMap::new();
    let first = sync(&mut db, &key, &roots, false, None, &mut caches)
        .unwrap()
        .unwrap();
    assert!(first.publication_change.is_none());
    let source_id = sources(&roots).sources.remove(0).id;
    let scope = format!("projection:{source_id}");
    let bucket: i64 = db
        .query_row(
            "SELECT id FROM buckets WHERE scope=?1 AND field='events'",
            [&scope],
            |row| row.get(0),
        )
        .unwrap();
    // Unchanged source links must not be rewritten on append.
    db.execute_batch(&format!("CREATE TRIGGER reject_old_event BEFORE UPDATE ON event_links WHEN NEW.bucket={bucket} BEGIN SELECT RAISE(ABORT,'historical event membership rewritten'); END;")).unwrap();
    writeln!(
        fs::OpenOptions::new().append(true).open(&path).unwrap(),
        "{}",
        json!({"type":"event_msg","timestamp":"2026-10-04T01:00:02Z","payload":{"type":"task_complete","turn_id":"a"}})
    )
    .unwrap();
    let second = sync(&mut db, &key, &roots, false, Some(&first), &mut caches)
        .unwrap()
        .unwrap();
    let change = second.publication_change.as_ref().unwrap();
    assert_eq!(
        change.baseline.snapshot_id,
        first.manifest.snapshot_ref.snapshot_id
    );
    assert_eq!(
        change.current.snapshot_id,
        second.manifest.snapshot_ref.snapshot_id
    );
    assert_eq!(change.turns_changed, 1);
    assert_eq!(change.measurements_added, 0);
    assert_eq!(first.events().unwrap().len(), 5);
    assert_eq!(second.events().unwrap().len(), 9);
    let saved = crate::live_index::load_map(&db, &scope).unwrap();
    let view_scope = format!("view:{key}");
    let saved_view = crate::live_index::load_map(&db, &view_scope).unwrap();
    db.execute_batch(&format!("DROP TRIGGER reject_old_event; CREATE TRIGGER reject_new_event BEFORE INSERT ON event_links WHEN NEW.bucket={bucket} BEGIN SELECT RAISE(ABORT,'synthetic publication failure'); END;")).unwrap();
    writeln!(
        fs::OpenOptions::new().append(true).open(&path).unwrap(),
        "{}",
        json!({"type":"event_msg","timestamp":"2026-10-04T01:00:03Z","payload":{"type":"task_started","turn_id":"b"}})
    )
    .unwrap();
    assert!(sync(&mut db, &key, &roots, false, Some(&second), &mut caches).is_err());
    assert_eq!(crate::live_index::load_map(&db, &scope).unwrap(), saved);
    assert_eq!(
        crate::live_index::load_map(&db, &view_scope).unwrap(),
        saved_view
    );
    assert_eq!(
        restore(&db, &key, &roots)
            .unwrap()
            .unwrap()
            .events()
            .unwrap(),
        second.events().unwrap()
    );
    let restored = restore(&db, &key, &roots).unwrap().unwrap();
    assert_eq!(
        serde_json::to_value(&restored.publication_change).unwrap(),
        serde_json::to_value(&second.publication_change).unwrap()
    );
    // The live worker discards parser caches after a failed transaction.
    caches.clear();
    db.execute_batch("DROP TRIGGER reject_new_event").unwrap();
    let recovered = sync(&mut db, &key, &roots, false, Some(&second), &mut caches)
        .unwrap()
        .unwrap();
    assert!(recovered.events().unwrap().len() > second.events().unwrap().len());
    let verified = sync(&mut db, &key, &roots, true, Some(&recovered), &mut caches)
        .unwrap()
        .unwrap();
    assert_eq!(verified.events().unwrap(), recovered.events().unwrap());
    fs::write(&path, initial).unwrap();
    let replacement = sync(&mut db, &key, &roots, false, Some(&verified), &mut caches)
        .unwrap()
        .unwrap();
    assert_eq!(replacement.events().unwrap().len(), 5);
    let original_ids: BTreeSet<_> = first
        .events()
        .unwrap()
        .iter()
        .map(|e| e.id().to_owned())
        .collect();
    assert!(
        replacement
            .events()
            .unwrap()
            .iter()
            .all(|e| !original_ids.contains(e.id()))
    );
    assert_eq!(
        restore(&db, &key, &roots)
            .unwrap()
            .unwrap()
            .events()
            .unwrap(),
        replacement.events().unwrap()
    );
}

fn source(root: &Path, title: &str) -> PathBuf {
    fs::create_dir(root.join("sessions")).unwrap();
    let path = root.join("sessions/source.jsonl");
    fs::write(
        &path,
        json!({"type":"session_meta","payload":{"id":"same-native-id"}}).to_string() + "\n",
    )
    .unwrap();
    fs::write(
        root.join("session_index.jsonl"),
        json!({"id":"same-native-id","thread_name":title,"updated_at":"2026-10-05T00:00:00Z"})
            .to_string()
            + "\n",
    )
    .unwrap();
    path
}
#[test]
fn title_observation_restore_is_source_scoped_and_rejects_corrupt_bindings() {
    let a = tempfile::tempdir().unwrap();
    let b = tempfile::tempdir().unwrap();
    let index = tempfile::tempdir().unwrap();
    source(a.path(), "source A");
    source(b.path(), "source B");
    let roots = vec![
        a.path().to_string_lossy().into_owned(),
        b.path().to_string_lossy().into_owned(),
    ];
    let key = source_key(&roots);
    let mut db = crate::live_index::open(&index.path().join("index.sqlite")).unwrap();
    let mut caches = BTreeMap::new();
    let first = sync(&mut db, &key, &roots, false, None, &mut caches)
        .unwrap()
        .unwrap();
    assert_eq!(first.manifest.title_observations.len(), 2);
    let restored = restore(&db, &key, &roots).unwrap().unwrap();
    assert_eq!(
        restored.manifest.title_observations,
        first.manifest.title_observations
    );
    for observation in &restored.manifest.title_observations {
        assert_eq!(
            restored
                .manifest
                .threads
                .iter()
                .find(|t| t.thread.id == observation.thread_id)
                .unwrap()
                .thread
                .title
                .as_deref(),
            Some(observation.title.as_str())
        );
    }
    let source_id = sources(&[roots[0].clone()]).sources.remove(0).id;
    let scope = format!("projection:{source_id}");
    let original = crate::live_index::load_map(&db, &scope).unwrap();
    let mut changed = original.clone();
    let rows = changed["title_observations"].as_object_mut().unwrap();
    rows.values_mut().next().unwrap()["threadId"] = json!("foreign-thread");
    crate::live_index::save_map(&db, &scope, &changed).unwrap();
    assert!(restore(&db, &key, &roots).is_err());
    assert_eq!(crate::live_index::load_map(&db, &scope).unwrap(), changed);
}
#[test]
fn observation_headers_reject_cached_append_restore_and_mixed_fallback_without_changing_old_view() {
    use std::io::Write;
    let a = tempfile::tempdir().unwrap();
    let b = tempfile::tempdir().unwrap();
    let index = tempfile::tempdir().unwrap();
    let path = source(a.path(), "A");
    source(b.path(), "B");
    let roots = vec![
        a.path().to_string_lossy().into_owned(),
        b.path().to_string_lossy().into_owned(),
    ];
    let key = source_key(&roots);
    let mut db = crate::live_index::open(&index.path().join("index.sqlite")).unwrap();
    let mut caches = BTreeMap::new();
    let first = sync(&mut db, &key, &roots, false, None, &mut caches)
        .unwrap()
        .unwrap();
    let source_id = sources(&[roots[0].clone()]).sources.remove(0).id;
    let projection = format!("projection:{source_id}");
    let parser = format!("parser:{source_id}:{}:1", adapters::codex::VERSION);
    let view = format!("view:{key}");
    let saved_view = crate::live_index::load_map(&db, &view).unwrap();
    let saved_projection = crate::live_index::load_map(&db, &projection).unwrap();
    let saved_parser = crate::live_index::load_map(&db, &parser).unwrap();
    for field in ["eventObservationVersion", "titleObservationVersion"] {
        for scope in [&projection, &parser] {
            for future in [None, Some(json!(99))] {
                crate::live_index::save_map(&db, &projection, &saved_projection).unwrap();
                crate::live_index::save_map(&db, &parser, &saved_parser).unwrap();
                let mut changed = if scope == &projection {
                    saved_projection.clone()
                } else {
                    saved_parser.clone()
                };
                changed.remove(field);
                if let Some(version) = future {
                    changed.insert(field.into(), version);
                }
                changed.insert("futureShape".into(), json!({"not":"current"}));
                crate::live_index::save_map(&db, scope, &changed).unwrap();
                for append in [false, true] {
                    if append {
                        fs::OpenOptions::new()
                            .append(true)
                            .open(&path)
                            .unwrap()
                            .write_all(b"\n")
                            .unwrap();
                    }
                    let error = sync(&mut db, &key, &roots, false, Some(&first), &mut caches)
                        .err()
                        .unwrap();
                    assert_eq!(
                        crate::live_index::failure_code(&error),
                        "UNSUPPORTED_VERSION"
                    );
                    assert_eq!(crate::live_index::load_map(&db, scope).unwrap(), changed);
                    assert_eq!(crate::live_index::load_map(&db, &view).unwrap(), saved_view);
                }
                if scope == &projection {
                    assert_eq!(
                        crate::live_index::failure_code(&restore(&db, &key, &roots).err().unwrap()),
                        "UNSUPPORTED_VERSION"
                    );
                }
            }
        }
    }
}

#[test]
fn future_publication_change_rejects_before_project_publication_without_writes() {
    let root = tempfile::tempdir().unwrap();
    let index = tempfile::tempdir().unwrap();
    source(root.path(), "A");
    let roots = vec![root.path().to_string_lossy().into_owned()];
    let key = source_key(&roots);
    let mut db = crate::live_index::open(&index.path().join("index.sqlite")).unwrap();
    sync(&mut db, &key, &roots, false, None, &mut BTreeMap::new()).unwrap();
    let scope = format!("view:{key}");
    let mut view = crate::live_index::load_map(&db, &scope).unwrap();
    // A future header deliberately has no current payload fields.
    view.insert(
        "publicationChange".into(),
        json!({"methodVersion":2,"futureShape":true}),
    );
    crate::live_index::save_map(&db, &scope, &view).unwrap();
    let error = restore_projects::restore_projects(
        &db,
        &key,
        &roots,
        || None,
        |_| panic!("unsupported publication must never expose partial facts"),
    )
    .err()
    .unwrap();
    assert_eq!(
        crate::live_index::failure_code(&error),
        "UNSUPPORTED_VERSION"
    );
    assert_eq!(crate::live_index::load_map(&db, &scope).unwrap(), view);
}
