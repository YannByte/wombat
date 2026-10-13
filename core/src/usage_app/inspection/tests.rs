use super::*;
use crate::session_events::{Event, Position, Time};
use std::sync::Arc;

pub(in crate::usage_app) fn snapshot(
    records: &[(&str, &str, &str, u64)],
    suffix: &str,
) -> Snapshot {
    let root = tempfile::tempdir().unwrap();
    let source = SourceInstance {
        id: "synthetic".into(),
        agent_kind: "codex".into(),
        root: "/synthetic".into(),
    };
    let mut collected = Collected::default();
    collected.sources.push(SourceReport {
        source,
        adapter_version: "synthetic".into(),
        source_versions: vec![],
        capabilities: Capabilities::default(),
        status: "complete".into(),
        files_read: 0,
        bytes_read: 0,
        issues: vec![],
    });
    let mut ids = BTreeSet::new();
    for (id, thread, at, n) in records {
        if ids.insert(*thread) {
            collected.threads.push(Thread {
                id: (*thread).into(),
                agent_kind: "codex".into(),
                source_instance_id: "synthetic".into(),
                upstream_id: (*thread).into(),
                title: None,
                project: Some(format!("/{thread}")),
                started_at: None,
                last_activity_at: None,
            });
        }
        let tokens = TokenUsage {
            input: Some(*n),
            cache_read: Some(0),
            cache_create: Some(0),
            output: Some(0),
            reasoning: Some(0),
            total: Some(*n),
            raw_input: Some(*n),
        };
        let reasons = tokens.unavailable_reasons(TokenUnavailableReason::Missing);
        collected.measurements.push(Arc::new(Measurement {
            id: (*id).into(),
            agent_kind: "codex".into(),
            source_instance_id: "synthetic".into(),
            thread_id: Some((*thread).into()),
            turn_id: None,
            response_id: Some((*id).into()),
            timestamp: Some((*at).into()),
            interval_end: None,
            grain: "response".into(),
            time_precision: "second".into(),
            model: ModelRef {
                raw: Some("gpt-5.4".into()),
                ..Default::default()
            },
            reasoning_effort: Some("high".into()),
            tokens,
            token_unavailable_reasons: reasons,
            pricing_context_conflict: false,
            request_scoped: true,
            reported_cost: None,
            service_tier: None,
            sequence: 0,
            evidence: vec![],
        }));
    }
    crate::usage_store::memory(
        collected,
        format!("live:synthetic:{suffix}"),
        crate::pricing_sync::current_at(root.path()).unwrap(),
        None,
    )
    .unwrap()
}
fn request(value: Value) -> Request {
    serde_json::from_value(value).unwrap()
}

#[test]
fn pure_checks_preserve_threshold_edges_and_ignore_unavailable_statistics() {
    let s = snapshot(
        &[("1", "a", "2026-09-02T00:00:00Z", 1_000_000)],
        "pure-rules",
    );
    let result = execute(
        request(serde_json::json!({"action":"investigate","scope":{"allTime":true}})),
        &s,
    )
    .unwrap();
    let mut c = result.inspection.unwrap().candidates.remove(0);
    let p = InspectionPolicy::default();
    c.input = Some(100_000);
    c.cache_share = Some(0.2);
    c.largest_uncached_jump = Some(100_000);
    c.determinate_operations = 5;
    c.failed_operations = 2;
    c.repeated_requests = 3;
    assert_eq!(
        rules::signals(&c, &p),
        vec![
            InspectionSignal::HighUsage,
            InspectionSignal::LowCacheReuse,
            InspectionSignal::InputJump,
            InspectionSignal::FailureShare,
            InspectionSignal::RepeatedRequest
        ]
    );
    c.input = Some(99_999);
    c.largest_uncached_jump = Some(99_999);
    c.determinate_operations = 4;
    c.repeated_requests = 2;
    assert_eq!(rules::signals(&c, &p), vec![InspectionSignal::HighUsage]);
    c.input = Some(100_000);
    c.cache_share = Some(0.2001);
    c.determinate_operations = 10;
    c.failed_operations = 3;
    assert_eq!(rules::signals(&c, &p), vec![InspectionSignal::HighUsage]);
    c.input = None;
    c.cache_share = None;
    c.largest_uncached_jump = None;
    c.determinate_operations = 0;
    c.failed_operations = 0;
    assert_eq!(rules::signals(&c, &p), vec![InspectionSignal::HighUsage]);
}

