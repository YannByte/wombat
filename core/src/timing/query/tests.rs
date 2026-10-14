use super::*;
use crate::adapters::contract::{
    Collected, SourceInstance, SourceReport, SourceWatermark, Thread, Turn, WatermarkState,
};
use crate::session_events::{
    ContentPhase, ContentPresence, LifecycleKind, MessageOrigin, MessageRecordKind, Phase,
    Position, Time as EventTime,
};
fn event(offset: u64, time: Option<i64>, payload: Payload) -> Arc<Event> {
    let time = time.map(|ms| {
        chrono::DateTime::from_timestamp_millis(ms)
            .unwrap()
            .to_rfc3339()
    });
    Arc::new(
        Event::new(
            Position {
                source_instance_id: "source-private".into(),
                file_id: "file-private".into(),
                generation: "generation-private".into(),
                byte_offset: offset,
                ordinal: 0,
            },
            Some("thread-private".into()),
            Some("turn-private".into()),
            EventTime::from_source(time.as_deref()).0,
            vec![],
            payload,
        )
        .unwrap(),
    )
}
fn boundary(
    offset: u64,
    time: Option<i64>,
    phase: Phase,
    duration: Option<u64>,
    ttft: Option<u64>,
) -> Arc<Event> {
    event(
        offset,
        time,
        Payload::Lifecycle {
            lifecycle: LifecycleKind::Turn,
            phase,
            native_id: Some("native-private".into()),
            duration_ms: duration,
            first_token_ms: ttft,
        },
    )
}
fn mcp_operation_event(
    offset: u64,
    millis: i64,
    call_id: &str,
    server: &str,
    phase: Phase,
    status: &str,
) -> Arc<Event> {
    let value = serde_json::from_value(serde_json::json!({
        "id": format!("operation-{offset}"),
        "threadId": "thread-private",
        "turnId": "turn-private",
        "callId": call_id,
        "kind": "mcpTool",
        "name": "lookup",
        "server": server,
        "tool": "lookup",
        "sequence": offset,
        "timePrecision": "millisecond",
        "status": status,
        "outcomeConflict": false,
        "evidence": []
    }))
    .unwrap();
    event(
        offset,
        Some(millis),
        Payload::Operation {
            value: Arc::new(value),
            phase,
        },
    )
}
fn data(events: Vec<Arc<Event>>) -> Collected {
    Collected {
        threads: vec![Thread {
            id: "thread-private".into(),
            agent_kind: "codex".into(),
            source_instance_id: "source-private".into(),
            upstream_id: "upstream-private".into(),
            title: Some("title-private".into()),
            project: Some("/project-private".into()),
            started_at: None,
            last_activity_at: None,
        }],
        turns: vec![Turn {
            id: "turn-private".into(),
            thread_id: "thread-private".into(),
            upstream_id: "native-private".into(),
            ordinal: 1,
            started_at: None,
            ended_at: None,
            last_activity_at: None,
            status: "completed".into(),
        }],
        sources: vec![SourceReport {
            source: SourceInstance {
                id: "source-private".into(),
                agent_kind: "codex".into(),
                root: "/root-private".into(),
            },
            adapter_version: "codex-rollout-6".into(),
            source_versions: vec![],
            capabilities: Default::default(),
            status: "complete".into(),
            files_read: 1,
            bytes_read: 100,
            issues: vec![],
        }],
        watermarks: vec![SourceWatermark {
            original_source: None,
            format_version: crate::adapters::contract::WATERMARK_FORMAT_VERSION,
            source_instance_id: "source-private".into(),
            file_id: "file-private".into(),
            generation: Some("generation-private".into()),
            committed_offset: 200_000,
            observed_bytes: Some(200_000),
            observed_at: "2026-10-01T00:00:00Z".into(),
            state: WatermarkState::Complete,
            issue_codes: vec![],
        }],
        events,
        ..Default::default()
    }
}
fn make_snapshot(events: Vec<Arc<Event>>) -> Snapshot {
    let root = tempfile::tempdir().unwrap();
    usage_store::memory(
        data(events),
        "live:private".into(),
        crate::pricing_sync::current_at(root.path()).unwrap(),
        None,
    )
    .unwrap()
}
fn request(profile: PrivacyProfile) -> Request {
    Request::Summary {
        thread_id: "thread-private".into(),
        turn_id: "turn-private".into(),
        snapshot_id: Some("live:private".into()),
        roots: vec![],
        scope: None,
        mode: Mode::Cached,
        privacy_profile: profile,
    }
}
fn query(snapshot: &Snapshot, request: &Request) -> Response {
    query_on_snapshot(
        snapshot,
        request,
        QueryFreshness {
            status: "cached".into(),
            ..Default::default()
        },
        &AtomicBool::new(false),
    )
    .unwrap()
}
fn local(response: Response) -> LocalResponse {
    match response {
        Response::Local(response) => *response,
        _ => panic!(),
    }
}
fn error_code(error: anyhow::Error) -> String {
    error
        .downcast_ref::<crate::dto::OperationError>()
        .unwrap()
        .code
        .into()
}
fn complete_events() -> Vec<Arc<Event>> {
    vec![
        boundary(0, Some(1_000), Phase::Started, None, None),
        event(
            1,
            Some(1_050),
            Payload::Message {
                origin: MessageOrigin::AssistantVisible,
                presence: ContentPresence::NonEmpty,
                native_id: Some("message-private".into()),
                record_kind: MessageRecordKind::LegacySnapshot,
                record_phase: Phase::Completed,
                content_phase: ContentPhase::FinalAnswer,
            },
        ),
        boundary(2, Some(1_100), Phase::Completed, Some(120), Some(0)),
    ]
}
#[test]
fn full_local_metrics_and_share_aliases_are_independent_whitelists() {
    let snapshot = make_snapshot(complete_events());
    let l = local(query(&snapshot, &request(PrivacyProfile::Local)));
    assert_eq!(l.time.native_wall_clock_ms.value, Some(120));
    assert_eq!(l.time.derived_wall_clock_ms.value, Some(100));
    assert_eq!(l.time.native_ttft_ms.value, Some(0));
    assert_eq!(l.time.first_content_record_delay_ms.value, Some(50));
    assert_eq!(
        l.time.first_content_record_delay_ms.status,
        MetricStatus::Derived
    );
    assert_eq!(
        l.time.strict_response_gap_ms.basis,
        Basis::MissingBatchCycle
    );
    assert_eq!(
        l.work.operation_candidates.basis,
        Basis::CanonicalOperationIdentity
    );
    assert_eq!(l.work.operation_candidates.value, Some(0));
    assert_eq!(l.context.active_context_occupancy.value, None);
    assert_eq!(l.evidence.collections.len(), 5);
    let operations = l
        .evidence
        .collections
        .iter()
        .find(|entry| matches!(entry.kind, CollectionKind::CanonicalOperations))
        .unwrap();
    assert_eq!(operations.count.value, Some(0));
    assert!(
        l.time
            .native_wall_clock_ms
            .evidence_refs
            .iter()
            .all(|id| id.starts_with("collection:"))
    );
    let a = query(&snapshot, &request(PrivacyProfile::ShareV1));
    let b = query(&snapshot, &request(PrivacyProfile::ShareV1));
    let a_json = serde_json::to_value(&a).unwrap();
    let b_json = serde_json::to_value(&b).unwrap();
    assert_ne!(a_json["scope"], b_json["scope"]);
    let bytes = serde_json::to_string(&a).unwrap();
    for secret in [
        "source-private",
        "thread-private",
        "turn-private",
        "file-private",
        "generation-private",
        "upstream-private",
        "title-private",
        "/root-private",
        "/project-private",
        "live:private",
        "native-private",
        "message-private",
        "2026-10-01",
        "1970-01-01",
    ] {
        assert!(!bytes.contains(secret), "{secret}");
    }
    assert!(a_json.get("readView").is_none());
    assert!(a_json.get("evidence").is_none());
    assert_eq!(a_json["relativeAnchors"]["startMs"]["value"], 0);
    assert_eq!(a_json["relativeAnchors"]["endMs"]["value"], 100);
    assert_eq!(a_json["time"]["nativeWallClockMs"]["value"], 120);
    assert!(fits(&a).unwrap());
}
#[test]
fn request_union_is_narrow_and_capabilities_need_no_snapshot() {
    for value in [
        serde_json::json!({"action":"capabilities","roots":["/private"]}),
        serde_json::json!({"action":"summary","threadId":"t","turnId":"r","verify":true}),
        serde_json::json!({"action":"evidence","threadId":"t","turnId":"r","snapshotId":"s","mode":"fresh"}),
        serde_json::json!({"action":"summary","threadId":"t","turnId":"r","scope":{"since":"2026-01-01"}}),
    ] {
        assert!(serde_json::from_value::<Request>(value).is_err());
    }
    let capabilities: Request =
        serde_json::from_value(serde_json::json!({"action":"capabilities"})).unwrap();
    assert!(matches!(
        dispatch(capabilities).unwrap(),
        Response::Capabilities(_)
    ));
    let mut invalid_request = request(PrivacyProfile::Local);
    if let Request::Summary { mode, .. } = &mut invalid_request {
        *mode = Mode::Fresh;
    }
    assert_eq!(
        error_code(validate(&invalid_request).unwrap_err()),
        "INVALID_ARGUMENT"
    );
    let schema = crate::dispatch("schema_timing_request", &serde_json::json!({})).unwrap();
    assert!(schema.to_string().contains("additionalProperties"));
}
#[test]
fn evidence_pages_keep_whole_denominator_and_bind_target_and_snapshot() {
    let snapshot = make_snapshot(complete_events());
    let r = Request::Evidence {
        collection: EvidenceSet::TurnEvents,
        object_ref: None,
        thread_id: "thread-private".into(),
        turn_id: "turn-private".into(),
        snapshot_id: "live:private".into(),
        roots: vec![],
        scope: None,
        cursor: None,
        limit: 1,
        privacy_profile: PrivacyProfile::Local,
    };
    let Response::Evidence(first) = query(&snapshot, &r) else {
        panic!()
    };
    assert_eq!(first.total.value, Some(3));
    assert_eq!(first.rows.len(), 1);
    assert!(first.rows[0].reference.starts_with("event:"));
    let mut next = r.clone();
    if let Request::Evidence { cursor, .. } = &mut next {
        *cursor = first.next_cursor.clone();
    }
    let Response::Evidence(second) = query(&snapshot, &next) else {
        panic!()
    };
    assert_eq!(second.total.value, Some(3));
    assert_ne!(first.rows[0].reference, second.rows[0].reference);
    let mut stale = next.clone();
    if let Request::Evidence { snapshot_id, .. } = &mut stale {
        *snapshot_id = "other-view".into();
    }
    assert_eq!(
        error_code(
            query_on_snapshot(
                &snapshot,
                &stale,
                Default::default(),
                &AtomicBool::new(false)
            )
            .unwrap_err()
        ),
        "VIEW_EXPIRED"
    );
    if let Request::Evidence { turn_id, .. } = &mut next {
        *turn_id = "other-turn".into();
    }
    assert!(
        query_on_snapshot(
            &snapshot,
            &next,
            Default::default(),
            &AtomicBool::new(false)
        )
        .is_err()
    );
    for token in ["00".repeat(4097), "z".into(), "000".into()] {
        assert!(decode_cursor(&Cursor { token }).is_err());
    }
}
#[test]
fn evidence_numeric_range_is_distinct_from_missing_and_zero() {
    let huge = boundary(0, Some(0), Phase::Completed, Some(u64::MAX), Some(0));
    let row = evidence_row(&huge).unwrap();
    assert_eq!(row.duration_ms, None);
    assert_eq!(row.first_token_ms, Some(0));
    assert!(row.gap_codes.iter().any(|code| code == "numeric_range"));
    let snapshot = make_snapshot(vec![huge]);
    let l = local(query(&snapshot, &request(PrivacyProfile::Local)));
    assert_eq!(l.time.native_wall_clock_ms.basis, Basis::NumericRange);
    assert!(l.quality.reason_codes.contains(&Basis::NumericRange));
}
#[test]
fn source_membership_snapshot_identity_and_cancellation_reject_safely() {
    let snapshot = make_snapshot(complete_events());
    let mut r = request(PrivacyProfile::Local);
    if let Request::Summary { scope, .. } = &mut r {
        *scope = Some(Scope {
            source_instance_id: Some("elsewhere".into()),
            agent_kind: None,
        });
    }
    assert_eq!(
        error_code(
            query_on_snapshot(&snapshot, &r, Default::default(), &AtomicBool::new(false))
                .unwrap_err()
        ),
        "INVALID_ARGUMENT"
    );
    assert_eq!(
        error_code(
            query_on_snapshot(
                &snapshot,
                &request(PrivacyProfile::Local),
                Default::default(),
                &AtomicBool::new(true)
            )
            .unwrap_err()
        ),
        "CANCELLED"
    );
    let safe = error_output(&operation_error(
        "SOURCE_UNREADABLE",
        "secret /private/raw-json",
    ));
    assert!(!serde_json::to_string(&safe).unwrap().contains("private"));
}
#[test]
fn native_fallback_reduces_complete_partition_including_final_conflict() {
    let mut events = Vec::with_capacity(100_001);
    events.push(boundary(0, None, Phase::Started, None, None));
    for offset in 1..100_000 {
        events.push(boundary(offset, None, Phase::Completed, Some(42), Some(5)));
    }
    events.push(boundary(100_000, None, Phase::Completed, Some(43), Some(5)));
    let snapshot = make_snapshot(events);
    let l = local(query(&snapshot, &request(PrivacyProfile::Local)));
    assert_eq!(l.time.native_wall_clock_ms.value, None);
    assert_eq!(l.time.native_wall_clock_ms.basis, Basis::BoundaryConflict);
    assert_eq!(l.time.native_ttft_ms.value, Some(5));
    assert_eq!(l.time.derived_wall_clock_ms.basis, Basis::ResourceLimit);
    assert_eq!(l.time.repeated_behavior.after_failure.count.value, None);
    assert_eq!(
        l.time
            .repeated_behavior
            .same_request_observation_count
            .value,
        None
    );
    assert!(
        l.time
            .repeated_behavior
            .coverage
            .reason_codes
            .contains(&RepeatCoverageReason::ResourceLimit)
    );
    assert_eq!(l.time.operation_coverage.candidate_operations.value, None);
    assert_eq!(
        l.time.operation_coverage.residual_ms.basis,
        Basis::ResourceLimit
    );
    assert!(
        l.time
            .operation_coverage
            .reason_codes
            .contains(&OperationCoverageReason::ResourceLimit)
    );
    assert_eq!(l.time.timeline.presentation, TimelinePresentation::List);
    assert_eq!(l.time.timeline.entry_count.value, None);
    assert_eq!(l.time.timeline.entry_count.basis, Basis::ResourceLimit);
    assert_eq!(l.time.timeline.identified_interval_count.value, None);
    assert_eq!(
        l.evidence.interval_pages.detail.reason,
        Basis::ResourceLimit
    );
    assert_eq!(l.evidence.interval_pages.located_interval_count.value, None);
    assert_eq!(
        l.evidence.interval_pages.missing_event_ref_count.value,
        None
    );
    assert!(l.evidence.interval_pages.entries.is_empty());
    assert_eq!(
        l.time.timeline.unlocated_interval_count.basis,
        Basis::ResourceLimit
    );
    assert!(l.time.timeline.tracks.is_empty());
    assert_eq!(l.context.input.samples.value, None);
    assert!(l.quality.partial);
    assert!(l.quality.reason_codes.contains(&Basis::BoundaryConflict));
    assert!(l.quality.reason_codes.contains(&Basis::ResourceLimit));
    assert!(matches!(
        l.evidence.collections[0].kind,
        CollectionKind::NativeBoundaryIndex
    ));
}
#[test]
fn cache_hits_refresh_freshness_and_remain_snapshot_owned() {
    let snapshot = make_snapshot(complete_events());
    let r = request(PrivacyProfile::Local);
    let first = local(
        query_on_snapshot(
            &snapshot,
            &r,
            QueryFreshness {
                status: "cached".into(),
                checked_at: Some("old".into()),
                ..Default::default()
            },
            &AtomicBool::new(false),
        )
        .unwrap(),
    );
    let second = local(
        query_on_snapshot(
            &snapshot,
            &r,
            QueryFreshness {
                status: "fresh".into(),
                checked_at: Some("new".into()),
                ..Default::default()
            },
            &AtomicBool::new(false),
        )
        .unwrap(),
    );
    assert!(!first.quality.partial);
    assert_eq!(second.freshness.checked_at.as_deref(), Some("new"));
    let key = serde_json::to_string(&(METHOD_VERSION, &r)).unwrap();
    assert!(snapshot.timing_cache.lock().unwrap().get(&key).is_some());
    assert!(super::super::cache::Cache::default().get(&key).is_none());
}
#[test]
fn canonical_input_type7_and_masks_reach_production_query() {
    let mut collected = data(complete_events());
    collected.measurements = [100u64, 500u64].into_iter().enumerate().map(|(i, input)| Arc::new(serde_json::from_value(serde_json::json!({"id":format!("m-{i}"),"agentKind":"codex","sourceInstanceId":"source-private","threadId":"thread-private","turnId":"turn-private","grain":"response","timePrecision":"unknown","model":{},"tokens":{"rawInput":input},"tokenUnavailableReasons":{"input":"missing","cacheRead":"missing","cacheCreate":"missing","output":"missing","reasoning":"missing","total":"missing","rawInput":null},"pricingContextConflict":false,"requestScoped":true,"sequence":i,"evidence":[]})).unwrap())).collect();
    let root = tempfile::tempdir().unwrap();
    let snapshot = usage_store::memory(
        collected,
        "live:private".into(),
        crate::pricing_sync::current_at(root.path()).unwrap(),
        None,
    )
    .unwrap();
    let l = local(query(&snapshot, &request(PrivacyProfile::Local)));
    assert_eq!(l.context.input.samples.value, Some(2));
    assert_eq!(l.context.input.median.value, Some(300.0));
    assert_eq!(l.context.input.p90.value, Some(460.0));
    assert_eq!(l.context.ratio.samples.value, Some(0));
    assert_eq!(l.context.ratio.p90.value, None);
    assert_eq!(l.time.intersection_masks_ms[0].value, Some(100));
    assert_eq!(l.time.unclassified_ms.value, Some(100));
    assert_eq!(l.context.quantile_method, "type_7");
}

