use super::*;

#[test]
fn thresholds_require_disjoint_samples_and_preserve_zero_unknown() {
    let p = ActivityPolicy::default();
    let mut c = ActivityStats {
        operations: 5,
        tasks: 2,
        determinate_operations: 5,
        failed_operations: 3,
        slow_operations: 2,
        maximum_duration_ms: Some(30_000),
        maximum_failures_in_task: 3,
        maximum_rejections_in_task: 2,
        ..Default::default()
    };
    let b = ActivityStats {
        determinate_operations: 20,
        failed_operations: 0,
        duration_samples: 8,
        median_duration_ms: Some(3_000.0),
        ..Default::default()
    };
    assert_eq!(
        signals(&c, &b, &p),
        vec![
            ActivitySignal::RepeatedSlowRequest,
            ActivitySignal::FailureSpike,
            ActivitySignal::DurationSpike,
            ActivitySignal::RecurringWorkflow,
            ActivitySignal::RepeatedFailure,
            ActivitySignal::RepeatedRejection
        ]
    );
    for (signal, mutate) in [
        (ActivitySignal::RepeatedSlowRequest, 0),
        (ActivitySignal::FailureSpike, 1),
        (ActivitySignal::DurationSpike, 2),
        (ActivitySignal::RecurringWorkflow, 3),
        (ActivitySignal::RepeatedFailure, 4),
        (ActivitySignal::RepeatedRejection, 5),
    ] {
        let mut below = c.clone();
        match mutate {
            0 => below.slow_operations = 1,
            1 => below.failed_operations = 2,
            2 => below.maximum_duration_ms = Some(29_999),
            3 => below.tasks = 1,
            4 => below.maximum_failures_in_task = 2,
            _ => below.maximum_rejections_in_task = 1,
        }
        assert!(!signals(&below, &b, &p).contains(&signal));
    }
    let mut insufficient = b.clone();
    insufficient.determinate_operations = 19;
    insufficient.duration_samples = 7;
    let hit = signals(&c, &insufficient, &p);
    assert!(!hit.contains(&ActivitySignal::FailureSpike));
    assert!(!hit.contains(&ActivitySignal::DurationSpike));
    insufficient = b.clone();
    insufficient.failed_operations = 5; // 60% is below three times 25%.
    insufficient.median_duration_ms = Some(3_001.0);
    let hit = signals(&c, &insufficient, &p);
    assert!(!hit.contains(&ActivitySignal::FailureSpike));
    assert!(!hit.contains(&ActivitySignal::DurationSpike));
    c.determinate_operations = 0;
    c.maximum_duration_ms = None;
    assert!(!signals(&c, &ActivityStats::default(), &p).contains(&ActivitySignal::FailureSpike));
    assert!(!signals(&c, &b, &p).contains(&ActivitySignal::DurationSpike));
}