#[test]
fn candidates_preserve_missing_order_and_scope_and_thresholds() {
    let s = snapshot(
        &[
            ("1", "a", "2026-09-02T00:00:00Z", 600_000),
            ("2", "a", "2026-09-02T00:01:00Z", 500_000),
            ("3", "b", "2026-09-02T00:00:00Z", 50),
        ],
        "one",
    );
    let r = execute(
        request(serde_json::json!({"action":"investigate","scope":{"allTime":true},"limit":1})),
        &s,
    )
    .unwrap();
    let i = r.inspection.unwrap();
    assert_eq!(i.candidate_count, 1);
    let c = &i.candidates[0];
    assert_eq!(c.usage.complete_token_total(), Some(1_100_000));
    assert_eq!(c.cache_share, Some(0.0));
    assert_eq!(
        c.signals,
        vec![InspectionSignal::HighUsage, InspectionSignal::LowCacheReuse]
    );
    assert_eq!(c.largest_uncached_jump, None);
    assert_eq!(
        c.evidence[0].snapshot_id,
        s.manifest.snapshot_ref.snapshot_id
    );
    assert_eq!(c.evidence[0].scope.all_time, Some(true));
    let r = execute(
        request(
            serde_json::json!({"action":"investigate","scope":{"project":"/b","allTime":true}}),
        ),
        &s,
    )
    .unwrap();
    assert_eq!(r.inspection.unwrap().candidate_count, 0);
    assert_eq!(r.summary.complete_token_total(), Some(50));
}
fn positioned(s: Snapshot, compact: bool, model_change: bool) -> Snapshot {
    positioned_with(s, compact, model_change, |_, _| {})
}
fn positioned_with(
    s: Snapshot,
    compact: bool,
    model_change: bool,
    update: fn(&mut Measurement, usize),
) -> Snapshot {
    let mut collected = Collected {
        sources: s.manifest.sources.clone(),
        threads: s
            .manifest
            .threads
            .iter()
            .map(|t| t.thread.clone())
            .collect(),
        ..Default::default()
    };
    for (i, row) in s.ledger().unwrap().iter().enumerate() {
        let mut m = (*row.fact).clone();
        if model_change && i > 0 {
            m.model.raw = Some("other-model".into());
        }
        update(&mut m, i);
        m.token_unavailable_reasons = m
            .tokens
            .unavailable_reasons(TokenUnavailableReason::Missing);
        let value = Arc::new(m);
        let pos = |ordinal| Position {
            source_instance_id: "synthetic".into(),
            file_id: "fixture".into(),
            generation: "one".into(),
            byte_offset: i as u64 * 10,
            ordinal,
        };
        if compact && i > 0 {
            collected.events.push(Arc::new(
                Event::new(
                    pos(0),
                    Some("a".into()),
                    None,
                    Time::from_source(value.timestamp.as_deref()).0,
                    vec![],
                    Payload::Lifecycle {
                        lifecycle: LifecycleKind::Compaction,
                        phase: crate::session_events::Phase::Unknown,
                        native_id: None,
                        duration_ms: None,
                        first_token_ms: None,
                    },
                )
                .unwrap(),
            ));
        }
        collected.events.push(Arc::new(
            Event::new(
                pos(1),
                value.thread_id.as_deref().map(str::to_owned),
                value.turn_id.as_deref().map(str::to_owned),
                Time::from_source(value.timestamp.as_deref()).0,
                vec![],
                Payload::Measurement {
                    value: Arc::clone(&value),
                    context_conflicts: vec![],
                    direct: true,
                    cumulative: None,
                    interval_start: None,
                    fingerprint: format!("f{i}"),
                },
            )
            .unwrap(),
        ));
        collected.measurements.push(value);
    }
    let root = tempfile::tempdir().unwrap();
    crate::usage_store::memory(
        collected,
        "live:synthetic:positioned".into(),
        crate::pricing_sync::current_at(root.path()).unwrap(),
        None,
    )
    .unwrap()
}
#[test]
fn trajectory_uses_native_position_and_breaks_at_compaction_and_model_change() {
    for (compact, model, boundary, delta) in [
        (false, false, None, Some(100_000)),
        (true, false, Some(InputBoundary::Compaction), None),
        (false, true, Some(InputBoundary::ModelChange), None),
    ] {
        let s = positioned(
            snapshot(
                &[
                    ("1", "a", "2026-09-02T00:00:00Z", 200_000),
                    ("2", "a", "2026-09-02T00:01:00Z", 300_000),
                ],
                "one",
            ),
            compact,
            model,
        );
        let r=execute(request(serde_json::json!({"action":"trajectory","threadId":"a","scope":{"allTime":true},"limit":20})),&s).unwrap();
        let i = r.inspection.unwrap();
        assert_eq!(i.trajectory.len(), 2);
        assert_eq!(i.trajectory[0].input_delta, None);
        assert_eq!(i.trajectory[1].boundary, boundary);
        assert_eq!(i.trajectory[1].uncached_delta, delta);
        assert!(
            i.limitations
                .contains(&InspectionLimit::ContextOccupancyUnavailable)
        );
    }
}
#[test]
fn trajectory_preserves_zero_decreases_native_order_and_missing_input_boundaries() {
    for (first, second, expected) in [(200, 200, 0), (300, 200, -100), (200, 300, 100)] {
        let s = positioned(
            snapshot(
                &[
                    ("1", "a", "2026-09-02T00:01:00Z", first),
                    ("2", "a", "2026-09-02T00:00:00Z", second),
                ],
                "late",
            ),
            false,
            false,
        );
        let result = execute(
            request(
                serde_json::json!({"action":"trajectory","threadId":"a","scope":{"allTime":true}}),
            ),
            &s,
        )
        .unwrap();
        assert_eq!(
            result.inspection.unwrap().trajectory[1].uncached_delta,
            Some(expected)
        );
    }
    let s = positioned_with(
        snapshot(
            &[
                ("1", "a", "2026-09-02T00:00:00Z", 200),
                ("2", "a", "2026-09-02T00:00:00Z", 300),
                ("3", "a", "2026-09-02T00:00:00Z", 400),
            ],
            "missing",
        ),
        false,
        false,
        |m, index| {
            if index == 1 {
                m.tokens.raw_input = None;
            }
        },
    );
    let result = execute(
        request(serde_json::json!({"action":"trajectory","threadId":"a","scope":{"allTime":true}})),
        &s,
    )
    .unwrap();
    let points = result.inspection.unwrap().trajectory;
    for point in &points[1..] {
        assert_eq!(point.boundary, Some(InputBoundary::MissingInput));
        assert_eq!(point.input_delta, None);
        assert_eq!(point.uncached_delta, None);
    }
}
#[test]
fn event_budget_is_shared_across_reads_and_rejects_before_decoding() {
    let mut s = positioned(
        snapshot(&[("1", "a", "2026-09-02T00:00:00Z", 200)], "budget"),
        false,
        false,
    );
    let thread = s.manifest.threads[0].clone();
    let bytes = s.manifest.events.partitions[0]
        .chunks
        .iter()
        .map(|c| c.bytes)
        .sum::<u64>();
    let mut work = EventWork {
        facts: 0,
        bytes: MAX_EVENT_BYTES - bytes,
    };
    assert_eq!(
        order_map(&thread_events(&s, &thread, &mut work).unwrap())
            .unwrap()
            .len(),
        1
    );
    // A subsequent read exceeds the aggregate budget even if its payload cannot decode.
    Arc::make_mut(&mut s.manifest.events.partitions[0]).chunks[0]
        .file
        .sha256 = "invalid".into();
    let error = thread_events(&s, &thread, &mut work).err().unwrap();
    assert_eq!(
        error
            .downcast_ref::<crate::dto::OperationError>()
            .unwrap()
            .code,
        "RESOURCE_LIMIT"
    );
}
#[test]
fn review_comparison_reconciles_and_invalid_queries_fail_closed() {
    let s = snapshot(
        &[
            ("1", "a", "2026-09-02T00:00:00Z", 100),
            ("2", "a", "2026-09-09T00:00:00Z", 140),
            ("3", "b", "2026-09-09T00:00:00Z", 20),
        ],
        "one",
    );
    let r=execute(request(serde_json::json!({"action":"review","scope":{"since":"2026-09-08","until":"2026-09-15","timezone":"UTC"}})),&s).unwrap();
    let i = r.inspection.unwrap();
    let review = i.review.unwrap();
    assert_eq!(r.summary.complete_token_total(), Some(160));
    assert_eq!(review.top_tasks.len(), 2);
    assert_eq!(review.models[0].usage.complete_token_total(), Some(160));
    let Some(Comparison::Periods { delta, .. }) = review.comparison else {
        panic!()
    };
    assert_eq!(delta.tokens, Some(60));
    for json in [
        serde_json::json!({"action":"trajectory"}),
        serde_json::json!({"action":"resources","sort":"tokens"}),
        serde_json::json!({"action":"review","scope":{"allTime":true}}),
        serde_json::json!({"action":"review","limit":10}),
    ] {
        assert!(crate::usage_app::validate(&request(json)).is_err());
    }
}

