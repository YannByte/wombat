use super::*;
fn fixture() -> Collected {
    let thread: Thread = Thread {
        id: "thread-a".into(),
        agent_kind: "test".into(),
        source_instance_id: "source-a".into(),
        upstream_id: "a".into(),
        title: Some("synthetic".into()),
        project: None,
        started_at: None,
        last_activity_at: None,
    };
    let turn = Turn {
        id: "turn-a".into(),
        thread_id: thread.id.clone(),
        upstream_id: "a".into(),
        ordinal: 1,
        started_at: None,
        ended_at: None,
        last_activity_at: None,
        status: "completed".into(),
    };
    let record = Measurement {
        id: "measurement-a".into(),
        agent_kind: "test".into(),
        source_instance_id: "source-a".into(),
        thread_id: Some(thread.id.as_str().into()),
        turn_id: Some(turn.id.as_str().into()),
        response_id: None,
        timestamp: Some("2026-09-29T01:00:00Z".into()),
        interval_end: None,
        grain: "response".into(),
        time_precision: "second".into(),
        model: ModelRef {
            raw: Some("gpt-5.3-codex".into()),
            provider: Some("openai".into()),
            api_provider: Some("openai".into()),
            pricing_model: None,
        },
        reasoning_effort: None,
        tokens: TokenUsage {
            input: Some(100),
            cache_read: Some(0),
            cache_create: Some(0),
            output: Some(10),
            reasoning: None,
            total: Some(110),
            raw_input: Some(100),
        },
        token_unavailable_reasons: TokenFields {
            reasoning: Some(TokenUnavailableReason::Missing),
            ..TokenFields::default()
        },
        pricing_context_conflict: false,
        request_scoped: true,
        reported_cost: None,
        service_tier: None,
        sequence: 1,
        evidence: vec![],
    };
    Collected {
        threads: vec![thread],
        turns: vec![turn],
        measurements: vec![record.into()],
        ..Default::default()
    }
}
#[test]
fn equal_prices_share_storage_without_merging_measurements_or_distinct_inputs() {
    let root = tempfile::tempdir().unwrap();
    let prices = crate::pricing_sync::current_at(root.path()).unwrap();
    let mut facts = fixture();
    for index in 0..5 {
        let mut fact = facts.measurements[0].as_ref().clone();
        fact.id = format!("copy-{index}");
        if index == 2 {
            fact.request_scoped = false;
        }
        if index == 3 {
            fact.tokens.output = None;
            fact.token_unavailable_reasons.output = Some(TokenUnavailableReason::Missing);
        }
        if index == 4 {
            fact.tokens.output = Some(0);
        }
        facts.measurements.push(fact.into());
    }
    let snapshot = memory(facts, "live:prices".into(), prices.clone(), None).unwrap();
    let rows = snapshot.live_ledger().unwrap();
    assert_eq!(rows.len(), 6);
    let by_id = |id: &str| rows.iter().find(|r| r.fact.id == id).unwrap().price.clone();
    assert!(Arc::ptr_eq(&by_id("copy-0"), &by_id("copy-1")));
    for id in ["copy-2", "copy-3", "copy-4"] {
        assert!(!Arc::ptr_eq(&by_id("copy-0"), &by_id(id)));
    }
    for row in rows {
        let expected = pricing::price_with_catalog(
            &row.fact.model,
            &row.fact.tokens,
            &PricingContext {
                request_scoped: row.fact.request_scoped,
                model_conflicted: row.fact.pricing_context_conflict,
            },
            &prices.catalog,
            &prices.catalog_hash,
        );
        assert_eq!(*row.price, expected);
    }
}

#[test]
fn persisted_ledger_rejects_inconsistent_token_observations() {
    let root = tempfile::tempdir().unwrap();
    let mut snapshot = save_at(root.path(), fixture()).unwrap();
    let mut rows = snapshot.ledger().unwrap();
    Arc::make_mut(&mut rows[0].fact)
        .token_unavailable_reasons
        .total = Some(TokenUnavailableReason::Missing);
    snapshot.manifest.ledger =
        super::files::save_json(&snapshot.directory, "ledger.json", &rows).unwrap();
    assert!(
        snapshot
            .ledger()
            .unwrap_err()
            .to_string()
            .contains("计量观察无效")
    );
}