#[test]
fn numeric_schema_bounds_integer_values_and_preserves_finite_large_ratios() {
    let count_schema = serde_json::to_value(
        schemars::generate::SchemaSettings::draft07()
            .into_generator()
            .into_root_schema_for::<Count>(),
    )
    .unwrap();
    assert_eq!(
        count_schema["properties"]["value"]["maximum"],
        MAX_SAFE_INTEGER
    );
    assert_eq!(count_schema["properties"]["value"]["minimum"], 0);
    let signed_schema = serde_json::to_value(
        schemars::generate::SchemaSettings::draft07()
            .into_generator()
            .into_root_schema_for::<Signed>(),
    )
    .unwrap();
    assert_eq!(
        signed_schema["properties"]["value"]["minimum"],
        -(MAX_SAFE_INTEGER as i64)
    );
    assert_eq!(
        m::count(Some(u128::MAX), Basis::NativeRecord, &[]).basis,
        Basis::NumericRange
    );
    assert_eq!(
        m::signed(Some(i128::MIN), Basis::ExplicitBoundary, &[]).basis,
        Basis::NumericRange
    );
    assert_eq!(
        m::number(Some(f64::INFINITY), Basis::Type7, &[]).value,
        None
    );
    assert_eq!(
        m::ratio(Some(u64::MAX as f64), Basis::HistoricalWindow, &[]).value,
        Some(u64::MAX as f64)
    );
    assert_eq!(safe_timestamp(Some(i64::MAX)), (None, true));
    assert_eq!(safe_timestamp(Some(0)), (Some(0), false));
    // The currently accepted chrono/RFC3339 source domain is itself below the
    // safe range; the projection guard remains explicit for future widening.
    assert!(
        chrono::DateTime::<chrono::Utc>::MAX_UTC
            .timestamp_millis()
            .unsigned_abs()
            < MAX_SAFE_INTEGER
    );
    assert!(
        chrono::DateTime::<chrono::Utc>::MIN_UTC
            .timestamp_millis()
            .unsigned_abs()
            < MAX_SAFE_INTEGER
    );
}
#[test]
fn fixed_share_freshness_preserves_fixed_and_syncing_enum_states() {
    let snapshot = make_snapshot(complete_events());
    for state in ["fixed", "syncing"] {
        let Response::Share(response) = query_on_snapshot(
            &snapshot,
            &request(PrivacyProfile::ShareV1),
            QueryFreshness {
                status: state.into(),
                ..Default::default()
            },
            &AtomicBool::new(false),
        )
        .unwrap() else {
            panic!()
        };
        assert_eq!(response.freshness.status, state);
    }
}
#[test]
fn cache_enforces_entry_count_total_bytes_and_oversized_results() {
    let snapshot = make_snapshot(complete_events());
    let result = local(query(&snapshot, &request(PrivacyProfile::Local)));
    let mut cache = super::super::cache::Cache::default();
    for i in 0..17 {
        assert!(cache.insert(format!("key-{i}"), result.clone()));
    }
    assert!(cache.get("key-0").is_none());
    assert!(cache.get("key-16").is_some());
    let mut large = result.clone();
    large.privacy.omitted_fields = vec!["x".repeat(170_000)];
    let mut bytes = super::super::cache::Cache::default();
    for i in 0..16 {
        assert!(bytes.insert(format!("key-{i}"), large.clone()));
    }
    assert!(bytes.get("key-0").is_none());
    assert!(bytes.get("key-15").is_some());
    assert!(!bytes.insert("oversized".into(), {
        let mut value = result.clone();
        value.privacy.omitted_fields = vec!["x".repeat(MAX_SUMMARY_BYTES)];
        value
    }));
    let mut partial = result;
    partial.quality.partial = true;
    assert!(!bytes.insert("partial".into(), partial));
}
#[test]
fn identity_time_and_content_conflicts_keep_specific_reasons() {
    let mut events = complete_events();
    events.push(boundary(
        3,
        Some(1_200),
        Phase::Completed,
        Some(121),
        Some(0),
    ));
    let snapshot = make_snapshot(events);
    let l = local(query(&snapshot, &request(PrivacyProfile::Local)));
    assert_eq!(l.time.native_wall_clock_ms.basis, Basis::BoundaryConflict);
    assert_eq!(l.time.derived_wall_clock_ms.basis, Basis::BoundaryConflict);
    assert_eq!(l.time.native_ttft_ms.value, Some(0));
    let mut events = complete_events();
    events[1] = event(
        2,
        Some(1_050),
        Payload::Message {
            origin: MessageOrigin::AssistantVisible,
            presence: ContentPresence::NonEmpty,
            native_id: Some("known".into()),
            record_kind: MessageRecordKind::ResponseSnapshot,
            record_phase: Phase::Completed,
            content_phase: ContentPhase::Unknown,
        },
    );
    events[2] = boundary(3, Some(1_100), Phase::Completed, Some(120), Some(0));
    events.insert(
        1,
        event(
            1,
            None,
            Payload::Message {
                origin: MessageOrigin::AssistantVisible,
                presence: ContentPresence::NonEmpty,
                native_id: None,
                record_kind: MessageRecordKind::ResponseSnapshot,
                record_phase: Phase::Completed,
                content_phase: ContentPhase::Unknown,
            },
        ),
    );
    let snapshot = make_snapshot(events);
    let l = local(query(&snapshot, &request(PrivacyProfile::Local)));
    assert_eq!(
        l.time.first_content_record_delay_ms.basis,
        Basis::MissingTime
    );
}
#[test]
fn excessive_context_detail_preserves_full_distribution_and_explains_omission() {
    let mut collected = data(complete_events());
    collected.measurements = (0..33).map(|i| Arc::new(serde_json::from_value(serde_json::json!({"id":format!("m-{i}"),"agentKind":"codex","sourceInstanceId":"source-private","threadId":"thread-private","turnId":"turn-private","grain":"response","timePrecision":"unknown","model":{},"tokens":{"rawInput":i},"tokenUnavailableReasons":{"input":"missing","cacheRead":"missing","cacheCreate":"missing","output":"missing","reasoning":"missing","total":"missing","rawInput":null},"pricingContextConflict":false,"requestScoped":true,"sequence":i,"evidence":[]})).unwrap())).collect();
    let root = tempfile::tempdir().unwrap();
    let snapshot = usage_store::memory(
        collected,
        "live:private".into(),
        crate::pricing_sync::current_at(root.path()).unwrap(),
        None,
    )
    .unwrap();
    let l = local(query(&snapshot, &request(PrivacyProfile::Local)));
    assert_eq!(l.context.input.samples.value, Some(33));
    assert_eq!(l.context.input.p90.value, Some(28.8));
    assert_eq!(l.context.segment_count.value, Some(33));
    assert!(l.context.segments.is_empty());
    assert_eq!(l.context.detail.reason, Basis::ResourceLimit);
    assert!(l.quality.partial);
    assert!(fits(&l).unwrap());
}