fn resource_facts() -> Collected {
    let base = snapshot(
        &[
            ("1", "a", "2026-09-02T00:00:00Z", 1),
            ("2", "b", "2026-09-02T00:00:00Z", 1),
        ],
        "one",
    );
    let mut collected = Collected {
        sources: base.manifest.sources.clone(),
        threads: base
            .manifest
            .threads
            .iter()
            .map(|t| t.thread.clone())
            .collect(),
        measurements: base
            .ledger()
            .unwrap()
            .into_iter()
            .map(|r| {
                let mut fact = (*r.fact).clone();
                fact.turn_id = Some("turn".into());
                Arc::new(fact)
            })
            .collect(),
        ..Default::default()
    };
    for (i, thread) in ["a", "a", "a", "a", "a", "a", "b"].iter().enumerate() {
        collected.operations.push(Arc::new(Operation {
            text_result: None,
            id: format!("op{i}"),
            thread_id: (*thread).into(),
            turn_id: Some("turn".into()),
            item_id: None,
            call_id: None,
            response_id: None,
            kind: "tool".into(),
            name: "read_file".into(),
            sequence: i as u64,
            timestamp: Some("2026-09-02T00:00:00Z".into()),
            time_precision: "second".into(),
            status: if [4, 5].contains(&i) {
                "failed"
            } else {
                "completed"
            }
            .into(),
            exit_code: None,
            outcome_conflict: false,
            duration_ms: if i < 2 { Some(100) } else { None },
            path: None,
            work: None,
            matching: Some(OperationMatchObservation {
                format_version: OPERATION_MATCH_VERSION,
                receiver_owner: Some("synthetic".into()),
                request_fingerprint: None,
                function_request_fingerprint: Some("a".repeat(64)),
                read_targets: vec![ReadMatchTarget {
                    path: "/shared/path".into(),
                    platform: SourcePathPlatform::Posix,
                }],
                expected_nonzero: false,
                gaps: vec![],
            }),
            server: None,
            tool: None,
            evidence: vec![],
        }));
    }
    collected
}