#[test]
fn single_measurement_query_rejects_invalid_price_parts_but_keeps_request_basis() {
    let root = tempfile::tempdir().unwrap();
    let prices = crate::pricing_sync::current_at(root.path()).unwrap();
    let snapshot = memory(fixture(), "live:parts".into(), prices, None).unwrap();
    let rows = snapshot.live_ledger().unwrap();
    let summary = crate::usage_app::summarize(&rows).unwrap();
    assert_eq!(summary.price.basis, rows[0].price.basis);
    let mut damaged = rows[0].clone();
    Arc::make_mut(&mut Arc::make_mut(&mut damaged.price).components[0]).status = "bad".into();
    assert!(crate::usage_app::summarize(&[&damaged]).is_err());
    assert!(crate::usage_app::summarize(&rows).is_ok());
}
#[test]
fn recent_turns_use_reliable_events_instead_of_ordinals_or_token_volume() {
    let root = tempfile::tempdir().unwrap();
    let prices = crate::pricing_sync::current_at(root.path()).unwrap();
    let mut facts = fixture();
    let mut recent = facts.turns[0].clone();
    recent.id = "recent".into();
    recent.upstream_id = "recent".into();
    recent.ordinal = 0;
    recent.started_at = Some("2026-09-28T00:00:00Z".into());
    recent.last_activity_at = Some("2026-09-30T00:00:00Z".into());
    let mut unknown = facts.turns[0].clone();
    unknown.id = "unknown".into();
    unknown.ordinal = 999;
    facts.turns.extend([recent, unknown]);
    let snapshot = memory(facts, "live:recent".into(), prices, None).unwrap();
    let request = serde_json::from_value(serde_json::json!({"action":"turns","threadId":"thread-a","sort":"recent","scope":{"allTime":true,"timezone":"UTC"}})).unwrap();
    let result = crate::usage_app::execute_snapshot(request, &snapshot).unwrap();
    let ids: Vec<_> = result
        .items
        .iter()
        .map(|i| match i {
            crate::usage_app_dto::Item::Turn { id, .. } => id.as_str(),
            _ => panic!(),
        })
        .collect();
    assert_eq!(ids, ["recent", "turn-a", "unknown"]);
}
#[test]
fn query_cache_is_revision_scoped_and_details_borrow_the_same_facts() {
    let root = tempfile::tempdir().unwrap();
    let prices = crate::pricing_sync::current_at(root.path()).unwrap();
    let mut facts = fixture();
    let first = memory(facts.clone(), "live:test:one".into(), prices.clone(), None).unwrap();
    let request = || {
        serde_json::from_value(serde_json::json!({"action":"usage","scope":{"since":"2026-09-29","until":"2026-09-30","timezone":"UTC"}})).unwrap()
    };
    let before = crate::usage_app::execute_snapshot(request(), &first).unwrap();
    let again = crate::usage_app::execute_snapshot(request(), &first).unwrap();
    assert_eq!(
        serde_json::to_value(&before).unwrap(),
        serde_json::to_value(&again).unwrap()
    );
    let fact = Arc::make_mut(&mut facts.measurements[0]);
    fact.tokens.total = Some(210);
    fact.tokens.input = Some(200);
    fact.tokens.raw_input = Some(200);
    let second = memory(facts, "live:test:two".into(), prices, Some(&first)).unwrap();
    let after = crate::usage_app::execute_snapshot(request(), &second).unwrap();
    assert_ne!(
        serde_json::to_value(&before.summary).unwrap(),
        serde_json::to_value(&after.summary).unwrap()
    );
    assert_eq!(
        serde_json::to_value(&before).unwrap(),
        serde_json::to_value(crate::usage_app::execute_snapshot(request(), &first).unwrap())
            .unwrap()
    );
    let rows = second
        .live_detail_rows("thread-a", Some("turn-a"))
        .unwrap()
        .unwrap();
    assert_eq!(rows[0].fact.tokens.total, Some(210));
    assert!(std::ptr::eq(
        rows[0],
        second.live_rows.as_ref().unwrap()[0].as_ref()
    ));
    assert!(second.live_detail_rows("missing", None).is_err());
    assert!(
        second
            .live_detail_rows("thread-a", Some("missing"))
            .is_err()
    );
}
#[test]
fn compact_live_indices_preserve_order_and_old_revision() {
    let root = tempfile::tempdir().unwrap();
    let prices = crate::pricing_sync::current_at(root.path()).unwrap();
    let mut collected = fixture();
    let mut other = collected.measurements[0].as_ref().clone();
    other.id = "measurement-z".into();
    collected.measurements.insert(0, other.into());
    let first = memory(collected.clone(), "live:a:one".into(), prices.clone(), None).unwrap();
    let old = first.turn("thread-a", "turn-a").unwrap();
    assert_eq!(
        old.measurements
            .iter()
            .map(|r| r.fact.tokens.total.unwrap())
            .sum::<u64>(),
        220
    );
    let fact = Arc::make_mut(&mut collected.measurements[0]);
    fact.tokens.input = Some(200);
    fact.tokens.raw_input = Some(200);
    fact.tokens.total = Some(210);
    let second = memory(collected.clone(), "live:a:two".into(), prices, Some(&first)).unwrap();
    let new = second.turn("thread-a", "turn-a").unwrap();
    assert_eq!(
        new.measurements
            .iter()
            .map(|r| r.fact.tokens.total.unwrap())
            .sum::<u64>(),
        320
    );
    assert_eq!(
        first.turn("thread-a", "turn-a").unwrap().measurements[1]
            .fact
            .tokens
            .total,
        Some(110)
    );
    let a = first.live_rows.as_ref().unwrap();
    let b = second.live_rows.as_ref().unwrap();
    assert!(Arc::ptr_eq(&a[0], &b[0]));
    assert!(Arc::ptr_eq(&collected.measurements[0], &b[1].fact));
}