#[test]
fn combined_response_schema_keeps_unsigned_signed_and_fractional_metrics_distinct() {
    let schema = crate::dispatch("schema_timing_local_response", &serde_json::json!({})).unwrap();
    let definitions = schema["definitions"].as_object().unwrap();
    let count = definitions
        .iter()
        .find(|(name, _)| name.starts_with("TimingMetric_") && name.contains("uint64"))
        .unwrap()
        .1;
    let signed = definitions
        .iter()
        .find(|(name, _)| {
            name.starts_with("TimingMetric_") && name.contains("int64") && !name.contains("uint64")
        })
        .unwrap()
        .1;
    let number = definitions
        .iter()
        .find(|(name, _)| name.starts_with("TimingMetric_") && name.contains("double"))
        .unwrap()
        .1;
    assert_eq!(
        count["properties"]["value"]["type"],
        serde_json::json!(["integer", "null"])
    );
    assert_eq!(count["properties"]["value"]["minimum"], 0);
    assert_eq!(
        signed["properties"]["value"]["minimum"],
        -(MAX_SAFE_INTEGER as i64)
    );
    assert_eq!(
        number["properties"]["value"]["type"],
        serde_json::json!(["number", "null"])
    );
    assert!(number["properties"]["value"].get("maximum").is_none());
    let snapshot = make_snapshot(complete_events());
    let mut response = local(query(&snapshot, &request(PrivacyProfile::Local)));
    response.time.coverage_ratio = m::ratio(Some(0.5), Basis::IntervalMask, &[]);
    response.time.boundary_discrepancy_ms = m::signed(Some(-5), Basis::ExplicitBoundary, &[]);
    response.work.message_record_candidates = m::observed(3, &[]);
    let json = serde_json::to_value(response).unwrap();
    assert_eq!(json["time"]["coverageRatio"]["value"], 0.5);
    assert_eq!(json["time"]["boundaryDiscrepancyMs"]["value"], -5);
    assert_eq!(json["work"]["messageRecordCandidates"]["value"], 3);
}