#[test]
fn resources_keep_project_identity_unknown_duration_and_unverified_changes() {
    let collected = resource_facts();
    let root = tempfile::tempdir().unwrap();
    let s = crate::usage_store::memory(
        collected,
        "live:synthetic:resources".into(),
        crate::pricing_sync::current_at(root.path()).unwrap(),
        None,
    )
    .unwrap();
    let r = execute(
        request(serde_json::json!({"action":"resources","scope":{"allTime":true}})),
        &s,
    )
    .unwrap();
    let i = r.inspection.unwrap();
    assert_eq!(i.resource_count, 2);
    let a = &i.resources[0];
    assert_eq!(a.operations, 6);
    assert_eq!(a.failed_operations, 2);
    assert_eq!(a.known_duration_ms, Some(200));
    assert_eq!(a.duration_covered_operations, 2);
    assert_eq!(a.actual_changes, None);
    assert_ne!(a.id, i.resources[1].id);
    assert_eq!(i.resources[1].known_duration_ms, None);
    let r = execute(
        request(serde_json::json!({"action":"investigate","scope":{"allTime":true}})),
        &s,
    )
    .unwrap();
    let i = r.inspection.unwrap();
    assert_eq!(
        i.candidates[0].signals,
        vec![InspectionSignal::RepeatedRequest]
    );
    assert_eq!(i.candidates[0].determinate_operations, 6);
    assert_eq!(i.candidates[0].repeated_requests, 5);
    let filtered = execute(
        request(
            serde_json::json!({"action":"resources","scope":{"allTime":true,"model":"gpt-5.4"}}),
        ),
        &s,
    )
    .unwrap()
    .inspection
    .unwrap();
    assert_eq!(filtered.resources[0].operations, 6);
    assert!(
        filtered
            .limitations
            .contains(&InspectionLimit::OperationModelAssociation)
    );
}

