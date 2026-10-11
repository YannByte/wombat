//! Restore committed facts one project at a time, prioritizing the visible project.
//! One read transaction binds every batch. Progress never seeds parser accounting.
use super::*;
use crate::live::{ProjectLoad, ProjectLoadState};

pub(crate) fn incomplete(snapshot: &Snapshot) -> bool {
    !snapshot.project_loads.is_empty()
        && snapshot
            .project_loads
            .iter()
            .any(|p| p.state != ProjectLoadState::Ready)
}
pub(crate) fn restore_projects(
    db: &rusqlite::Connection,
    key: &str,
    roots: &[String],
    priority: impl Fn() -> Option<String>,
    mut publish: impl FnMut(Arc<Snapshot>),
) -> Result<Option<Arc<Snapshot>>> {
    let read_tx = db.unchecked_transaction()?;
    let db = &read_tx;
    let prior = crate::live_index::load_map(db, &format!("view:{key}"))?;
    let Some(id) = prior.get("id").and_then(Value::as_str) else {
        return Ok(None);
    };
    // Validate the header before decoding facts or publishing a partial project.
    let publication_change = prior
        .get("publicationChange")
        .filter(|value| !value.is_null())
        .map(|value| {
            if value.get("methodVersion").and_then(Value::as_u64) != Some(1) {
                return Err(operation_error(
                    "UNSUPPORTED_VERSION",
                    "不支持的刷新变化摘要版本",
                ));
            }
            let change: crate::usage_app_dto::PublicationChange =
                serde_json::from_value(value.clone())?;
            anyhow::ensure!(
                change.current.snapshot_id == id,
                "publication identity mismatch"
            );
            Ok(change)
        })
        .transpose()?;
    let prices = crate::pricing_sync::current()?;
    if prior.get("prices").and_then(Value::as_str) != Some(&prices.catalog_hash) {
        return Ok(None);
    }
    let mut collected = Collected::default();
    for source in sources(roots).sources {
        let epoch = crate::live_index::load_map(db, &format!("epoch:{}", source.id))?;
        if prior.get("epochs").and_then(|v| v.get(&source.id)) != Some(&Value::Object(epoch)) {
            return Ok(None);
        }
        let failure = prior
            .get("failures")
            .and_then(|v| v.get(&source.id))
            .map(|v| serde_json::from_value::<SourceReport>(v.clone()))
            .transpose()?;
        let mut value = match load_collected_except(db, &source.id, true, true, true)? {
            Some(value) => value,
            None if failure.is_some() => Collected::default(),
            None => return Ok(None),
        };
        if let Some(failure) = failure {
            value.issues = failure.issues.clone();
            value.sources = vec![failure];
            value.watermarks = serde_json::from_value(
                prior
                    .get("failureWatermarks")
                    .and_then(|v| v.get(&source.id))
                    .ok_or_else(|| anyhow::anyhow!("missing failure watermark observations"))?
                    .clone(),
            )?;
            validate_watermarks(&value.watermarks)?;
        }
        collected.watermarks.extend(value.watermarks);
        collected.sources.extend(value.sources);
        collected.issues.extend(value.issues);
        collected.threads.extend(value.threads);
        collected.turns.extend(value.turns);
        collected.measurements.extend(value.measurements);
        collected.operations.extend(value.operations);
        collected
            .title_observations
            .extend(value.title_observations);
    }
    let events = crate::usage_store::load_cold_event_index(db, key)?
        .ok_or_else(|| operation_error("INDEX_UNAVAILABLE", "已提交冷证据索引缺失"))?;
    let operations =
        crate::usage_store::load_cold_operations(db, &collected.sources, &collected.threads)?;
    let mut snapshot = crate::usage_store::memory_cold_operations(
        collected,
        id.to_owned(),
        prices.clone(),
        events,
        operations,
    )?;
    if let Some(at) = prior.get("createdAt").and_then(Value::as_str) {
        snapshot.manifest.snapshot_ref.created_at = at.into();
    }
    snapshot.publication_change = publication_change;
    snapshot.capture_cold_events(db, key, prices)?;
    let mut groups: BTreeMap<Option<String>, usize> = BTreeMap::new();
    for thread in &snapshot.manifest.threads {
        groups.entry(thread.thread.project.clone()).or_default();
    }
    if groups.is_empty() {
        groups.insert(None, 0);
    }
    let mut progress: Vec<_> = groups
        .keys()
        .cloned()
        .map(|project| ProjectLoad {
            project,
            state: ProjectLoadState::Pending,
        })
        .collect();
    // Build accounting and query indexes once. Project progress shares that state.
    while groups.len() > 1 {
        let project = next_project(&groups, priority());
        groups.remove(&project);
        for load in &mut progress {
            if load.project == project {
                load.state = ProjectLoadState::Ready;
            } else if load.state == ProjectLoadState::Loading {
                load.state = ProjectLoadState::Pending;
            }
        }
        let next = next_project(&groups, priority());
        progress
            .iter_mut()
            .find(|p| p.project == next)
            .unwrap()
            .state = ProjectLoadState::Loading;
        publish(Arc::new(snapshot.project_view(
            format!("live:{key}:{}", uuid::Uuid::new_v4()),
            progress.clone(),
        )));
    }
    read_tx.commit()?;
    let snapshot = Arc::new(snapshot);
    publish(Arc::clone(&snapshot));
    Ok(Some(snapshot))
}