#[test]
fn cancellation_cannot_publish_a_cache_entry() {
    let snapshot = make_snapshot(complete_events());
    let value = local(query(&snapshot, &request(PrivacyProfile::Local)));
    let mut cache = super::super::cache::Cache::default();
    let error = cache
        .insert_cancellable("cancelled".into(), value, &AtomicBool::new(true))
        .unwrap_err();
    assert_eq!(error_code(error), "CANCELLED");
    assert!(cache.get("cancelled").is_none());
}

fn timed_item(offset: u64, id: &str, start: Option<i64>, end: Option<i64>) -> Arc<Event> {
    event(
        offset,
        None,
        Payload::Item {
            item_kind: crate::session_events::ItemKind::Command,
            native_id: Some(id.into()),
            phase: Phase::Completed,
            started_at_ms: start,
            completed_at_ms: end,
            duration: None,
        },
    )
}
#[test]
fn timeline_is_rust_relative_projection_with_fragment_proof_and_private_share_aliases() {
    let operation = timed_item(2, "item-private", Some(1010), Some(1060));
    let snapshot = make_snapshot(vec![
        boundary(0, Some(1000), Phase::Started, None, None),
        operation.clone(),
        boundary(3, Some(1100), Phase::Completed, Some(200), None),
    ]);
    let result = local(query(&snapshot, &request(PrivacyProfile::Local)));
    let t = &result.time.timeline;
    assert_eq!(t.presentation, TimelinePresentation::Timeline);
    assert_eq!(t.detail.support, Support::Supported);
    assert_eq!(t.entry_count.value, Some(3));
    assert_eq!(t.identified_interval_count.value, Some(1));
    assert_eq!((t.tracks[0].start_ms, t.tracks[0].end_ms), (10, 60));
    assert_eq!(
        t.tracks[0].evidence_refs,
        [format!("event:{}", operation.id())]
    );
    assert_eq!(t.tracks[0].evidence_scope, FragmentEvidence::EventRecords);
    assert_eq!(
        t.unclassified_gaps
            .iter()
            .map(|g| (g.start_ms, g.end_ms))
            .collect::<Vec<_>>(),
        [(0, 10), (60, 100)]
    );
    assert_eq!(
        t.unclassified_gaps[0].evidence_scope,
        FragmentEvidence::TurnCollection
    );
    assert_eq!(result.time.native_wall_clock_ms.value, Some(200));
    assert_eq!(result.time.derived_wall_clock_ms.value, Some(100));
    let first = super::super::share::project(&result);
    let second = super::super::share::project(&result);
    assert_ne!(
        first.time.timeline.tracks[0].interval_alias,
        second.time.timeline.tracks[0].interval_alias
    );
    assert_ne!(
        first.time.timeline.tracks[0].evidence_refs,
        second.time.timeline.tracks[0].evidence_refs
    );
    assert_eq!(
        (
            first.time.timeline.tracks[0].start_ms,
            first.time.timeline.tracks[0].end_ms
        ),
        (10, 60)
    );
    let encoded = serde_json::to_string(&first).unwrap();
    for private in [
        operation.id(),
        "item-private",
        "file-private",
        "generation-private",
        "thread-private",
        "source-private",
    ] {
        assert!(!encoded.contains(private));
    }
}
#[test]
fn timeline_list_for_running_missing_anchors_and_majority_unlocated() {
    let running = make_snapshot(vec![
        boundary(0, Some(1000), Phase::Started, None, None),
        timed_item(1, "located", Some(1010), Some(1020)),
    ]);
    let t = local(query(&running, &request(PrivacyProfile::Local)))
        .time
        .timeline;
    assert_eq!(t.presentation, TimelinePresentation::List);
    assert_eq!(t.detail.reason, Basis::RunningTurn);
    assert_eq!(t.entry_count.value, None);
    assert_eq!(t.entry_count.basis, Basis::RunningTurn);
    assert!(t.tracks.is_empty());
    let missing = make_snapshot(vec![timed_item(1, "located", Some(10), Some(20))]);
    let t = local(query(&missing, &request(PrivacyProfile::Local)))
        .time
        .timeline;
    assert_eq!(t.presentation, TimelinePresentation::List);
    assert_eq!(t.detail.reason, Basis::MissingTime);
    assert_eq!(t.entry_count.value, None);
    let partial = make_snapshot(vec![
        boundary(0, Some(1000), Phase::Started, None, None),
        timed_item(1, "located", Some(1010), Some(1020)),
        timed_item(2, "open-one", Some(1030), None),
        timed_item(3, "open-two", Some(1040), None),
        boundary(4, Some(1100), Phase::Completed, None, None),
    ]);
    let t = local(query(&partial, &request(PrivacyProfile::Local)))
        .time
        .timeline;
    assert_eq!(t.presentation, TimelinePresentation::List);
    assert_eq!(t.identified_interval_count.value, Some(3));
    assert_eq!(t.unlocated_interval_count.value, Some(2));
    assert_eq!(t.detail.support, Support::Partial);
    assert_eq!(t.tracks.len(), 1);
}
#[test]
fn timeline_detail_resource_limit_preserves_full_aggregates_and_exact_counts() {
    let mut events = vec![boundary(0, Some(1000), Phase::Started, None, None)];
    events.extend((0..200).map(|n| {
        timed_item(
            n + 1,
            &format!("i{n:03}"),
            Some(1000 + n as i64),
            Some(1001 + n as i64),
        )
    }));
    events.push(boundary(201, Some(1201), Phase::Completed, Some(250), None));
    let snapshot = make_snapshot(events);
    let result = local(query(&snapshot, &request(PrivacyProfile::Local)));
    let t = result.time.timeline;
    assert_eq!(t.presentation, TimelinePresentation::List);
    assert_eq!(t.detail.reason, Basis::ResourceLimit);
    assert_eq!(t.entry_count.value, Some(201));
    assert_eq!(t.track_count.value, Some(200));
    assert_eq!(t.unclassified_gap_count.value, Some(1));
    assert!(t.tracks.is_empty() && t.unclassified_gaps.is_empty());
    assert_eq!(result.time.command.union_ms.value, Some(200));
    assert_eq!(result.time.command.sum_ms.value, Some(200));
    assert_eq!(result.time.unclassified_ms.value, Some(1));
    assert_eq!(result.time.native_wall_clock_ms.value, Some(250));
    assert!(result.quality.reason_codes.contains(&Basis::ResourceLimit));
}

#[test]
fn timeline_numeric_range_drops_detail_without_unsafe_coordinates_or_false_zero() {
    let mut a = analysis::analyze(analysis::AnalyzeInput {
        source: "source",
        thread: "thread",
        turn: "turn",
        events: &[],
        measurements: &[],
        budget: analysis::Budget {
            events: 0,
            measurements: 0,
            lifecycle_records: 0,
        },
    });
    a.state = analysis::State::Completed;
    a.intervals = super::super::intervals::analyze(
        Some(super::super::intervals::Window {
            start_ms: i64::MIN,
            end_ms: i64::MAX,
        }),
        &[],
        &[],
        0,
    );
    let t = m::time(&a, &[], None, None).timeline;
    assert_eq!(t.presentation, TimelinePresentation::List);
    assert_eq!(t.detail.reason, Basis::NumericRange);
    assert_eq!(t.unclassified_gap_count.value, Some(1));
    assert_eq!(t.entry_count.value, Some(1));
    assert!(t.tracks.is_empty() && t.unclassified_gaps.is_empty());
    let schema = crate::dispatch("schema_timing_local_response", &serde_json::json!({})).unwrap();
    assert_eq!(
        schema["definitions"]["TimelineTrack"]["properties"]["startMs"]["maximum"],
        MAX_SAFE_INTEGER
    );
    assert_eq!(
        schema["definitions"]["Timeline"]["properties"]["tracks"]["maxItems"],
        200
    );
}