#[test]
fn resources_do_not_read_unrelated_event_history_or_widen_inspection_budgets() {
    let mut collected = resource_facts();
    for offset in 0..=MAX_WORK {
        collected.events.push(Arc::new(
            Event::new(
                Position {
                    source_instance_id: "synthetic".into(),
                    file_id: "history".into(),
                    generation: "one".into(),
                    byte_offset: offset as u64,
                    ordinal: 0,
                },
                Some("a".into()),
                None,
                Time::from_source(Some("2026-09-02T00:00:00Z")).0,
                vec![],
                Payload::Lifecycle {
                    lifecycle: LifecycleKind::Compaction,
                    phase: crate::session_events::Phase::Unknown,
                    native_id: None,
                    duration_ms: None,
                    first_token_ms: None,
                },
            )
            .unwrap(),
        ));
    }
    let root = tempfile::tempdir().unwrap();
    let s = crate::usage_store::memory(
        collected,
        "live:synthetic:resource-history".into(),
        crate::pricing_sync::current_at(root.path()).unwrap(),
        None,
    )
    .unwrap();
    let result = execute(
        request(serde_json::json!({"action":"resources","scope":{"allTime":true}})),
        &s,
    )
    .unwrap();
    assert_eq!(result.summary.complete_token_total(), Some(2));
    let inspection = result.inspection.unwrap();
    assert_eq!(inspection.resource_count, 2);
    assert_eq!(inspection.resources[0].operations, 6);
    assert_eq!(inspection.resources[0].failed_operations, 2);
    assert_eq!(inspection.resources[0].known_duration_ms, Some(200));
    assert_eq!(inspection.resources[1].known_duration_ms, None);
    let error = execute(
        request(serde_json::json!({"action":"investigate","scope":{"allTime":true}})),
        &s,
    )
    .err()
    .unwrap();
    assert_eq!(
        error
            .downcast_ref::<crate::dto::OperationError>()
            .unwrap()
            .code,
        "RESOURCE_LIMIT"
    );
}
#[test]
fn filtered_model_cannot_bridge_an_intervening_model() {
    let s = positioned(
        snapshot(
            &[
                ("1", "a", "2026-09-02T00:00:00Z", 200_000),
                ("2", "a", "2026-09-02T00:01:00Z", 300_000),
                ("3", "a", "2026-09-02T00:02:00Z", 700_000),
            ],
            "one",
        ),
        false,
        true,
    );
    let r=execute(request(serde_json::json!({"action":"trajectory","threadId":"a","scope":{"allTime":true,"model":"other-model"}})),&s).unwrap();
    let i = r.inspection.unwrap();
    assert_eq!(i.trajectory.len(), 2);
    assert_eq!(i.trajectory[0].boundary, Some(InputBoundary::ModelChange));
    assert_eq!(i.trajectory[0].uncached_delta, None);
    assert_eq!(i.trajectory[1].uncached_delta, Some(400_000));
}