fn next_project(
    groups: &BTreeMap<Option<String>, usize>,
    preferred: Option<String>,
) -> Option<String> {
    if let Some(project) = preferred
        && groups.contains_key(&Some(project.clone()))
    {
        return Some(project);
    }
    groups
        .iter()
        .filter(|(project, _)| project.is_some())
        .min_by_key(|(project, facts)| (*facts, *project))
        .map(|(project, _)| project.clone())
        .unwrap_or(None)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source(root: &std::path::Path, name: &str, tokens: u64) {
        fs::create_dir_all(root.join("sessions")).unwrap();
        let rows = [
            json!({"type":"session_meta","payload":{"id":name,"cwd":format!("/synthetic/{name}")}}),
            json!({"type":"turn_context","payload":{"turn_id":"turn","model":"gpt-5.4"}}),
            json!({"type":"event_msg","payload":{"type":"token_usage_record","thread_id":name,"turn_id":"turn","response_id":"response","usage":{"input_tokens":tokens-10,"output_tokens":10,"total_tokens":tokens}}}),
        ];
        fs::write(
            root.join(format!("sessions/{name}.jsonl")),
            rows.iter()
                .map(|v| v.to_string() + "\n")
                .collect::<String>(),
        )
        .unwrap();
    }

    #[test]
    fn measurement_only_unassigned_turn_restores_and_append_changes_are_exact() {
        use std::io::Write;
        let root = tempfile::tempdir().unwrap();
        let index = tempfile::tempdir().unwrap();
        fs::create_dir_all(root.path().join("sessions")).unwrap();
        let path = root.path().join("sessions/a.jsonl");
        let record = |id: &str| json!({"type":"event_msg","payload":{"type":"token_usage_record","thread_id":"a","response_id":id,"usage":{"input_tokens":100,"output_tokens":10,"total_tokens":110}}});
        fs::write(
            &path,
            format!(
                "{}\n{}\n",
                json!({"type":"session_meta","payload":{"id":"a","cwd":"/synthetic/a"}}),
                record("first")
            ),
        )
        .unwrap();
        let roots = vec![root.path().to_string_lossy().into_owned()];
        let key = source_key(&roots);
        let mut caches = BTreeMap::new();
        let mut db = crate::live_index::open(&index.path().join("index.sqlite")).unwrap();
        let first = sync(&mut db, &key, &roots, false, None, &mut caches)
            .unwrap()
            .unwrap();
        let id = &first.manifest.threads[0].thread.id;
        assert!(first.manifest.threads[0].turns.contains_key("unassigned"));
        assert_eq!(first.turn(id, "unassigned").unwrap().measurements.len(), 1);
        let restored = restore(&db, &key, &roots).unwrap().unwrap();
        assert_eq!(
            restored.turn(id, "unassigned").unwrap().measurements.len(),
            1
        );
        writeln!(
            fs::OpenOptions::new().append(true).open(&path).unwrap(),
            "{}",
            record("second")
        )
        .unwrap();
        let next = sync(&mut db, &key, &roots, false, Some(&restored), &mut caches)
            .unwrap()
            .unwrap();
        let change = next.publication_change.as_ref().unwrap();
        assert_eq!(
            (
                change.measurements_added,
                change.measurements_removed,
                change.measurements_changed
            ),
            (1, 0, 0)
        );
        assert_eq!(change.delta.tokens, Some(110));
        assert_eq!(next.turn(id, "unassigned").unwrap().measurements.len(), 2);
        assert_eq!(first.turn(id, "unassigned").unwrap().measurements.len(), 1);
    }

    #[test]
    fn persisted_aggregates_match_independent_source_scan_for_every_presentation() {
        use std::io::Write;
        let root = tempfile::tempdir().unwrap();
        let index = tempfile::tempdir().unwrap();
        source(root.path(), "a", 110);
        source(root.path(), "b", 220);
        let mut file = fs::OpenOptions::new()
            .append(true)
            .open(root.path().join("sessions/a.jsonl"))
            .unwrap();
        for (n, time, model, effort) in [
            (0, "2026-10-10T18:10:00Z", "gpt-5.4", "high"),
            (1, "2026-10-10T18:50:00Z", "unknown-model", "low"),
            (2, "2026-03-08T06:59:59Z", "gpt-5.4", "high"),
            (3, "2026-03-08T07:00:00Z", "gpt-5.4", "low"),
            (4, "2026-10-11", "gpt-5.4", "high"),
            (5, "2026-10-10T18:20:00Z", "gpt-5.4", "high"),
            (6, "2007-11-04T02:10:00Z", "gpt-5.4", "high"),
            (7, "2007-11-04T02:30:30Z", "gpt-5.4", "high"),
            (8, "2007-11-04T02:50:00Z", "gpt-5.4", "high"),
        ] {
            let turn = format!("turn-{n}");
            writeln!(file,"{}",json!({"timestamp":time,"type":"turn_context","payload":{"turn_id":turn,"model":model,"reasoning_effort":effort}})).unwrap();
            writeln!(file,"{}",json!({"timestamp":time,"type":"event_msg","payload":{"type":"token_usage_record","thread_id":"a","turn_id":turn,"response_id":format!("next-{n}"),"usage":{"input_tokens":100,"output_tokens":10,"total_tokens":110}}})).unwrap();
        }
        drop(file);
        let roots = vec![root.path().to_string_lossy().into_owned()];
        let key = source_key(&roots);
        let mut db = crate::live_index::open(&index.path().join("index.sqlite")).unwrap();
        let stored = sync(&mut db, &key, &roots, false, None, &mut BTreeMap::new())
            .unwrap()
            .unwrap();
        assert!(stored.has_stored_ledger());
        assert_eq!(stored.measurement_facts().count(), 0);
        let collected = crate::adapters::collect(
            &DiscoveryRequest {
                roots: vec![root.path().into()],
            },
            &RunContext::default(),
        );
        let direct = crate::usage_store::memory(
            collected,
            "live:independent:oracle".into(),
            crate::pricing_sync::current().unwrap(),
            None,
        )
        .unwrap();
        for entry in &stored.manifest.threads {
            for turn in entry.turns.keys() {
                let a = stored.turn(&entry.thread.id, turn).unwrap();
                let b = direct.turn(&entry.thread.id, turn).unwrap();
                assert_eq!(
                    serde_json::to_value(a.measurements).unwrap(),
                    serde_json::to_value(b.measurements).unwrap()
                );
            }
        }
        assert!(
            db.query_row(
                "SELECT count(*) FROM measurement_summaries WHERE json_array_length(targets)>1",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap()
                > 0
        );
        let mut queries = vec![];
        for timezone in ["UTC", "America/New_York", "Asia/Kathmandu"] {
            for action in ["usage", "threads"] {
                for scope in [
                    json!({"allTime":true}),
                    json!({"since":"2026-10-11","until":"2026-10-12"}),
                    json!({"project":"/synthetic/a","allTime":true}),
                    json!({"model":"unknown-model","allTime":true}),
                    json!({"undated":true}),
                ] {
                    let mut query = json!({"action":action,"scope":scope});
                    query["scope"]["timezone"] = json!(timezone);
                    queries.push(query);
                }
            }
        }
        for action in ["usage", "threads"] {
            queries.push(json!({"action":action,"scope":{"since":"2007-11-04","until":"2007-11-05","timezone":"America/St_Johns"}}));
        }
        for presentation in ["projects", "models", "distribution"] {
            queries.push(json!({"action":"usage","presentation":presentation,"scope":{"allTime":true,"timezone":"UTC"}}));
        }
        for sort in ["cost", "recent", "tokens"] {
            queries.push(json!({"action":"threads","sort":sort,"search":"a","scope":{"allTime":true,"timezone":"UTC"},"limit":1}));
        }
        for query in queries {
            let request: crate::usage_app_dto::Request =
                serde_json::from_value(query.clone()).unwrap();
            let a = serde_json::to_value(
                crate::usage_app::execute_snapshot(request.clone(), &stored).unwrap(),
            )
            .unwrap();
            let b =
                serde_json::to_value(crate::usage_app::execute_snapshot(request, &direct).unwrap())
                    .unwrap();
            for field in [
                "summary",
                "items",
                "scope",
                "availableRange",
                "page",
                "facets",
                "distribution",
            ] {
                assert_eq!(a.get(field), b.get(field), "{query}: {field}");
            }
        }
    }

    #[test]
    fn newly_written_frames_supply_exact_boundaries_without_another_payload_decode() {
        use std::io::Write;
        let root = tempfile::tempdir().unwrap();
        let index = tempfile::tempdir().unwrap();
        source(root.path(), "a", 110);
        let mut file = fs::OpenOptions::new()
            .append(true)
            .open(root.path().join("sessions/a.jsonl"))
            .unwrap();
        for (kind, time) in [
            ("task_started", "2026-10-01T00:00:00Z"),
            ("task_complete", "2026-10-01T00:00:02Z"),
        ] {
            writeln!(file, "{}", json!({"timestamp":time,"type":"event_msg","payload":{"type":kind,"turn_id":"turn"}})).unwrap();
        }
        let roots = vec![root.path().to_string_lossy().into_owned()];
        let key = source_key(&roots);
        let mut db = crate::live_index::open(&index.path().join("index.sqlite")).unwrap();
        let before = crate::live_index::decoded_event_count();
        let initial = sync(&mut db, &key, &roots, false, None, &mut BTreeMap::new())
            .unwrap()
            .unwrap();
        assert_eq!(crate::live_index::decoded_event_count(), before);
        let boundary = initial
            .manifest
            .events
            .partitions
            .iter()
            .find(|p| p.target.turn_id.is_some())
            .unwrap();
        assert_eq!(
            boundary.native_boundary.summary.state,
            crate::timing::analysis::State::Completed
        );
        assert_eq!(boundary.native_boundary.summary.candidates, 2);
        let decoded =
            crate::usage_store::cold_event_index_from_sources(&db, &initial.manifest.sources, None)
                .unwrap();
        assert!(crate::live_index::decoded_event_count() > before);
        assert_eq!(
            serde_json::to_value(&decoded).unwrap(),
            serde_json::to_value(&initial.manifest.events).unwrap()
        );
        let remaining: i64 = db
            .query_row("SELECT count(*) FROM written_event_blocks", [], |r| {
                r.get(0)
            })
            .unwrap();
        assert_eq!(remaining, 0);
    }

    #[test]
    fn append_reuses_unaffected_evidence_partitions_without_decoding_their_archive() {
        use std::io::Write;
        let root = tempfile::tempdir().unwrap();
        let index = tempfile::tempdir().unwrap();
        source(root.path(), "a", 110);
        source(root.path(), "b", 220);
        let mut large = fs::OpenOptions::new()
            .append(true)
            .open(root.path().join("sessions/b.jsonl"))
            .unwrap();
        for _ in 0..1000 {
            writeln!(large, "{}", json!({"type":"event_msg","payload":{"type":"user_message","message":"independent unchanged evidence"}})).unwrap();
        }
        let roots = vec![root.path().to_string_lossy().into_owned()];
        let key = source_key(&roots);
        let mut db = crate::live_index::open(&index.path().join("index.sqlite")).unwrap();
        let mut caches = BTreeMap::new();
        let initial = sync(&mut db, &key, &roots, false, None, &mut caches)
            .unwrap()
            .unwrap();
        let unchanged_thread = initial
            .manifest
            .threads
            .iter()
            .find(|t| t.thread.upstream_id == "b")
            .unwrap()
            .thread
            .id
            .as_str();
        let unchanged = initial
            .manifest
            .events
            .partitions
            .iter()
            .find(|p| p.target.thread_id.as_deref() == Some(unchanged_thread) && p.count >= 1000)
            .unwrap();
        let before = crate::live_index::decoded_event_count();
        writeln!(fs::OpenOptions::new().append(true).open(root.path().join("sessions/a.jsonl")).unwrap(), "{}",
            json!({"type":"event_msg","payload":{"type":"token_usage_record","thread_id":"a","turn_id":"turn","response_id":"second","usage":{"input_tokens":100,"output_tokens":10,"total_tokens":110}}})).unwrap();
        let next = sync(&mut db, &key, &roots, false, Some(&initial), &mut caches)
            .unwrap()
            .unwrap();
        assert!(crate::live_index::decoded_event_count() - before < 1000);
        let reused = next
            .manifest
            .events
            .partitions
            .iter()
            .find(|p| p.target == unchanged.target)
            .unwrap();
        assert!(Arc::ptr_eq(unchanged, reused));
        assert_eq!(
            next.ledger()
                .unwrap()
                .iter()
                .map(|r| r.fact.tokens.total.unwrap())
                .sum::<u64>(),
            440
        );
        let restored = restore(&db, &key, &roots).unwrap().unwrap();
        assert_eq!(
            serde_json::to_value(next.events().unwrap()).unwrap(),
            serde_json::to_value(restored.events().unwrap()).unwrap()
        );
    }

    #[test]
    fn restored_operations_are_cold_and_old_turns_remain_exact_after_result_correction() {
        use std::io::Write;
        let root = tempfile::tempdir().unwrap();
        let index = tempfile::tempdir().unwrap();
        source(root.path(), "a", 110);
        let path = root.path().join("sessions/a.jsonl");
        writeln!(fs::OpenOptions::new().append(true).open(&path).unwrap(), "{}",
            json!({"type":"response_item","payload":{"type":"function_call","call_id":"command","name":"exec_command","arguments":"{\"cmd\":\"pwd\"}"}})).unwrap();
        let roots = vec![root.path().to_string_lossy().into_owned()];
        let key = source_key(&roots);
        let mut db = crate::live_index::open(&index.path().join("index.sqlite")).unwrap();
        let initial = sync(&mut db, &key, &roots, false, None, &mut BTreeMap::new())
            .unwrap()
            .unwrap();
        let operation = initial.operation_facts().unwrap().remove(0);
        let thread = operation.thread_id.to_string();
        let turn = operation
            .turn_id
            .as_deref()
            .unwrap_or("unassigned")
            .to_owned();
        let before = serde_json::to_value(initial.turn(&thread, &turn).unwrap()).unwrap();
        let restored = super::restore(&db, &key, &roots).unwrap().unwrap();
        assert_eq!(restored.resident_operation_facts().count(), 0);
        assert_eq!(
            serde_json::to_value(restored.turn(&thread, &turn).unwrap()).unwrap(),
            before
        );
        writeln!(fs::OpenOptions::new().append(true).open(&path).unwrap(), "{}",
            json!({"type":"response_item","payload":{"type":"function_call_output","call_id":"command","output":{"exit_code":1}}})).unwrap();
        let next = sync(
            &mut db,
            &key,
            &roots,
            false,
            Some(&restored),
            &mut BTreeMap::new(),
        )
        .unwrap()
        .unwrap();
        assert_eq!(next.operation_facts().unwrap()[0].status.as_ref(), "failed");
        assert_eq!(
            serde_json::to_value(restored.turn(&thread, &turn).unwrap()).unwrap(),
            before
        );
        assert_eq!(
            serde_json::to_value(initial.turn(&thread, &turn).unwrap()).unwrap(),
            before
        );
    }

    #[test]
    fn unchanged_and_price_only_publications_do_not_decode_observations() {
        let root = tempfile::tempdir().unwrap();
        let index = tempfile::tempdir().unwrap();
        source(root.path(), "a", 110);
        let roots = vec![root.path().to_string_lossy().into_owned()];
        let key = source_key(&roots);
        let mut db = crate::live_index::open(&index.path().join("index.sqlite")).unwrap();
        let original = sync(&mut db, &key, &roots, false, None, &mut BTreeMap::new())
            .unwrap()
            .unwrap();
        let expected = serde_json::to_value(original.ledger().unwrap()).unwrap();
        let epoch = crate::live_index::load_map(
            &db,
            &format!("epoch:{}", original.manifest.sources[0].source.id),
        )
        .unwrap();
        for prior in [None, Some(original.as_ref())] {
            let before = crate::live_index::decoded_event_count();
            assert!(
                sync(&mut db, &key, &roots, false, prior, &mut BTreeMap::new())
                    .unwrap()
                    .is_none()
            );
            assert_eq!(crate::live_index::decoded_event_count(), before);
            // Force only the publication price identity to become stale. Source
            // cursors and epochs remain unchanged; no parser replay is needed.
            crate::live_index::put(
                &db,
                &format!("view:{key}"),
                "prices",
                "",
                &"synthetic-previous-price-catalog",
            )
            .unwrap();
            let current = sync(&mut db, &key, &roots, false, prior, &mut BTreeMap::new())
                .unwrap()
                .unwrap();
            assert_eq!(crate::live_index::decoded_event_count(), before);
            assert_eq!(
                serde_json::to_value(current.ledger().unwrap()).unwrap(),
                expected
            );
            assert_eq!(
                serde_json::to_value(&current.manifest.events).unwrap(),
                serde_json::to_value(&original.manifest.events).unwrap()
            );
            assert_eq!(
                crate::live_index::load_map(
                    &db,
                    &format!("epoch:{}", original.manifest.sources[0].source.id),
                )
                .unwrap(),
                epoch
            );
        }
    }

    #[test]
    fn cold_evidence_is_version_bound_after_append_and_restore_does_no_decoding() {
        let root = tempfile::tempdir().unwrap();
        let index = tempfile::tempdir().unwrap();
        source(root.path(), "a", 110);
        let roots = vec![root.path().to_string_lossy().into_owned()];
        let key = source_key(&roots);
        let mut db = crate::live_index::open(&index.path().join("index.sqlite")).unwrap();
        let mut caches = BTreeMap::new();
        let original = sync(&mut db, &key, &roots, false, None, &mut caches)
            .unwrap()
            .unwrap();
        let events = serde_json::to_value(original.events().unwrap()).unwrap();
        let before = crate::live_index::decoded_event_count();
        let restored = restore(&db, &key, &roots).unwrap().unwrap();
        assert_eq!(crate::live_index::decoded_event_count(), before);
        assert_eq!(restored.ledger().unwrap()[0].fact.tokens.total, Some(110));
        assert_eq!(crate::live_index::decoded_event_count(), before);
        use std::io::Write;
        let mut file = fs::OpenOptions::new()
            .append(true)
            .open(root.path().join("sessions/a.jsonl"))
            .unwrap();
        writeln!(file, "{}", json!({"type":"event_msg","payload":{"type":"token_usage_record","thread_id":"a","turn_id":"turn","response_id":"second","usage":{"input_tokens":210,"output_tokens":10,"total_tokens":220}}})).unwrap();
        let current = sync(&mut db, &key, &roots, false, Some(&restored), &mut caches)
            .unwrap()
            .unwrap();
        assert_eq!(
            current
                .ledger()
                .unwrap()
                .iter()
                .map(|r| r.fact.tokens.total.unwrap())
                .sum::<u64>(),
            330
        );
        assert_eq!(
            serde_json::to_value(original.events().unwrap()).unwrap(),
            events
        );
        assert_eq!(
            serde_json::to_value(restored.events().unwrap()).unwrap(),
            events
        );
        assert!(current.events().unwrap().len() > original.events().unwrap().len());
        let reload = restore(&db, &key, &roots).unwrap().unwrap();
        assert_eq!(
            serde_json::to_value(reload.events().unwrap()).unwrap(),
            serde_json::to_value(current.events().unwrap()).unwrap()
        );
    }

    #[test]
    fn cold_statistics_skip_corrupt_unrelated_evidence_but_metadata_versions_fail_before_publish() {
        let root = tempfile::tempdir().unwrap();
        let index = tempfile::tempdir().unwrap();
        source(root.path(), "a", 110);
        source(root.path(), "b", 220);
        let roots = vec![root.path().to_string_lossy().into_owned()];
        let key = source_key(&roots);
        let mut db = crate::live_index::open(&index.path().join("index.sqlite")).unwrap();
        let original = sync(&mut db, &key, &roots, false, None, &mut BTreeMap::new())
            .unwrap()
            .unwrap();
        let bad = original
            .manifest
            .threads
            .iter()
            .find(|t| t.thread.upstream_id == "b")
            .unwrap()
            .thread
            .id
            .clone();
        let block: i64 = db
            .query_row(
                "SELECT rowid FROM event_blocks WHERE json_extract(target,'$[0]')=?1 LIMIT 1",
                [&bad],
                |r| r.get(0),
            )
            .unwrap();
        db.execute(
            "UPDATE event_blocks SET data=zeroblob(length(data)) WHERE rowid=?1",
            [block],
        )
        .unwrap();
        let before = crate::live_index::decoded_event_count();
        let restored = restore(&db, &key, &roots).unwrap().unwrap();
        let query =
            serde_json::from_value(json!({"action":"usage","scope":{"allTime":true}})).unwrap();
        assert_eq!(
            crate::usage_app::execute_snapshot(query, &restored)
                .unwrap()
                .summary
                .tokens
                .total,
            Some(330)
        );
        assert_eq!(crate::live_index::decoded_event_count(), before);
        let partition = restored
            .manifest
            .events
            .partitions
            .iter()
            .find(|p| p.target.thread_id.as_deref() == Some(&bad))
            .unwrap();
        let error = restored
            .event_page(
                &partition.target,
                1,
                None,
                Default::default(),
                &std::sync::atomic::AtomicBool::new(false),
            )
            .unwrap_err();
        assert_eq!(crate::live_index::failure_code(&error), "INDEX_UNAVAILABLE");
        // The old fixed view still reads its verified prior blocks.
        assert!(original.events().is_ok());
        db.execute("UPDATE event_blocks SET codec=99 WHERE rowid=?1", [block])
            .unwrap();
        let error = restore_projects(
            &db,
            &key,
            &roots,
            || None,
            |_| panic!("future codec must not publish"),
        )
        .err()
        .unwrap();
        assert_eq!(
            crate::live_index::failure_code(&error),
            "INDEX_UNSUPPORTED_VERSION"
        );
    }
    #[test]
    fn requested_project_is_readable_before_later_projects_without_changing_committed_facts() {
        let source = tempfile::tempdir().unwrap();
        let index = tempfile::tempdir().unwrap();
        fs::create_dir(source.path().join("sessions")).unwrap();
        for (name, project, tokens) in [("a", "/synthetic/a", 110), ("b", "/synthetic/b", 220)] {
            let rows = [
                json!({"type":"session_meta","payload":{"id":name,"cwd":project}}),
                json!({"type":"turn_context","payload":{"turn_id":"turn","model":"gpt-5.4"}}),
                json!({"type":"event_msg","payload":{"type":"token_usage_record","thread_id":name,"turn_id":"turn","response_id":"response","usage":{"input_tokens":tokens-10,"output_tokens":10,"total_tokens":tokens}}}),
            ];
            fs::write(
                source.path().join(format!("sessions/{name}.jsonl")),
                rows.iter()
                    .map(|v| v.to_string() + "\n")
                    .collect::<String>(),
            )
            .unwrap();
        }
        let roots = vec![source.path().to_string_lossy().into_owned()];
        let key = source_key(&roots);
        let mut db = crate::live_index::open(&index.path().join("index.sqlite")).unwrap();
        let committed = sync(&mut db, &key, &roots, false, None, &mut BTreeMap::new())
            .unwrap()
            .unwrap();
        let original = crate::live_index::load_map(&db, &format!("view:{key}")).unwrap();
        db.pragma_update(None, "query_only", true).unwrap();
        let mut batches = vec![];
        let decoded_before = crate::live_index::decoded_event_count();
        let restored = restore_projects(
            &db,
            &key,
            &roots,
            || Some("/synthetic/b".into()),
            |view| batches.push(view),
        )
        .unwrap()
        .unwrap();
        assert_eq!(
            crate::live_index::decoded_event_count(),
            decoded_before,
            "restoration must not decode observations"
        );
        let first = &batches[0];
        assert!(Arc::ptr_eq(&first.summary_index, &restored.summary_index));
        let shared = restored
            .manifest
            .threads
            .iter()
            .find(|t| t.thread.id == first.manifest.threads[0].thread.id)
            .unwrap();
        assert!(Arc::ptr_eq(&first.manifest.threads[0], shared));
        for partition in &first.manifest.events.partitions {
            let shared = restored
                .manifest
                .events
                .partitions
                .iter()
                .find(|p| p.target == partition.target)
                .unwrap();
            assert!(Arc::ptr_eq(partition, shared));
        }
        assert!(incomplete(first));
        assert_eq!(first.manifest.threads.len(), 1);
        assert_eq!(
            first.manifest.threads[0].thread.project.as_deref(),
            Some("/synthetic/b")
        );
        assert_eq!(
            first
                .ledger()
                .unwrap()
                .iter()
                .map(|r| r.fact.tokens.total.unwrap())
                .sum::<u64>(),
            220
        );
        assert!(
            first
                .project_loads
                .iter()
                .any(|p| p.project.as_deref() == Some("/synthetic/a")
                    && p.state != ProjectLoadState::Ready)
        );
        assert!(first.manifest.sources.iter().all(|s| s.status == "partial"));
        let query = |project: &str| {
            serde_json::from_value(
                json!({"action":"usage","scope":{"allTime":true,"project":project}}),
            )
            .unwrap()
        };
        let ready = crate::usage_app::execute_snapshot(query("/synthetic/b"), first).unwrap();
        assert_eq!(ready.summary.tokens.total, Some(220));
        let pending = crate::usage_app::execute_snapshot(query("/synthetic/a"), first).unwrap_err();
        assert_eq!(
            pending
                .downcast_ref::<crate::dto::OperationError>()
                .unwrap()
                .code,
            "SYNC_PENDING"
        );
        assert_eq!(
            restored
                .ledger()
                .unwrap()
                .iter()
                .map(|r| r.fact.tokens.total.unwrap())
                .sum::<u64>(),
            330
        );
        assert_eq!(
            restored.events().unwrap().len(),
            committed.events().unwrap().len()
        );
        assert_eq!(
            serde_json::to_value(&restored.manifest.events).unwrap(),
            serde_json::to_value(&committed.manifest.events).unwrap()
        );
        let normalize = |snapshot: &Snapshot| {
            let mut manifest = serde_json::to_value(&snapshot.manifest).unwrap();
            for (field, key) in [("watermarks", "fileId"), ("threads", "thread")] {
                manifest[field]
                    .as_array_mut()
                    .unwrap()
                    .sort_by_key(|v| v[key].to_string());
            }
            manifest
        };
        assert_eq!(normalize(&restored), normalize(&committed));
        assert_eq!(
            crate::live_index::load_map(&db, &format!("view:{key}")).unwrap(),
            original
        );
        let changes = db.total_changes();
        restore_projects(&db, &key, &roots, || None, |_| {}).unwrap();
        assert_eq!(db.total_changes(), changes);
    }
}