fn empty_message(offset: u64) -> Arc<Event> {
    event(
        offset,
        None,
        Payload::Message {
            origin: MessageOrigin::AssistantVisible,
            presence: ContentPresence::Empty,
            native_id: None,
            record_kind: MessageRecordKind::LegacySnapshot,
            record_phase: Phase::Completed,
            content_phase: ContentPhase::Unknown,
        },
    )
}
#[test]
fn fragment_navigation_groups_exact_event_pages_and_excludes_control_order() {
    let start = event(
        1,
        Some(1010),
        Payload::Item {
            item_kind: crate::session_events::ItemKind::Command,
            native_id: Some("operation".into()),
            phase: Phase::Started,
            started_at_ms: None,
            completed_at_ms: None,
            duration: None,
        },
    );
    let end = event(
        399,
        Some(1060),
        Payload::Item {
            item_kind: crate::session_events::ItemKind::Command,
            native_id: Some("operation".into()),
            phase: Phase::Completed,
            started_at_ms: None,
            completed_at_ms: None,
            duration: None,
        },
    );
    let together = event(
        400,
        Some(1020),
        Payload::Item {
            item_kind: crate::session_events::ItemKind::Command,
            native_id: Some("together".into()),
            phase: Phase::Started,
            started_at_ms: None,
            completed_at_ms: None,
            duration: None,
        },
    );
    let together_end = event(
        401,
        Some(1070),
        Payload::Item {
            item_kind: crate::session_events::ItemKind::Command,
            native_id: Some("together".into()),
            phase: Phase::Completed,
            started_at_ms: None,
            completed_at_ms: None,
            duration: None,
        },
    );
    let mut events = vec![
        boundary(0, Some(1000), Phase::Started, None, None),
        start.clone(),
    ];
    events.extend((2..399).map(empty_message));
    events.extend([
        end.clone(),
        together.clone(),
        together_end.clone(),
        boundary(402, Some(1100), Phase::Completed, None, None),
    ]);
    let mut position = empty_message(200).position().clone();
    position.ordinal = 1;
    events.push(Arc::new(
        Event::new(
            position,
            Some("thread-private".into()),
            None,
            EventTime::from_source(None).0,
            vec![crate::session_events::Gap::SourcePartial],
            Payload::Message {
                origin: MessageOrigin::Unknown,
                presence: ContentPresence::Unknown,
                native_id: None,
                record_kind: MessageRecordKind::Unknown,
                record_phase: Phase::Unknown,
                content_phase: ContentPhase::Unknown,
            },
        )
        .unwrap(),
    ));
    events.reverse();
    let snapshot = make_snapshot(events);
    let result = local(query(&snapshot, &request(PrivacyProfile::Local)));
    let nav = &result.evidence.interval_pages;
    assert_eq!(nav.detail.support, Support::Supported);
    assert_eq!(nav.candidate_interval_count.value, Some(2));
    assert_eq!(nav.located_interval_count.value, Some(2));
    assert_eq!(nav.page_count.value, Some(3));
    let operation = &nav.entries[0];
    assert_eq!(
        operation.interval_alias,
        result.time.timeline.tracks[0].interval_alias
    );
    assert_eq!(operation.pages.len(), 2);
    assert!(operation.pages[0].cursor.is_none());
    assert_eq!(
        operation.pages[0].evidence_refs,
        [format!("event:{}", start.id())]
    );
    assert_eq!(
        operation.pages[1].evidence_refs,
        [format!("event:{}", end.id())]
    );
    assert_eq!(nav.entries[1].pages.len(), 1);
    assert_eq!(
        nav.entries[1].pages[0].evidence_refs,
        [
            format!("event:{}", together.id()),
            format!("event:{}", together_end.id())
        ]
    );
    for entry in &nav.entries {
        for page in &entry.pages {
            let Response::Evidence(evidence) = query(
                &snapshot,
                &Request::Evidence {
                    collection: EvidenceSet::TurnEvents,
                    object_ref: None,
                    thread_id: "thread-private".into(),
                    turn_id: "turn-private".into(),
                    snapshot_id: "live:private".into(),
                    roots: vec![],
                    scope: None,
                    cursor: page.cursor.clone(),
                    limit: page.limit,
                    privacy_profile: PrivacyProfile::Local,
                },
            ) else {
                panic!()
            };
            for reference in &page.evidence_refs {
                assert!(evidence.rows.iter().any(|row| &row.reference == reference));
            }
        }
    }
    let shared = super::super::share::project(&result);
    let encoded = serde_json::to_string(&shared).unwrap();
    assert!(
        !encoded.contains("intervalPages")
            && !encoded.contains("cursor")
            && !encoded.contains("token")
    );
    let schema = crate::dispatch("schema_timing_share_response", &serde_json::json!({})).unwrap();
    assert!(schema["definitions"]["Cursor"].is_null());
    assert!(schema["definitions"]["IntervalPages"].is_null());
}
#[test]
fn fragment_navigation_duplicate_or_missing_event_ids_are_rejected_and_cancelled() {
    let support = timed_item(1, "support", Some(1010), Some(1020));
    let events = vec![
        boundary(0, Some(1000), Phase::Started, None, None),
        support.clone(),
        boundary(2, Some(1100), Phase::Completed, None, None),
    ];
    let snapshot = make_snapshot(events.clone());
    let result = local(query(&snapshot, &request(PrivacyProfile::Local)));
    let target = TurnTarget {
        source: "source-private",
        thread: "thread-private",
        turn: "turn-private",
    };
    let mut duplicate = events.clone();
    duplicate.push(support.clone());
    let error = navigation::build(
        &snapshot,
        target,
        &result.time.timeline,
        Some(&duplicate),
        None,
        &AtomicBool::new(false),
    )
    .unwrap_err();
    assert_eq!(error_code(error), "SNAPSHOT_CORRUPT");
    let missing: Vec<_> = events
        .into_iter()
        .filter(|event| event.id() != support.id())
        .collect();
    let error = navigation::build(
        &snapshot,
        target,
        &result.time.timeline,
        Some(&missing),
        None,
        &AtomicBool::new(false),
    )
    .unwrap_err();
    assert_eq!(error_code(error), "SNAPSHOT_CORRUPT");
    let error = navigation::build(
        &snapshot,
        target,
        &result.time.timeline,
        Some(&[]),
        None,
        &AtomicBool::new(true),
    )
    .unwrap_err();
    assert_eq!(error_code(error), "CANCELLED");
}
#[test]
fn fragment_navigation_byte_budget_discards_all_locators_and_retains_counts() {
    let mut events = vec![boundary(0, Some(1000), Phase::Started, None, None)];
    events.extend((1..6000).map(|n| {
        if n % 200 == 1 {
            timed_item(
                n,
                &format!("i{n}"),
                Some(1010 + n as i64 / 200),
                Some(1011 + n as i64 / 200),
            )
        } else {
            empty_message(n)
        }
    }));
    events.push(boundary(
        6000,
        Some(1100),
        Phase::Completed,
        Some(150),
        None,
    ));
    let mut snapshot = make_snapshot(events);
    snapshot.manifest.snapshot_ref.snapshot_id = format!("live:{}", "x".repeat(1000));
    let mut request = request(PrivacyProfile::Local);
    if let Request::Summary { snapshot_id, .. } = &mut request {
        *snapshot_id = Some(snapshot.manifest.snapshot_ref.snapshot_id.clone());
    }
    let result = local(query(&snapshot, &request));
    let nav = result.evidence.interval_pages;
    assert_eq!(nav.detail.support, Support::Unavailable);
    assert_eq!(nav.detail.reason, Basis::ResourceLimit);
    assert!(nav.entries.is_empty());
    assert_eq!(nav.candidate_interval_count.value, Some(30));
    assert_eq!(nav.located_interval_count.value, Some(30));
    assert_eq!(nav.page_count.value, Some(30));
    assert_eq!(nav.missing_event_ref_count.value, Some(0));
    assert!(serde_json::to_vec(&nav).unwrap().len() <= navigation::LIMIT_BYTES);
    assert_eq!(result.time.timeline.tracks.len(), 30);
    assert_eq!(result.time.command.union_ms.value, Some(30));
    assert_eq!(result.time.native_wall_clock_ms.value, Some(150));
    assert!(result.quality.reason_codes.contains(&Basis::ResourceLimit));
}

