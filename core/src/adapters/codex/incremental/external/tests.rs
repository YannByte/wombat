use super::*;
use serde_json::json;
fn report() -> SourceReport {
    SourceReport {
        source: SourceInstance {
            id: "synthetic".into(),
            agent_kind: "codex".into(),
            root: "/synthetic".into(),
        },
        adapter_version: VERSION.into(),
        source_versions: vec![],
        capabilities: CodexAdapter.descriptor().capabilities,
        status: "complete".into(),
        files_read: 0,
        bytes_read: 0,
        issues: vec![],
    }
}
fn candidate(
    n: usize,
    thread: Option<&str>,
    direct: bool,
    start: Option<u64>,
    end: Option<u64>,
) -> Candidate {
    let mut measurement:Measurement=serde_json::from_value(json!({"id":format!("m{n:06}"),"agentKind":"synthetic","sourceInstanceId":"synthetic","grain":"response","timePrecision":"unknown","model":{},"tokens":{"total":1},"tokenUnavailableReasons":{"input":"missing","cacheRead":"missing","cacheCreate":"missing","output":"missing","reasoning":"missing","total":null,"rawInput":"missing"},"pricingContextConflict":false,"requestScoped":direct,"sequence":n,"evidence":[{"file":"/synthetic/measurements","line":n}]})).unwrap();
    measurement.thread_id = thread.map(Arc::from);
    measurement.turn_id = (!n.is_multiple_of(5)).then(|| Arc::from(format!("turn-{}", n % 9)));
    Candidate {
        measurement: Arc::new(measurement),
        direct,
        cumulative: end,
        interval_start: start,
        fingerprint: format!("fingerprint-{}", n % 37),
    }
}
#[test]
fn external_counter_reconciliation_matches_exact_reference_with_cycles_unknowns_and_u64_ranges() {
    let root = tempfile::tempdir().unwrap();
    let db = crate::live_index::open(&root.path().join("index.sqlite")).unwrap();
    prepare(&db).unwrap();
    let cancelled = std::sync::atomic::AtomicBool::new(false);
    let mut facts = Facts {
        parents: HashMap::from([
            ("b".into(), "a".into()),
            ("c".into(), "b".into()),
            ("x".into(), "y".into()),
            ("y".into(), "x".into()),
        ]),
        ..Facts::default()
    };
    let owners = [
        Some("a"),
        Some("b"),
        Some("c"),
        Some("x"),
        Some("unrelated"),
        None,
    ];
    for n in 0_usize..1200 {
        let start = if n.is_multiple_of(11) {
            u64::MAX - 1000 + (n % 300) as u64
        } else {
            (n % 30) as u64 * 10
        };
        let end = start + (n % 8) as u64 * 4;
        let value = candidate(
            n,
            owners[n % owners.len()],
            n.is_multiple_of(3),
            (!n.is_multiple_of(13)).then_some(start),
            (!n.is_multiple_of(17)).then_some(end),
        );
        stage_candidate(
            &db,
            &value.measurement.id,
            &MeasurementState {
                candidate: value.clone(),
                issues: vec![],
            },
        )
        .unwrap();
        facts
            .measurements
            .insert(value.measurement.id.clone(), value);
    }
    let forest =
        session_relations::SessionRelations::new_cancellable(&facts.parents, &cancelled).unwrap();
    let inherited = facts.inherited_measurements(&forest, &cancelled).unwrap();
    let mut expected_report = report();
    let covered = facts
        .direct_exclusions(&inherited, &mut expected_report, &cancelled)
        .unwrap();
    let expected: BTreeMap<_, _> = facts
        .measurements
        .iter()
        .filter(|(id, _)| !inherited.contains(id.as_str()) && !covered.contains(id.as_str()))
        .map(|(id, c)| (id.clone(), serde_json::to_value(&c.measurement).unwrap()))
        .collect();
    let mut actual_report = report();
    accounting::project(&db, &forest, &mut actual_report, &cancelled).unwrap();
    let mut actual = BTreeMap::new();
    crate::live_index::each_located(
        &db,
        "projection:synthetic",
        Some("measurements"),
        |_, id, payload, _| {
            actual.insert(
                id.to_owned(),
                serde_json::from_str::<serde_json::Value>(payload)?,
            );
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(actual, expected);
    assert_eq!(
        serde_json::to_value(actual_report.issues).unwrap(),
        serde_json::to_value(expected_report.issues).unwrap()
    );
    assert!(
        actual.values().any(|v| v["threadId"].is_null()),
        "missing owner remains distinct"
    );
}
#[test]
fn recovery_header_never_loads_domain_bodies_and_missing_blocks_fail_without_fallback() {
    let root = tempfile::tempdir().unwrap();
    let db = crate::live_index::open(&root.path().join("index.sqlite")).unwrap();
    prepare(&db).unwrap();
    let source = report().source;
    let value = candidate(0, Some("thread"), true, None, None);
    let id = value.measurement.id.clone();
    stage_candidate(
        &db,
        &id,
        &MeasurementState {
            candidate: value,
            issues: vec![],
        },
    )
    .unwrap();
    resume::save(&db, &source, &Facts::default()).unwrap();
    save(&db, &source).unwrap();
    clear(&db).unwrap();
    let metadata = resume::load(&db, &source).unwrap().unwrap();
    assert!(
        metadata.measurements.is_empty()
            && metadata.operations.is_empty()
            && metadata.events.is_empty()
    );
    restore(&db, &source, &std::sync::atomic::AtomicBool::new(false)).unwrap();
    clear(&db).unwrap();
    crate::live_index::remove(&db, &resume::scope(&source), "candidates", "m000000").unwrap();
    assert!(restore(&db, &source, &std::sync::atomic::AtomicBool::new(false)).is_err());
    crate::live_index::put(&db, &resume::scope(&source), "version", "", &999).unwrap();
    assert_eq!(
        resume::validate(&db, &source)
            .unwrap_err()
            .downcast_ref::<crate::dto::OperationError>()
            .unwrap()
            .code,
        "UNSUPPORTED_VERSION"
    );
}

#[test]
fn append_reopens_only_affected_conversation_and_retains_other_canonical_blocks() {
    use std::io::Write;
    let root = tempfile::tempdir().unwrap();
    let index = tempfile::tempdir().unwrap();
    fs::create_dir(root.path().join("sessions")).unwrap();
    for n in 0..64 {
        let mut rows = vec![
            json!({"type":"session_meta","payload":{"id":format!("thread-{n}")}}),
            json!({"type":"turn_context","payload":{"turn_id":"turn","model":"gpt-5.4"}}),
        ];
        for r in 0..4 {
            rows.push(json!({"type":"event_msg","payload":{"type":"token_usage_record","response_id":format!("response-{r}"),"usage":{"input_tokens":100,"output_tokens":10,"total_tokens":110}}}));
            rows.push(json!({"type":"response_item","payload":{"type":"function_call","name":"read_file","call_id":format!("call-{r}"),"arguments":"{}"}}));
        }
        fs::write(
            root.path().join(format!("sessions/{n}.jsonl")),
            rows.iter()
                .map(|r| r.to_string() + "\n")
                .collect::<String>(),
        )
        .unwrap();
    }
    let source = CodexAdapter
        .discover(&DiscoveryRequest {
            roots: vec![root.path().into()],
        })
        .sources
        .remove(0);
    let db = crate::live_index::open(&index.path().join("index.sqlite")).unwrap();
    let mut cache = Cache::default();
    let before = sync_cached(&db, &source, false, &mut cache)
        .unwrap()
        .unwrap()
        .collected;
    assert_eq!(before.measurements.len(), 256);
    assert_eq!(before.operations.len(), 256);
    let blocks:Vec<(i64,String)>=db.prepare("SELECT f.id,f.digest FROM fact_blocks f JOIN buckets b ON b.id=f.bucket WHERE b.scope=?1 AND b.field='operations' ORDER BY f.id").unwrap().query_map([format!("projection:{}",source.id)],|r|Ok((r.get(0)?,r.get(1)?))).unwrap().collect::<rusqlite::Result<_>>().unwrap();
    crate::live_index::begin_event_changes(&db).unwrap();
    writeln!(fs::OpenOptions::new().append(true).open(root.path().join("sessions/0.jsonl")).unwrap(),"{}",json!({"type":"event_msg","payload":{"type":"token_usage_record","response_id":"new","usage":{"input_tokens":200,"output_tokens":10,"total_tokens":210}}})).unwrap();
    let decoded = crate::live_index::decoded_event_count();
    let after = sync_cached(&db, &source, false, &mut Cache::default())
        .unwrap()
        .unwrap()
        .collected;
    assert!(
        crate::live_index::decoded_event_count() - decoded < 32,
        "an append must not decode unrelated archive targets"
    );
    assert_eq!(after.operations, before.operations);
    assert_eq!(after.measurements.len(), 257);
    assert_eq!(cache.encoded_bytes, 0);
    for (block, digest) in blocks {
        assert_eq!(
            db.query_row("SELECT digest FROM fact_blocks WHERE id=?1", [block], |r| r
                .get::<_, String>(0))
                .unwrap(),
            digest
        );
    }
    assert_eq!(
        db.query_row("SELECT count(*) FROM codex_candidates", [], |r| r
            .get::<_, i64>(0))
            .unwrap(),
        0,
        "the completed update releases its disk working relation"
    );
}

#[test]
fn component_references_preserve_collision_fallback_and_fail_closed_when_missing() {
    let root = tempfile::tempdir().unwrap();
    let db = crate::live_index::open(&root.path().join("index.sqlite")).unwrap();
    prepare(&db).unwrap();
    let cancelled = std::sync::atomic::AtomicBool::new(false);
    for (n, thread) in ["a", "b"].into_iter().enumerate() {
        let operation: Operation = serde_json::from_value(json!({
            "id":"duplicate","threadId":thread,"callId":format!("call-{n}"),
            "kind":"command","name":"synthetic","sequence":n,"timestamp":null,
            "timePrecision":"unknown","status":"running","outcomeConflict":false,
            "evidence":[{"file":"/synthetic/source","line":n+1}]
        }))
        .unwrap();
        let state = Component {
            identity: facts::Prepared {
                preferred: "duplicate".into(),
                fallback: format!("fallback-{n}"),
                anchor: (
                    "synthetic".into(),
                    "/synthetic/source".into(),
                    "g".into(),
                    n as u64,
                    0,
                ),
                scope: ("synthetic".into(), Some(thread.into()), None),
                operation: None,
                aliases: Arc::from([]),
                issues: vec![],
            },
            operation: Arc::new(operation),
        };
        stage_component(&db, &format!("raw-{n}"), thread, &state).unwrap();
    }
    let mut report = report();
    resolve_operations(&db, &mut report, &cancelled).unwrap();
    let mut facts = Facts::default();
    let parents = facts.parents.clone();
    let forest =
        session_relations::SessionRelations::new_cancellable(&parents, &cancelled).unwrap();
    operations::project(&db, &mut facts, &forest, &mut report, &cancelled).unwrap();
    let mut actual = BTreeMap::new();
    crate::live_index::each_located(
        &db,
        "projection:synthetic",
        Some("operations"),
        |_, id, payload, _| {
            let value: Operation = serde_json::from_str(payload)?;
            assert_eq!(id, value.id);
            actual.insert(id.to_owned(), value.thread_id);
            Ok(())
        },
    )
    .unwrap();
    assert_eq!(
        actual,
        BTreeMap::from([
            ("duplicate".into(), Arc::from("a")),
            ("fallback-1".into(), Arc::from("b"))
        ])
    );
    db.execute("DELETE FROM codex_components WHERE id='raw-1'", [])
        .unwrap();
    assert!(operations::project(&db, &mut facts, &forest, &mut report, &cancelled).is_err());
}

#[test]
fn complete_scan_preserves_issue_budget_across_parse_and_canonical_publication() {
    let root = tempfile::tempdir().unwrap();
    let index = tempfile::tempdir().unwrap();
    fs::create_dir(root.path().join("sessions")).unwrap();
    let mut text = String::from("{\"type\":\"session_meta\",\"payload\":{\"id\":\"thread\"}}\n");
    text.push_str(
        "{\"type\":\"turn_context\",\"payload\":{\"turn_id\":\"turn\",\"model\":\"gpt-5.4\"}}\n",
    );
    for _ in 0..1002 {
        text.push_str("invalid-json\n");
    }
    text.push_str(&json!({"type":"event_msg","payload":{"type":"token_usage_record","response_id":"direct","usage":{"input_tokens":100,"output_tokens":10,"total_tokens":110}}}).to_string());
    text.push('\n');
    text.push_str(&json!({"type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":100,"output_tokens":10,"total_tokens":110},"last_token_usage":{"input_tokens":100,"output_tokens":10,"total_tokens":110}}}}).to_string());
    text.push('\n');
    fs::write(root.path().join("sessions/test.jsonl"), text).unwrap();
    let source = CodexAdapter
        .discover(&DiscoveryRequest {
            roots: vec![root.path().into()],
        })
        .sources
        .remove(0);
    let reference = crate::adapters::collect(
        &DiscoveryRequest {
            roots: vec![root.path().into()],
        },
        &RunContext::default(),
    );
    let db = crate::live_index::open(&index.path().join("index.sqlite")).unwrap();
    let actual = sync_cached(&db, &source, false, &mut Cache::default())
        .unwrap()
        .unwrap()
        .collected;
    let mut expected = reference.sources[0].issues.clone();
    expected.sort_by_key(|i| i.evidence.as_ref().map_or(0, |e| e.line));
    assert!(
        serde_json::to_value(&actual.sources[0].issues).unwrap()
            == serde_json::to_value(&expected).unwrap(),
        "full source diagnostics retain the same admitted issues and checkpoint order"
    );
    assert!(
        actual.sources[0]
            .issues
            .iter()
            .any(|i| i.code == "issueLimit")
    );
}