#[test]
fn refresh_response_uses_captured_rows_without_reading_or_building_a_report() {
    let root = tempfile::tempdir().unwrap();
    let snapshot = memory(
        fixture(),
        "live:refresh".into(),
        crate::pricing_sync::current_at(root.path()).unwrap(),
        None,
    )
    .unwrap();
    let rows = snapshot.live_ledger().unwrap();
    let result = crate::usage_app::refresh_response(&snapshot, &rows).unwrap();
    assert_eq!(result.action, crate::usage_app_dto::Action::Refresh);
    assert_eq!(result.summary.tokens.total, Some(110));
    assert_eq!(result.summary.measurement_count, 1);
    assert_eq!(result.available_range.since.as_deref(), Some("2026-09-29"));
    assert_eq!(result.available_range.until.as_deref(), Some("2026-09-30"));
    assert_eq!(result.snapshot_ref.snapshot_id, "live:refresh");
    assert!(result.items.is_empty());
    assert!(result.facets.is_none());
    assert!(result.distribution.is_none());
    assert_eq!(result.page.total, 0);
}

#[test]
fn immutable_generation_and_exact_turn_read() {
    let root = tempfile::tempdir().unwrap();
    let old = save_at(root.path(), fixture()).unwrap();
    let old_id = old.manifest.snapshot_ref.snapshot_id.clone();
    let next = save_at(root.path(), Collected::default()).unwrap();
    assert_ne!(old_id, next.manifest.snapshot_ref.snapshot_id);
    assert_eq!(
        load_at(root.path(), Some(&old_id))
            .unwrap()
            .ledger()
            .unwrap()[0]
            .fact
            .tokens
            .total,
        Some(110)
    );
    assert_eq!(
        load_at(root.path(), None)
            .unwrap()
            .manifest
            .snapshot_ref
            .snapshot_id,
        next.manifest.snapshot_ref.snapshot_id
    );
    let turn = old.turn("thread-a", "turn-a").unwrap();
    assert_eq!(turn.measurements.len(), 1);
    // A target-turn read does not access the global ledger.
    fs::remove_file(old.directory.join("ledger.json")).unwrap();
    assert_eq!(
        old.turn("thread-a", "turn-a").unwrap().measurements[0]
            .fact
            .id,
        "measurement-a"
    );
    assert!(old.ledger().is_err());
}
#[test]
fn corruption_uncommitted_and_refresh_busy_are_detected() {
    let root = tempfile::tempdir().unwrap();
    let lock = RefreshLock::at(root.path()).unwrap();
    assert!(RefreshLock::at(root.path()).is_err());
    drop(lock);
    assert!(RefreshLock::at(root.path()).is_ok());
    let snapshot = save_at(root.path(), fixture()).unwrap();
    let latest = fs::read(root.path().join("latest.json")).unwrap();
    private_dir(&root.path().join("generations/uncommitted/.pending-a")).unwrap();
    assert!(load_at(root.path(), Some("uncommitted")).is_err());
    assert_eq!(fs::read(root.path().join("latest.json")).unwrap(), latest);
    let path = snapshot
        .directory
        .join(&snapshot.manifest.threads[0].file.file);
    fs::write(path, b"{}").unwrap();
    assert!(snapshot.turn("thread-a", "turn-a").is_err());
}