#[test]
fn fragment_navigation_known_zero_and_unknown_proof_remain_distinct() {
    let snapshot = make_snapshot(complete_events());
    let result = local(query(&snapshot, &request(PrivacyProfile::Local)));
    assert_eq!(
        result.evidence.interval_pages.detail.reason,
        Basis::NoCandidates
    );
    assert_eq!(
        result
            .evidence
            .interval_pages
            .candidate_interval_count
            .value,
        Some(0)
    );
    assert_eq!(
        result.evidence.interval_pages.located_interval_count.value,
        Some(0)
    );
    assert_eq!(
        result.evidence.interval_pages.missing_event_ref_count.value,
        Some(0)
    );
    let support = timed_item(1, "support", Some(1010), Some(1020));
    let events = vec![
        boundary(0, Some(1000), Phase::Started, None, None),
        support,
        boundary(2, Some(1100), Phase::Completed, None, None),
    ];
    let snapshot = make_snapshot(events.clone());
    let mut timeline = local(query(&snapshot, &request(PrivacyProfile::Local)))
        .time
        .timeline;
    timeline.tracks[0].evidence_scope = FragmentEvidence::Unavailable;
    timeline.tracks[0].evidence_refs.clear();
    let result = navigation::build(
        &snapshot,
        TurnTarget {
            source: "source-private",
            thread: "thread-private",
            turn: "turn-private",
        },
        &timeline,
        Some(&events),
        None,
        &AtomicBool::new(false),
    )
    .unwrap();
    assert_eq!(result.detail.support, Support::Partial);
    assert_eq!(result.detail.reason, Basis::MissingIdentity);
    assert_eq!(result.candidate_interval_count.value, Some(1));
    assert_eq!(result.located_interval_count.value, Some(0));
    assert_eq!(result.missing_event_ref_count.value, None);
    assert_eq!(result.missing_event_ref_count.basis, Basis::MissingIdentity);
    assert!(result.entries.is_empty());
}

fn work_operation(
    id: &str,
    kind: &str,
    status: &str,
    work: Option<crate::adapters::contract::WorkObservation>,
) -> Arc<crate::adapters::contract::Operation> {
    let mut op:crate::adapters::contract::Operation=serde_json::from_value(serde_json::json!({"id":id,"threadId":"thread-private","turnId":"turn-private","callId":id,"kind":kind,"name":"safe","sequence":1,"timePrecision":"unknown","status":status,"outcomeConflict":false,"evidence":[]})).unwrap();
    op.work = work;
    Arc::new(op)
}
fn file_work(paths: &[(&str, Option<&str>)]) -> crate::adapters::contract::WorkObservation {
    use crate::adapters::contract::*;
    WorkObservation {
        format_version: WORK_OBSERVATION_VERSION,
        stage: WorkStage::Terminal,
        data: WorkData::FileChange {
            changes: Some(
                paths
                    .iter()
                    .map(|(path, moved)| FilePathChange {
                        path: (*path).into(),
                        change: ChangeKind::Update,
                        move_path: moved.map(str::to_owned),
                    })
                    .collect(),
            ),
        },
        gaps: vec![],
    }
}
fn work_snapshot(
    operations: Vec<Arc<crate::adapters::contract::Operation>>,
    events: Vec<Arc<Event>>,
    partial: bool,
) -> Snapshot {
    let mut collected = data(events);
    collected.operations = operations;
    if partial {
        collected.sources[0].status = "partial".into();
    }
    let root = tempfile::tempdir().unwrap();
    usage_store::memory(
        collected,
        "live:private".into(),
        crate::pricing_sync::current_at(root.path()).unwrap(),
        None,
    )
    .unwrap()
}
#[test]
fn work_summary_consumes_canonical_operations_and_reported_terminal_paths() {
    let snapshot = work_snapshot(
        vec![
            work_operation(
                "failed",
                "file",
                "failed",
                Some(file_work(&[("/private-work/a", Some("/private-work/b"))])),
            ),
            work_operation(
                "declined",
                "file",
                "declined",
                Some(file_work(&[("/private-work/a", None)])),
            ),
            work_operation("active", "command", "running", None),
        ],
        complete_events(),
        true,
    );
    let result = local(query(&snapshot, &request(PrivacyProfile::Local)));
    assert_eq!(result.work.outcomes.determinate_operations.value, Some(1));
    assert_eq!(result.work.outcomes.failed.value, Some(1));
    assert_eq!(result.work.outcomes.rejected.value, Some(1));
    assert_eq!(result.work.outcomes.nonterminal.value, Some(1));
    assert_eq!(result.work.outcomes.failure_ratio.value, Some(1.0));
    assert!(result.work.outcomes.partial);
    assert_eq!(result.work.operation_candidates.value, Some(3));
    assert_eq!(result.work.closed_operations.value, Some(2));
    assert_eq!(result.work.failed_operations.value, Some(1));
    assert_eq!(result.work.file_change_records.value, Some(2));
    assert_eq!(result.work.changed_files.value, Some(2));
    assert_eq!(result.work.changed_files.basis, Basis::ReportedFilePaths);
    assert_eq!(result.work.changed_files.status, MetricStatus::Derived);
    assert_eq!(
        result.work.operation_candidates.status,
        MetricStatus::Observed
    );
    assert_eq!(
        result.work.changed_files.evidence_refs,
        vec!["collection:canonical_operations"]
    );
    assert_eq!(result.work.added_lines.value, None);
    assert_eq!(
        result.work.added_lines.basis,
        Basis::MissingRepositoryBaseline
    );
    assert_eq!(
        result.work.labelled_command_ms.basis,
        Basis::UnsupportedMethod
    );
    assert!(result.quality.reason_codes.contains(&Basis::SourcePartial));
    let Response::Share(shared) = query(&snapshot, &request(PrivacyProfile::ShareV1)) else {
        panic!()
    };
    assert_eq!(shared.work.outcomes.failure_ratio.value, Some(1.0));
    assert_eq!(shared.work.outcomes.failed.value, Some(1));
    assert!(
        shared
            .work
            .outcomes
            .failed
            .evidence_refs
            .iter()
            .all(|r| shared.basis_collections.iter().any(|c| &c.reference == r))
    );
    assert_eq!(shared.work.changed_files.value, Some(2));
    assert!(shared.basis_collections.iter().any(|c| {
        shared
            .work
            .changed_files
            .evidence_refs
            .contains(&c.reference)
    }));
    assert!(
        !serde_json::to_string(&shared)
            .unwrap()
            .contains("/private-work")
    );
}
#[test]
fn work_missing_targets_proposed_and_source_budgets_are_not_zero() {
    let snapshot = work_snapshot(
        vec![work_operation(
            "relative",
            "file",
            "completed",
            Some(file_work(&[("a", None)])),
        )],
        complete_events(),
        false,
    );
    let result = local(query(&snapshot, &request(PrivacyProfile::Local)));
    assert_eq!(result.work.changed_files.value, None);
    assert_eq!(result.work.changed_files.basis, Basis::MissingTarget);
    assert!(result.quality.reason_codes.contains(&Basis::MissingTarget));
    let mut proposed = file_work(&[("/synthetic/a", None)]);
    proposed.stage = crate::adapters::contract::WorkStage::Proposed;
    let snapshot = work_snapshot(
        vec![work_operation(
            "proposed",
            "file",
            "running",
            Some(proposed),
        )],
        complete_events(),
        false,
    );
    let result = local(query(&snapshot, &request(PrivacyProfile::Local)));
    assert_eq!(result.work.changed_files.value, None);
    assert_eq!(result.work.file_change_records.value, Some(1));
    assert_eq!(result.work.closed_operations.value, Some(0));
}
#[test]
fn work_native_conflicts_and_resource_gaps_keep_distinct_reasons() {
    for (gap, basis) in [
        (
            crate::adapters::contract::WorkGap::ConflictingObservation,
            Basis::BoundaryConflict,
        ),
        (
            crate::adapters::contract::WorkGap::ResourceLimit,
            Basis::ResourceLimit,
        ),
    ] {
        let mut observed = file_work(&[]);
        observed.data = crate::adapters::contract::WorkData::FileChange { changes: None };
        observed.gaps = vec![gap];
        let snapshot = work_snapshot(
            vec![work_operation("file", "file", "failed", Some(observed))],
            complete_events(),
            false,
        );
        let result = local(query(&snapshot, &request(PrivacyProfile::Local)));
        assert_eq!(result.work.file_change_records.value, Some(1));
        assert_eq!(result.work.failed_operations.value, Some(1));
        assert_eq!(result.work.changed_files.value, None);
        assert_eq!(result.work.changed_files.basis, basis);
        assert!(result.quality.reason_codes.contains(&basis));
    }
}
#[test]
fn work_safe_message_counts_preserve_unknown_user_provenance_and_source_gap() {
    let mut events = complete_events();
    for (i, origin) in [
        MessageOrigin::UserInput,
        MessageOrigin::UserUnclassified,
        MessageOrigin::InjectedContext,
        MessageOrigin::Reasoning,
        MessageOrigin::Inherited,
        MessageOrigin::InterAgent,
    ]
    .into_iter()
    .enumerate()
    {
        events.push(event(
            20 + i as u64,
            None,
            Payload::Message {
                origin,
                presence: ContentPresence::Empty,
                native_id: None,
                record_kind: MessageRecordKind::LegacySnapshot,
                record_phase: Phase::Completed,
                content_phase: ContentPhase::Unknown,
            },
        ));
    }
    let snapshot = work_snapshot(vec![], events, true);
    let result = local(query(&snapshot, &request(PrivacyProfile::Local)));
    assert_eq!(result.work.user_boundary_records.value, None);
    assert_eq!(
        result.work.user_boundary_records.basis,
        Basis::UnknownMessageOrigin
    );
    assert_eq!(result.work.injected_context_records.value, Some(1));
    assert_eq!(result.work.reasoning_message_records.value, Some(1));
    assert!(
        result
            .quality
            .reason_codes
            .contains(&Basis::UnknownMessageOrigin)
    );
    assert!(result.quality.reason_codes.contains(&Basis::SourcePartial));
}
#[test]
fn work_observed_empty_and_fallback_unavailable_are_distinct() {
    let snapshot = make_snapshot(complete_events());
    let result = local(query(&snapshot, &request(PrivacyProfile::Local)));
    assert_eq!(result.work.operation_candidates.value, Some(0));
    assert_eq!(result.work.changed_files.value, Some(0));
    assert_eq!(result.work.user_boundary_records.value, Some(0));
    let analysis = analysis::analyze(analysis::AnalyzeInput {
        source: "source",
        thread: "thread",
        turn: "turn",
        measurements: &[],
        events: &[],
        budget: analysis::Budget {
            events: 0,
            measurements: 0,
            lifecycle_records: 0,
        },
    });
    let mapped = m::work(
        &analysis,
        &[],
        &[],
        None,
        Some(Basis::ResourceLimit),
        Some(Basis::ResourceLimit),
    );
    assert_eq!(mapped.changed_files.value, None);
    assert_eq!(mapped.changed_files.basis, Basis::ResourceLimit);
    assert_eq!(mapped.user_boundary_records.value, None);
    let mapped = m::work(&analysis, &[], &[], None, None, Some(Basis::ResourceLimit));
    assert_eq!(mapped.operation_candidates.value, None);
    assert_eq!(mapped.operation_candidates.basis, Basis::ResourceLimit);
    assert_eq!(mapped.user_boundary_records.value, Some(0));
}