fn fixture() -> Snapshot {
    let base = super::super::tests::snapshot(
        &[
            ("m1", "a", "2026-09-02T00:00:00Z", 1),
            ("m2", "a", "2026-09-09T00:00:00Z", 1),
            ("m3", "b", "2026-09-09T00:00:00Z", 1),
        ],
        "activity",
    );
    let mut collected = Collected {
        sources: base.manifest.sources.clone(),
        threads: base
            .manifest
            .threads
            .iter()
            .map(|t| {
                let mut thread = t.thread.clone();
                thread.project = Some("/shared".into());
                thread
            })
            .collect(),
        measurements: base.ledger().unwrap().into_iter().map(|r| r.fact).collect(),
        ..Default::default()
    };
    for i in 0..27 {
        let thread = if i >= 25 { "b" } else { "a" };
        collected.operations.push(Arc::new(Operation {
            text_result: None,
            id: format!("op{i:02}"),
            thread_id: thread.into(),
            turn_id: Some("turn".into()),
            item_id: None,
            call_id: None,
            response_id: None,
            kind: "command".into(),
            name: "exec_command".into(),
            sequence: i,
            timestamp: Some(
                if i < 20 {
                    "2026-09-02T00:00:00Z"
                } else {
                    "2026-09-09T00:00:00Z"
                }
                .into(),
            ),
            time_precision: "second".into(),
            status: if !(20..25).contains(&i) {
                "completed"
            } else if i < 23 {
                "failed"
            } else {
                "declined"
            }
            .into(),
            exit_code: None,
            outcome_conflict: false,
            duration_ms: Some(if i < 20 { 3_000 } else { 30_000 }),
            path: None,
            work: None,
            matching: Some(OperationMatchObservation {
                format_version: OPERATION_MATCH_VERSION,
                receiver_owner: Some(thread.into()),
                request_fingerprint: Some("a".repeat(64)),
                function_request_fingerprint: None,
                read_targets: vec![],
                expected_nonzero: false,
                gaps: vec![],
            }),
            server: None,
            tool: None,
            evidence: vec![],
        }));
    }
    let root = tempfile::tempdir().unwrap();
    crate::usage_store::memory(
        collected,
        "live:synthetic:activity".into(),
        crate::pricing_sync::current_at(root.path()).unwrap(),
        None,
    )
    .unwrap()
}
#[test]
fn assembled_inspection_merges_six_signals_with_original_and_baseline_scopes() {
    let s = fixture();
    let request = serde_json::from_value(serde_json::json!({"action":"investigate", "scope":{
        "since":"2026-09-08", "until":"2026-09-15", "timezone":"UTC", "project":"/shared"
    }}))
    .unwrap();
    let response = super::super::execute(request, &s).unwrap();
    assert_eq!(response.inspection.as_ref().unwrap().method_version, 3);
    let activity = response.inspection.unwrap().activity.unwrap();
    assert_eq!(activity.finding_count, 1);
    let f = &activity.findings[0];
    assert_eq!(f.signals.len(), 6);
    assert_eq!(f.current.operations, 7);
    assert_eq!(f.current.tasks, 2);
    assert_eq!(f.current.determinate_operations, 5);
    assert_eq!(f.current.failed_operations, 3);
    assert_eq!(f.current.rejected_operations, 2);
    assert_eq!(f.baseline.as_ref().unwrap().operations, 20);
    assert_eq!(
        f.baseline.as_ref().unwrap().median_duration_ms,
        Some(3_000.0)
    );
    assert_eq!(activity.current_coverage.matched_operations, 7);
    assert!(
        f.evidence
            .iter()
            .all(|e| e.scope.since.as_deref() == Some("2026-09-08")
                && e.snapshot_id == s.manifest.snapshot_ref.snapshot_id)
    );
    assert!(
        f.baseline_evidence
            .iter()
            .all(|e| e.scope.since.as_deref() == Some("2026-09-01")
                && e.scope.until.as_deref() == Some("2026-09-08")
                && e.scope.project.as_deref() == Some("/shared"))
    );
}
#[test]
fn unbounded_scope_runs_nonhistorical_checks_without_fabricating_baseline() {
    let s = fixture();
    let request = serde_json::from_value(
        serde_json::json!({"action":"investigate", "scope":{"allTime":true}}),
    )
    .unwrap();
    let activity = super::super::execute(request, &s)
        .unwrap()
        .inspection
        .unwrap()
        .activity
        .unwrap();
    assert!(activity.baseline_scope.is_none());
    assert!(activity.baseline_coverage.is_none());
    let f = &activity.findings[0];
    assert!(f.baseline.is_none());
    assert!(f.baseline_evidence.is_empty());
    assert!(!f.signals.contains(&ActivitySignal::FailureSpike));
    assert!(!f.signals.contains(&ActivitySignal::DurationSpike));
}
#[test]
fn unreliable_identity_conflicts_and_expected_exit_codes_do_not_create_failures() {
    let s = fixture();
    let thread = &s.manifest.threads[0];
    let mut op = s
        .turn("a", "turn")
        .unwrap()
        .operations
        .into_iter()
        .find(|o| o.status.as_ref() == "failed")
        .unwrap();
    op.exit_code = Some(1);
    op.matching.as_mut().unwrap().expected_nonzero = true;
    assert_eq!(outcomes(&op).failed, 0);
    op.exit_code = Some(2);
    assert_eq!(outcomes(&op).failed, 1);
    op.outcome_conflict = true;
    assert_eq!(outcomes(&op).failed, 0);
    assert_eq!(terminal_duration(&op, &outcomes(&op)), None);
    op.matching.as_mut().unwrap().receiver_owner = Some("foreign".into());
    assert!(key(thread, &op).is_none());
    op.matching.as_mut().unwrap().receiver_owner = Some("a".into());
    let mut foreign_project = thread.as_ref().clone();
    foreign_project.thread.project = Some("/other".into());
    assert_ne!(key(thread, &op), key(&foreign_project, &op));
    foreign_project.thread.project = None;
    let unknown_a = key(&foreign_project, &op);
    foreign_project.thread.id = "b".into();
    op.matching.as_mut().unwrap().receiver_owner = Some("b".into());
    assert_ne!(unknown_a, key(&foreign_project, &op));
}