fn safe_event() -> Arc<crate::session_events::Event> {
    use crate::session_events::*;
    Arc::new(
        Event::new(
            Position {
                source_instance_id: "source-a".into(),
                file_id: "file-a".into(),
                generation: "generation-a".into(),
                byte_offset: 123,
                ordinal: 0,
            },
            Some("thread-a".into()),
            Some("turn-a".into()),
            Time::from_source(None).0,
            vec![],
            Payload::Lifecycle {
                lifecycle: LifecycleKind::Turn,
                phase: Phase::Completed,
                native_id: Some("a".into()),
                duration_ms: Some(0),
                first_token_ms: None,
            },
        )
        .unwrap(),
    )
}

#[test]
fn safe_events_survive_memory_and_fixed_snapshot_without_turn_duplication() {
    let root = tempfile::tempdir().unwrap();
    let mut facts = fixture();
    let event = safe_event();
    facts.events.push(event.clone());
    let prices = crate::pricing_sync::current_at(root.path()).unwrap();
    let live = memory(facts.clone(), "live:events".into(), prices, None).unwrap();
    assert!(Arc::ptr_eq(&live.events().unwrap()[0], &event));
    let saved = save_at(root.path(), facts).unwrap();
    let restored = load_at(root.path(), Some(&saved.manifest.snapshot_ref.snapshot_id)).unwrap();
    assert_eq!(
        serde_json::to_value(restored.events().unwrap()).unwrap(),
        serde_json::to_value(live.events().unwrap()).unwrap()
    );
    assert!(
        !serde_json::to_string(&restored.turn("thread-a", "turn-a").unwrap())
            .unwrap()
            .contains(event.id())
    );
    fs::write(
        restored
            .directory
            .join(&restored.manifest.events.partitions[0].chunks[0].file.file),
        "[]",
    )
    .unwrap();
    assert!(
        restored
            .events()
            .unwrap_err()
            .to_string()
            .contains("事件分片校验失败")
    );
}

#[test]
fn duplicate_events_and_previous_snapshot_format_are_rejected() {
    let root = tempfile::tempdir().unwrap();
    let mut facts = fixture();
    facts.events = vec![safe_event(), safe_event()];
    assert!(save_at(root.path(), facts.clone()).is_err());
    let prices = crate::pricing_sync::current_at(root.path()).unwrap();
    assert!(memory(facts, "live:duplicate".into(), prices, None).is_err());
    assert!(!root.path().join("latest.json").exists());
    let saved = save_at(root.path(), fixture()).unwrap();
    let path = saved.directory.join("manifest.json");
    let mut old = serde_json::to_value(&saved.manifest).unwrap();
    old["schemaVersion"] = serde_json::json!(3);
    fs::write(&path, serde_json::to_vec(&old).unwrap()).unwrap();
    assert!(
        load_at(root.path(), None)
            .err()
            .unwrap()
            .to_string()
            .contains("不支持此快照版本")
    );
    assert_eq!(
        serde_json::from_slice::<serde_json::Value>(&fs::read(path).unwrap()).unwrap(),
        old
    );
}

#[test]
fn explicit_previous_directory_snapshot_is_rejected_without_touching_it() {
    let home = tempfile::tempdir().unwrap();
    let old = home.path().join("usage-v3/generations/old-id/committed");
    fs::create_dir_all(&old).unwrap();
    let path = old.join("manifest.json");
    fs::write(&path, "retained old data").unwrap();
    assert!(
        load_at(&home.path().join("usage-v4"), Some("old-id"))
            .err()
            .unwrap()
            .to_string()
            .contains("不支持此快照版本")
    );
    assert_eq!(fs::read_to_string(path).unwrap(), "retained old data");
    assert!(
        load_at(&home.path().join("usage-v4"), None)
            .err()
            .unwrap()
            .to_string()
            .contains("尚无用量数据")
    );
}