#[test]
fn mcp_time_is_delivered_in_local_and_private_relative_share_projection() {
    let operation = event(
        2,
        Some(1070),
        Payload::Item {
            item_kind: crate::session_events::ItemKind::Mcp,
            native_id: Some("mcp-item-private".into()),
            phase: Phase::Completed,
            started_at_ms: Some(1010),
            completed_at_ms: Some(1060),
            duration: None,
        },
    );
    let snapshot = make_snapshot(vec![
        boundary(0, Some(1000), Phase::Started, None, None),
        operation.clone(),
        boundary(3, Some(1100), Phase::Completed, Some(100), None),
    ]);
    let result = local(query(&snapshot, &request(PrivacyProfile::Local)));
    assert_eq!(result.method_version, "safe_event_turn_v7");
    assert_eq!(result.time.mcp.union_ms.value, Some(50));
    assert_eq!(result.time.mcp.sum_ms.value, Some(50));
    assert_eq!(result.time.mcp.closed.value, Some(1));
    assert_eq!(result.time.intersection_masks_ms.len(), 16);
    assert_eq!(result.time.intersection_masks_ms[8].value, Some(50));
    assert_eq!(result.coverage.lifecycle_candidates.len(), 4);
    assert_eq!(result.time.timeline.tracks[0].category, TrackCategory::Mcp);
    assert_eq!(
        (
            result.time.timeline.tracks[0].start_ms,
            result.time.timeline.tracks[0].end_ms
        ),
        (10, 60)
    );
    let shared = super::super::share::project(&result);
    assert_eq!(shared.time.mcp.union_ms.value, Some(50));
    assert_eq!(shared.time.timeline.tracks[0].category, TrackCategory::Mcp);
    assert_eq!(
        (
            shared.time.timeline.tracks[0].start_ms,
            shared.time.timeline.tracks[0].end_ms
        ),
        (10, 60)
    );
    let encoded = serde_json::to_string(&shared).unwrap();
    for private in [
        operation.id(),
        "mcp-item-private",
        "file-private",
        "generation-private",
        "thread-private",
        "source-private",
    ] {
        assert!(!encoded.contains(private), "{private}");
    }
    let schema = crate::dispatch("schema_timing_local_response", &serde_json::json!({})).unwrap();
    assert_eq!(
        schema["definitions"]["Time"]["properties"]["intersectionMasksMs"]["minItems"],
        16
    );
    assert_eq!(
        schema["definitions"]["Time"]["properties"]["intersectionMasksMs"]["maxItems"],
        16
    );
    assert_eq!(
        schema["definitions"]["Coverage"]["properties"]["lifecycleCandidates"]["minItems"],
        4
    );
}

#[test]
fn mcp_target_and_outcome_conflicts_preserve_time_and_report_local_quality() {
    let snapshot = make_snapshot(vec![
        boundary(0, Some(1_000), Phase::Started, None, None),
        mcp_operation_event(10, 1_010, "target-call", "docs", Phase::Started, "running"),
        mcp_operation_event(
            20,
            1_030,
            "target-call",
            "docs",
            Phase::Completed,
            "completed",
        ),
        mcp_operation_event(
            21,
            1_030,
            "target-call",
            "other",
            Phase::Completed,
            "completed",
        ),
        mcp_operation_event(30, 1_040, "outcome-call", "docs", Phase::Started, "running"),
        mcp_operation_event(
            40,
            1_060,
            "outcome-call",
            "docs",
            Phase::Completed,
            "completed",
        ),
        mcp_operation_event(41, 1_060, "outcome-call", "docs", Phase::Failed, "failed"),
        boundary(100, Some(1_100), Phase::Completed, Some(100), None),
    ]);
    let result = local(query(&snapshot, &request(PrivacyProfile::Local)));
    assert_eq!(result.time.mcp.union_ms.value, Some(40));
    assert_eq!(result.time.mcp.sum_ms.value, Some(40));
    assert_eq!(result.time.mcp.closed.value, Some(2));
    assert!(result.quality.partial);
    assert!(result.quality.reason_codes.contains(&Basis::TargetConflict));
    assert!(
        result
            .quality
            .reason_codes
            .contains(&Basis::OutcomeConflict)
    );
    assert!(
        !result
            .quality
            .reason_codes
            .contains(&Basis::MissingIdentity)
    );
    assert!(!result.quality.reason_codes.contains(&Basis::MissingTime));
}