#[test]
fn date_windows_preserve_authorization_and_ignore_dst_length_changes() {
    let scope: Scope = serde_json::from_value(serde_json::json!({
        "since":"2026-03-08", "until":"2026-03-10", "timezone":"America/New_York",
        "project":"/synthetic", "sourceInstanceId":"synthetic", "model":"gpt-5.4"
    }))
    .unwrap();
    let baseline = baseline_scope(&scope).unwrap().unwrap();
    assert_eq!(baseline.since.as_deref(), Some("2026-03-06"));
    assert_eq!(baseline.until.as_deref(), Some("2026-03-08"));
    assert_eq!(baseline.project, scope.project);
    assert_eq!(baseline.source_instance_id, scope.source_instance_id);
    assert_eq!(baseline.model, scope.model);
    let tz: Tz = "America/New_York".parse().unwrap();
    let s = fixture();
    let mut op = s.turn("a", "turn").unwrap().operations.remove(0);
    op.timestamp = Some("2026-03-08T04:59:59Z".into());
    let mut unfiltered = scope.clone();
    unfiltered.model = None;
    let mut previous = baseline;
    previous.model = None;
    assert!(!operation_matches(&op, &unfiltered, tz, &[]));
    assert!(operation_matches(&op, &previous, tz, &[]));
    op.timestamp = Some("2026-03-08T05:00:00Z".into());
    assert!(operation_matches(&op, &unfiltered, tz, &[]));
    assert!(!operation_matches(&op, &previous, tz, &[]));
    op.timestamp = None;
    assert!(!operation_matches(&op, &unfiltered, tz, &[]));
    assert!(!operation_matches(&op, &previous, tz, &[]));
}

#[test]
fn findings_are_bounded_with_complete_counts_and_partial_identity_coverage() {
    let s = fixture();
    let thread = &s.manifest.threads[0];
    let mut op = s.turn("a", "turn").unwrap().operations.remove(0);
    op.duration_ms = Some(30_000);
    let scope: Scope = serde_json::from_value(serde_json::json!({"allTime":true})).unwrap();
    let mut builder = Builder::new(&scope).unwrap();
    for group in 0..31 {
        op.matching.as_mut().unwrap().request_fingerprint = Some(crate::hash(group.to_string()));
        for n in 0..2 {
            op.id = format!("group{group}-op{n}");
            builder.add(&s, &scope, thread, &op, false);
        }
    }
    op.matching = None;
    builder.add(&s, &scope, thread, &op, false);
    let review = builder.finish();
    assert_eq!(review.finding_count, 31);
    assert_eq!(review.findings.len(), 30);
    assert_eq!(review.current_coverage.observed_operations, 63);
    assert_eq!(review.current_coverage.matched_operations, 62);
    assert!(
        review
            .findings
            .iter()
            .all(|f| f.current.operations == 2 && !f.evidence.is_empty())
    );
}