#[test]
fn compaction_comparison_is_observed_and_never_continuous_or_cross_scope() {
    for (first, second) in [(300, 200), (200, 300), (200, 200)] {
        let s = positioned_with(
            snapshot(
                &[
                    ("1-before", "a", "2026-09-02T00:00:00Z", first),
                    ("2-after", "a", "2026-09-03T00:00:00Z", second),
                ],
                "comparison",
            ),
            true,
            false,
            |m, _| {
                m.turn_id = Some("synthetic-turn".into());
            },
        );
        let result = execute(
            request(
                serde_json::json!({"action":"trajectory","threadId":"a","scope":{"allTime":true}}),
            ),
            &s,
        )
        .unwrap();
        let points = result.inspection.unwrap().trajectory;
        let comparison = points[1].compaction_comparison.as_ref().unwrap();
        assert_eq!(comparison.before_input, first);
        assert_eq!(comparison.after_input, second);
        assert_eq!(comparison.input_difference, second as i64 - first as i64);
        assert_eq!(comparison.before_measurement_id, "1-before");
        assert_eq!(
            comparison.before_evidence.snapshot_id,
            points[1].evidence.snapshot_id
        );
        assert_eq!(points[1].input_delta, None);
        assert_eq!(points[1].uncached_delta, None);
        let result=execute(request(serde_json::json!({"action":"trajectory","threadId":"a","scope":{"since":"2026-09-03","until":"2026-09-04","timezone":"UTC"}})),&s).unwrap();
        assert!(
            result.inspection.unwrap().trajectory[0]
                .compaction_comparison
                .is_none()
        );
    }
    let s = positioned(
        snapshot(
            &[
                ("1-before", "a", "2026-09-02T00:00:00Z", 300),
                ("2-after", "a", "2026-09-03T00:00:00Z", 200),
            ],
            "model",
        ),
        true,
        true,
    );
    assert!(
        execute(
            request(
                serde_json::json!({"action":"trajectory","threadId":"a","scope":{"allTime":true}})
            ),
            &s
        )
        .unwrap()
        .inspection
        .unwrap()
        .trajectory[1]
            .compaction_comparison
            .is_none()
    );
}

#[test]
fn concentration_reconciles_full_scope_and_distinguishes_zero_and_missing() {
    let ids: Vec<_> = (1..=12).map(|n| format!("t{n}")).collect();
    let rows: Vec<_> = ids
        .iter()
        .enumerate()
        .map(|(i, id)| {
            (
                id.as_str(),
                id.as_str(),
                "2026-09-02T00:00:00Z",
                (i + 1) as u64,
            )
        })
        .collect();
    let s = snapshot(&rows, "concentration");
    let r=execute(request(serde_json::json!({"action":"review","scope":{"since":"2026-09-02","until":"2026-09-03","timezone":"UTC"}})),&s).unwrap();
    let c = r.inspection.unwrap().review.unwrap().concentration.unwrap();
    assert_eq!(c.measured_tasks, 12);
    assert_eq!(c.total_tokens, Some(78));
    assert_eq!(c.top_task_tokens, Some(12));
    assert_eq!(c.top_five_tokens, Some(50));
    assert_eq!(c.top_ten_tokens, Some(75));
    assert_eq!(c.remaining_task_usage.complete_token_total(), Some(3));
    assert_eq!(c.top_task_share, Some(12.0 / 78.0));
    assert_eq!(c.top_ten_share, Some(75.0 / 78.0));
    for missing in [false, true] {
        let s = positioned_with(
            snapshot(&[("zero", "a", "2026-09-02T00:00:00Z", 0)], "zero"),
            false,
            false,
            if missing {
                |m, _| {
                    m.tokens = TokenUsage::default();
                }
            } else {
                |_, _| {}
            },
        );
        let c=execute(request(serde_json::json!({"action":"review","scope":{"since":"2026-09-02","until":"2026-09-03","timezone":"UTC"}})),&s).unwrap().inspection.unwrap().review.unwrap().concentration.unwrap();
        assert_eq!(c.total_tokens, if missing { None } else { Some(0) });
        assert_eq!(c.top_task_share, None);
    }
}