fn interval_item(
    offset: u64,
    kind: crate::session_events::ItemKind,
    id: Option<&str>,
    start: i64,
    end: i64,
) -> Arc<Event> {
    event(
        offset,
        None,
        Payload::Item {
            item_kind: kind,
            native_id: id.map(str::to_owned),
            phase: Phase::Completed,
            started_at_ms: Some(start),
            completed_at_ms: Some(end),
            duration: None,
        },
    )
}
#[test]
fn operation_residual_is_distinct_from_category_gaps_and_share_is_private() {
    use crate::session_events::ItemKind;
    let snapshot = make_snapshot(vec![
        boundary(0, Some(0), Phase::Started, None, None),
        interval_item(1, ItemKind::Tool, Some("tool-private"), 2000, 10000),
        interval_item(2, ItemKind::File, Some("file-private"), 8000, 12000),
        interval_item(3, ItemKind::Reasoning, Some("reason-private"), 12000, 30000),
        interval_item(4, ItemKind::Compaction, Some("compact-private"), 0, 2000),
        boundary(5, Some(30000), Phase::Completed, Some(31000), None),
    ]);
    let l = local(query(&snapshot, &request(PrivacyProfile::Local)));
    assert_eq!(l.output_version, 6);
    assert_eq!(l.time.native_wall_clock_ms.value, Some(31000));
    assert_eq!(l.time.unclassified_ms.value, Some(10000));
    let c = &l.time.operation_coverage;
    assert_eq!(c.method_version, 1);
    assert_eq!(c.endpoint_method_version, 1);
    assert_eq!(c.candidate_operations.value, Some(2));
    assert_eq!(c.paired_operations.value, Some(2));
    assert_eq!(c.covered_ms.value, Some(10000));
    assert_eq!(c.covered_ms.basis, Basis::OperationUnion);
    assert_eq!(c.residual_ms.value, Some(20000));
    assert_eq!(c.residual_ms.basis, Basis::OperationResidual);
    assert_eq!(c.residual_ms.status, MetricStatus::Derived);
    assert_eq!(c.residual_range_count.value, Some(2));
    assert_eq!(
        c.residual_ranges
            .iter()
            .map(|r| (r.start_ms, r.end_ms))
            .collect::<Vec<_>>(),
        vec![(0, 2000), (12000, 30000)]
    );
    assert!(!c.partial);
    let Response::Share(s) = query(&snapshot, &request(PrivacyProfile::ShareV1)) else {
        panic!()
    };
    assert_eq!(
        s.time.operation_coverage.residual_ms.value,
        c.residual_ms.value
    );
    assert_eq!(
        s.time.operation_coverage.covered_ms.value,
        c.covered_ms.value
    );
    let text = serde_json::to_string(&s).unwrap();
    for private in [
        "source-private",
        "file-private",
        "tool-private",
        "thread-private",
        "turn-private",
        "live:private",
    ] {
        assert!(!text.contains(private));
    }
}
#[test]
fn operation_residual_without_pairs_preserves_zero_and_explains_no_subtraction() {
    for end in [0, 30000] {
        let l = local(query(
            &make_snapshot(vec![
                boundary(0, Some(0), Phase::Started, None, None),
                boundary(1, Some(end), Phase::Completed, None, None),
            ]),
            &request(PrivacyProfile::Local),
        ));
        let c = l.time.operation_coverage;
        assert_eq!(c.candidate_operations.value, Some(0));
        assert_eq!(c.paired_operations.value, Some(0));
        assert_eq!(c.covered_ms.value, Some(0));
        assert_eq!(c.residual_ms.value, Some(end as u64));
        assert_eq!(c.residual_range_count.value, Some(u64::from(end > 0)));
        assert!(
            c.reason_codes
                .contains(&OperationCoverageReason::NoPairedOperations)
        );
        assert_eq!(c.detail.support, Support::Supported);
    }
}
#[test]
fn operation_residual_retains_values_with_identity_and_source_gaps() {
    use crate::session_events::ItemKind;
    let mut events = vec![
        boundary(0, Some(0), Phase::Started, None, None),
        interval_item(1, ItemKind::Tool, Some("paired"), 2000, 10000),
        interval_item(2, ItemKind::Tool, None, 10000, 12000),
        boundary(4, Some(30000), Phase::Completed, None, None),
    ];
    events.push(event(
        3,
        Some(13000),
        Payload::Item {
            item_kind: ItemKind::File,
            native_id: Some("open".into()),
            phase: Phase::Started,
            started_at_ms: Some(13000),
            completed_at_ms: None,
            duration: None,
        },
    ));
    let root = tempfile::tempdir().unwrap();
    let mut collected = data(events);
    collected.sources[0].status = "partial".into();
    let snapshot = usage_store::memory(
        collected,
        "live:private".into(),
        crate::pricing_sync::current_at(root.path()).unwrap(),
        None,
    )
    .unwrap();
    let c = local(query(&snapshot, &request(PrivacyProfile::Local)))
        .time
        .operation_coverage;
    assert_eq!(c.candidate_operations.value, Some(2));
    assert_eq!(c.paired_operations.value, Some(1));
    assert_eq!(c.identity_gap_records.value, Some(1));
    assert_eq!(c.covered_ms.value, Some(8000));
    assert_eq!(c.residual_ms.value, Some(22000));
    assert!(c.partial);
    for reason in [
        OperationCoverageReason::IdentityGaps,
        OperationCoverageReason::UnlocatedOperations,
        OperationCoverageReason::SourcePartial,
    ] {
        assert!(c.reason_codes.contains(&reason));
    }
}
#[test]
fn operation_residual_detail_limit_preserves_full_totals_and_range_count() {
    use crate::session_events::ItemKind;
    let mut events = vec![boundary(0, Some(0), Phase::Started, None, None)];
    for i in 0..250 {
        events.push(interval_item(
            i + 1,
            ItemKind::Tool,
            Some(&format!("tool-{i}")),
            (i * 2) as i64,
            (i * 2 + 1) as i64,
        ));
    }
    events.push(boundary(251, Some(1000), Phase::Completed, None, None));
    let c = local(query(
        &make_snapshot(events),
        &request(PrivacyProfile::Local),
    ))
    .time
    .operation_coverage;
    assert_eq!(c.paired_operations.value, Some(250));
    assert_eq!(c.covered_ms.value, Some(250));
    assert_eq!(c.residual_ms.value, Some(750));
    assert_eq!(c.residual_range_count.value, Some(250));
    assert!(!c.partial);
    assert!(c.residual_ranges.is_empty());
    assert_eq!(c.detail.support, Support::Unavailable);
    assert!(
        c.reason_codes
            .contains(&OperationCoverageReason::DetailLimit)
    );
}

#[test]
fn operation_pairs_without_turn_window_explain_unavailable_residual_instead_of_zero() {
    let l = local(query(
        &make_snapshot(vec![interval_item(
            0,
            crate::session_events::ItemKind::Tool,
            Some("pair"),
            2000,
            10000,
        )]),
        &request(PrivacyProfile::Local),
    ));
    let c = l.time.operation_coverage;
    assert_eq!(c.paired_operations.value, Some(1));
    assert!(c.partial);
    assert_eq!(c.residual_ms.value, None);
    assert_eq!(c.covered_ms.value, None);
    assert_eq!(c.residual_range_count.value, None);
    assert!(
        c.reason_codes
            .contains(&OperationCoverageReason::MissingWindow)
    );
    assert!(
        !c.reason_codes
            .contains(&OperationCoverageReason::ResourceLimit)
    );
    assert!(c.residual_ranges.is_empty());
}

#[path = "tests/repeats.rs"]
mod repeats;

#[test]
fn input_change_is_shared_by_fixed_turn_summary_inspection_and_private_share() {
    let measurements: Vec<Arc<crate::adapters::contract::Measurement>>=[100_u64,20000].into_iter().enumerate().map(|(i,n)|Arc::new(serde_json::from_value(serde_json::json!({
        "id":format!("measurement-private-{i}"),"agentKind":"codex","sourceInstanceId":"source-private",
        "threadId":"thread-private","turnId":"turn-private","grain":"response","timePrecision":"unknown",
        "model":{"raw":"model"},"reasoningEffort":"high","tokens":{"rawInput":n},"requestScoped":true,
        "tokenUnavailableReasons":{"input":"missing","cacheRead":"missing","cacheCreate":"missing","output":"missing","reasoning":"missing","total":"missing","rawInput":null},"pricingContextConflict":false,"sequence":i,"evidence":[]
    })).unwrap())).collect();
    let events = measurements
        .iter()
        .enumerate()
        .map(|(i, value)| {
            event(
                i as u64 + 1,
                None,
                Payload::Measurement {
                    value: value.clone(),
                    context_conflicts: vec![],
                    direct: true,
                    cumulative: None,
                    interval_start: None,
                    fingerprint: format!("sample-{i}"),
                },
            )
        })
        .collect();
    let mut collected = data(events);
    collected.measurements = measurements.clone();
    let root = tempfile::tempdir().unwrap();
    let snapshot = usage_store::memory(
        collected,
        "live:private".into(),
        crate::pricing_sync::current_at(root.path()).unwrap(),
        None,
    )
    .unwrap();
    let l = local(query(&snapshot, &request(PrivacyProfile::Local)));
    let stats = l.context.input_change.statistics.as_ref().unwrap();
    assert_eq!(stats.maximum_increase.value, Some(19900));
    assert_eq!(stats.maximum_increase.status, MetricStatus::Derived);
    assert_eq!(stats.comparable_stages, 1);
    assert_eq!(stats.largest_increase.as_ref().unwrap().first_input, 100);
    let activity = crate::optimize::activity::evaluate(&l);
    assert_eq!(activity.checks[4].observed.value, Some(19900));
    assert_eq!(
        activity.checks[4].outcome,
        crate::optimize_dto::RuleOutcome::Hit
    );
    assert!(
        activity
            .advice
            .contains(&crate::optimize_dto::ActivityRule::InspectInputChange)
    );
    let shared = share::project(&l);
    let ss = shared.context.input_change.statistics.unwrap();
    assert_eq!(ss.maximum_increase.value, Some(19900));
    assert_eq!(ss.largest_increase.as_ref().unwrap().delta, 19900);
    assert_ne!(
        stats.largest_increase.as_ref().unwrap().first_ref,
        ss.largest_increase.as_ref().unwrap().first_ref
    );
    let json = serde_json::to_string(&ss).unwrap();
    assert!(!json.contains("private"));
}