#[test]
fn context_inventory_counts_only_native_injected_and_window_records() {
    use crate::session_events::{ContentPhase, ContentPresence, MessageRecordKind, Phase};
    let mut s = positioned(
        snapshot(&[("usage", "a", "2026-09-02T00:00:00Z", 200)], "context"),
        false,
        false,
    );
    let mut collected = Collected {
        sources: s.manifest.sources.clone(),
        threads: s
            .manifest
            .threads
            .iter()
            .map(|t| t.thread.clone())
            .collect(),
        measurements: s
            .ledger()
            .unwrap()
            .iter()
            .map(|r| Arc::clone(&r.fact))
            .collect(),
        ..Default::default()
    };
    for (n, origin) in [
        MessageOrigin::InjectedContext,
        MessageOrigin::UserInput,
        MessageOrigin::Inherited,
        MessageOrigin::Reasoning,
    ]
    .into_iter()
    .enumerate()
    {
        collected.events.push(Arc::new(
            Event::new(
                Position {
                    source_instance_id: "synthetic".into(),
                    file_id: "context".into(),
                    generation: "one".into(),
                    byte_offset: n as u64,
                    ordinal: 0,
                },
                Some("a".into()),
                None,
                Time::from_source(Some("2026-09-02T00:00:00Z")).0,
                vec![],
                Payload::Message {
                    origin,
                    presence: ContentPresence::NonEmpty,
                    native_id: None,
                    record_kind: MessageRecordKind::ResponseSnapshot,
                    record_phase: Phase::Completed,
                    content_phase: ContentPhase::Unknown,
                },
            )
            .unwrap(),
        ));
    }
    collected.events.push(Arc::new(
        Event::new(
            Position {
                source_instance_id: "synthetic".into(),
                file_id: "context".into(),
                generation: "one".into(),
                byte_offset: 5,
                ordinal: 0,
            },
            Some("a".into()),
            None,
            Time::from_source(Some("2026-09-02T00:00:00Z")).0,
            vec![],
            Payload::ContextWindow {
                model: Some("gpt-5.4".into()),
                tokens: 128_000,
            },
        )
        .unwrap(),
    ));
    let root = tempfile::tempdir().unwrap();
    s = crate::usage_store::memory(
        collected,
        "live:synthetic:context".into(),
        crate::pricing_sync::current_at(root.path()).unwrap(),
        None,
    )
    .unwrap();
    let r = execute(
        request(serde_json::json!({"action":"context","scope":{"allTime":true},"limit":1})),
        &s,
    )
    .unwrap();
    let i = r.inspection.unwrap();
    let inventory = i.context.unwrap();
    assert_eq!(inventory.observed_records, 2);
    assert_eq!(inventory.injected_records, 1);
    assert_eq!(inventory.model_window_records, 1);
    assert_eq!(inventory.records.len(), 1);
    assert_eq!(r.page.total, 2);
    for record in inventory.records {
        assert_eq!(record.bytes, None);
        assert_eq!(record.content_version, None);
        assert_eq!(
            record.evidence.snapshot_id,
            s.manifest.snapshot_ref.snapshot_id
        );
        assert_eq!(record.evidence.scope.all_time, Some(true));
    }
    assert!(
        i.limitations
            .contains(&InspectionLimit::ContextMetadataUnavailable)
    );
    let r=execute(request(serde_json::json!({"action":"context","scope":{"since":"2026-09-03","until":"2026-09-04","timezone":"UTC"}})),&s).unwrap();
    assert_eq!(r.inspection.unwrap().context.unwrap().observed_records, 0);
}

#[test]
fn concentration_keeps_unassigned_measurements_in_the_denominator_and_remainder() {
    let s = positioned_with(
        snapshot(
            &[
                ("1", "a", "2026-09-02T00:00:00Z", 100),
                ("2", "a", "2026-09-02T00:00:00Z", 50),
            ],
            "unassigned",
        ),
        false,
        false,
        |m, i| {
            if i == 1 {
                m.thread_id = None;
            }
        },
    );
    let r=execute(request(serde_json::json!({"action":"review","scope":{"since":"2026-09-02","until":"2026-09-03","timezone":"UTC"}})),&s).unwrap();
    let c = r.inspection.unwrap().review.unwrap().concentration.unwrap();
    assert_eq!(c.total_tokens, Some(150));
    assert_eq!(c.top_task_tokens, Some(100));
    assert_eq!(c.top_task_share, Some(100.0 / 150.0));
    assert_eq!(c.remaining_task_usage.complete_token_total(), Some(50));
    assert_eq!(c.measured_tasks, 1);
}

#[test]
fn selected_turn_avoids_unrelated_event_partitions_and_preserves_exact_measurements() {
    let original = positioned_with(
        snapshot(
            &[
                ("small", "a", "2026-09-02T00:00:00Z", 1_000_000),
                ("large", "a", "2026-09-02T00:00:01Z", 200),
            ],
            "selected-turn",
        ),
        false,
        false,
        |m, _| m.turn_id = Some(if m.id == "small" { "small" } else { "large" }.into()),
    );
    let mut collected = Collected {
        sources: original.manifest.sources.clone(),
        threads: original
            .manifest
            .threads
            .iter()
            .map(|t| t.thread.clone())
            .collect(),
        measurements: original
            .ledger()
            .unwrap()
            .into_iter()
            .map(|r| r.fact)
            .collect(),
        events: original.events().unwrap(),
        ..Default::default()
    };
    for n in 0..100_001 {
        collected.events.push(Arc::new(
            Event::new(
                Position {
                    source_instance_id: "synthetic".into(),
                    file_id: "unrelated".into(),
                    generation: "one".into(),
                    byte_offset: n,
                    ordinal: 0,
                },
                Some("a".into()),
                Some("large".into()),
                Time::from_source(Some("2026-09-02T00:00:01Z")).0,
                vec![],
                Payload::ContextWindow {
                    model: Some("gpt-5.4".into()),
                    tokens: 128_000,
                },
            )
            .unwrap(),
        ));
    }
    let root = tempfile::tempdir().unwrap();
    let s = crate::usage_store::memory(
        collected,
        "live:synthetic:turn-budget".into(),
        crate::pricing_sync::current_at(root.path()).unwrap(),
        None,
    )
    .unwrap();
    let full = request(
        serde_json::json!({"action":"investigate","scope":{"threadId":"a","allTime":true},"compact":true}),
    );
    assert!(
        execute_snapshot(full, &s)
            .unwrap_err()
            .downcast_ref::<crate::dto::OperationError>()
            .is_some_and(|e| e.code == "RESOURCE_LIMIT")
    );
    let r=execute_snapshot(request(serde_json::json!({"action":"investigate","scope":{"threadId":"a","turnId":"small","allTime":true},"compact":true})),&s).unwrap();
    assert_eq!(r.summary.complete_token_total(), Some(1_000_000));
    let i = r.inspection.unwrap();
    assert!(i.limitations.contains(&InspectionLimit::SelectedTurnOnly));
    assert_eq!(
        i.candidates[0].evidence[0].turn_id.as_deref(),
        Some("small")
    );
    assert_eq!(
        i.candidates[0].evidence[0].snapshot_id,
        s.manifest.snapshot_ref.snapshot_id
    );
    assert!(execute_snapshot(request(serde_json::json!({"action":"investigate","scope":{"threadId":"a","turnId":"missing"}})),&s).unwrap_err().downcast_ref::<crate::dto::OperationError>().is_some_and(|e| e.code == "NOT_FOUND"));
    for value in [
        serde_json::json!({"action":"investigate","scope":{"turnId":"small"}}),
        serde_json::json!({"action":"usage","scope":{"threadId":"a","turnId":"small"}}),
    ] {
        assert!(execute_snapshot(request(value), &s).is_err());
    }
}

#[test]
fn inspection_waits_only_for_selected_recorded_project_and_keeps_global_usage_preview() {
    use crate::live::{ProjectLoad, ProjectLoadState};
    let mut s = snapshot(
        &[
            ("a", "a", "2026-09-02T00:00:00Z", 100),
            ("b", "b", "2026-09-02T00:00:00Z", 200),
        ],
        "scoped-ready",
    );
    s.project_loads = vec![
        ProjectLoad {
            project: Some("/a".into()),
            state: ProjectLoadState::Ready,
        },
        ProjectLoad {
            project: Some("/b".into()),
            state: ProjectLoadState::Loading,
        },
    ];
    for scope in [
        serde_json::json!({"project":"/a","allTime":true}),
        serde_json::json!({"threadId":"a","allTime":true}),
    ] {
        assert_eq!(
            execute_snapshot(
                request(serde_json::json!({"action":"investigate","scope":scope})),
                &s
            )
            .unwrap()
            .summary
            .complete_token_total(),
            Some(100)
        );
    }
    for scope in [
        serde_json::json!({"project":"/b"}),
        serde_json::json!({"allTime":true}),
    ] {
        assert!(
            execute_snapshot(
                request(serde_json::json!({"action":"investigate","scope":scope})),
                &s
            )
            .unwrap_err()
            .downcast_ref::<crate::dto::OperationError>()
            .is_some_and(|e| e.code == "SYNC_PENDING")
        );
    }
    assert!(
        execute_snapshot(
            request(serde_json::json!({"action":"usage","scope":{"allTime":true}})),
            &s
        )
        .is_ok()
    );
}
