use super::*;
use std::sync::Arc;

fn snapshot(records: &[(&str, &str, &str, u64)], suffix: &str) -> Snapshot {
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
            reasoning_effort: None,
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
fn period(limit: usize, offset: usize) -> Request {
    request(
        serde_json::json!({"action":"compare","scope":{"since":"2026-09-08","until":"2026-09-15","timezone":"UTC"},"comparison":{"kind":"periods","baselineSince":"2026-09-01","baselineUntil":"2026-09-08","dimension":"project"},"limit":limit,"offset":offset}),
    )
}
#[test]
fn signed_drivers_reconcile_full_totals_across_pages_and_zero_baselines() {
    let s = snapshot(
        &[
            ("1", "a", "2026-09-02T00:00:00Z", 100),
            ("2", "b", "2026-09-02T00:00:00Z", 70),
            ("3", "a", "2026-09-09T00:00:00Z", 140),
            ("4", "b", "2026-09-09T00:00:00Z", 20),
            ("5", "c", "2026-09-09T00:00:00Z", 30),
        ],
        "one",
    );
    // Independent truth: 170 -> 190; a +40, b -50, c +30.
    for offset in [0, 1, 2, 3, 100] {
        let r = execute(period(1, offset), &s).unwrap();
        let Some(Comparison::Periods {
            baseline,
            current,
            delta,
            drivers,
            remaining,
            ..
        }) = r.comparison
        else {
            panic!()
        };
        assert_eq!(baseline.usage.complete_token_total(), Some(170));
        assert_eq!(current.usage.complete_token_total(), Some(190));
        assert_eq!(delta.tokens, Some(20));
        assert_eq!(
            drivers.iter().map(|d| d.delta.tokens.unwrap()).sum::<i64>()
                + remaining.tokens.unwrap(),
            20
        );
        assert_eq!(
            drivers
                .iter()
                .filter_map(|d| d.delta.cost.as_ref())
                .map(|c| c.parse::<Decimal>().unwrap())
                .sum::<Decimal>()
                + remaining.cost.unwrap().parse::<Decimal>().unwrap(),
            delta.cost.unwrap().parse::<Decimal>().unwrap()
        );
        for d in drivers {
            assert_eq!(
                d.delta.tokens,
                Some(match d.key.as_deref().unwrap() {
                    "/a" => 40,
                    "/b" => -50,
                    "/c" => 30,
                    _ => panic!(),
                })
            );
            if d.key.as_deref() == Some("/c") {
                assert_eq!(d.delta.token_ratio, None);
            }
        }
    }
}
#[test]
fn invalid_comparisons_reject_direct_callers() {
    for (a, b, c, d) in [
        ("2026-09-01", "2026-09-01", "2026-09-08", "2026-09-15"),
        ("2026-09-01", "2026-09-09", "2026-09-08", "2026-09-16"),
        ("2026-09-01", "2026-09-08", "2026-09-08", "2026-09-16"),
        ("not-a-date", "2026-09-08", "2026-09-08", "2026-09-15"),
    ] {
        let mut r = period(20, 0);
        r.comparison = Some(ComparisonRequest::Periods {
            baseline_since: a.into(),
            baseline_until: b.into(),
            dimension: DriverDimension::Project,
        });
        r.scope.since = Some(c.into());
        r.scope.until = Some(d.into());
        assert!(crate::usage_app::validate(&r).is_err());
    }
    let mut r = period(20, 0);
    r.action = Action::Usage;
    assert!(super::validate_comparison(&r).is_err());
    let r = request(
        serde_json::json!({"action":"compare","comparison":{"kind":"sessions","leftThreadId":"a","rightThreadId":"a"}}),
    );
    assert!(super::validate_comparison(&r).is_err());
}
#[test]
fn session_and_publication_changes_preserve_identity_and_exact_cost() {
    let a = snapshot(
        &[
            ("1", "a", "2026-09-02T00:00:00Z", 100),
            ("2", "b", "2026-09-02T00:00:00Z", 70),
        ],
        "one",
    );
    let b = snapshot(
        &[
            ("1", "a", "2026-09-02T00:00:00Z", 110),
            ("3", "c", "2026-09-02T00:00:00Z", 30),
        ],
        "two",
    );
    let change = publication_change(&a, &b, None).unwrap();
    assert_eq!(
        (
            change.measurements_added,
            change.measurements_removed,
            change.measurements_changed,
            change.threads_added
        ),
        (1, 1, 1, 1)
    );
    assert_eq!(change.delta.tokens, Some(-30));
    assert!(!change.prices_changed);
    let r=execute(request(serde_json::json!({"action":"compare","comparison":{"kind":"sessions","leftThreadId":"a","rightThreadId":"b"}})),&a).unwrap();
    let Some(Comparison::Sessions {
        left, right, delta, ..
    }) = r.comparison
    else {
        panic!()
    };
    assert_eq!(left.own.complete_token_total(), Some(100));
    assert_eq!(right.selected.complete_token_total(), Some(70));
    assert_eq!(delta.tokens, Some(-30));
    assert_eq!(left.member_count, 1);
}

#[test]
fn missing_totals_and_unpriced_measurements_do_not_become_zero_deltas() {
    let mut s = snapshot(&[("1", "a", "2026-09-02T00:00:00Z", 100)], "one");
    // Immutable summary values from a known record, then independently erase all
    // inputs that could establish a total, retaining explicit missing reasons.
    let mut summary = summarize(&s.ledger().unwrap().iter().collect::<Vec<_>>()).unwrap();
    summary.tokens.total = None;
    summary
        .token_analysis
        .total_analysis
        .as_mut()
        .unwrap()
        .subtotal = None;
    summary
        .token_analysis
        .total_analysis
        .as_mut()
        .unwrap()
        .unavailable_records = 1;
    summary.price.status = "unknown".into();
    summary.price.cost = None;
    assert!(delta(&summary, &summarize(&[]).unwrap()).tokens.is_none());
    assert!(delta(&summary, &summarize(&[]).unwrap()).cost.is_none());
    s.manifest.snapshot_ref.created_at = "2026-09-10T12:00:00Z".into();
    let r = execute(period(20, 0), &s).unwrap();
    let Some(Comparison::Periods {
        baseline, current, ..
    }) = r.comparison
    else {
        panic!()
    };
    assert!(!baseline.partial);
    assert!(current.partial);
}
#[test]
fn explicit_family_counts_each_canonical_measurement_once() {
    let root = tempfile::tempdir().unwrap();
    let sources = root.path().join("source");
    std::fs::create_dir_all(sources.join("sessions")).unwrap();
    for (id, parent, n) in [
        ("parent", None, 100),
        ("child", Some("parent"), 30),
        ("grandchild", Some("child"), 20),
        ("other", None, 80),
    ] {
        let rows=serde_json::json!({"type":"session_meta","payload":{"id":id,"forked_from_id":parent}}).to_string()+"\n"+&serde_json::json!({"type":"turn_context","payload":{"turn_id":"u","model":"gpt-5.4"}}).to_string()+"\n"+&serde_json::json!({"type":"event_msg","timestamp":"2026-09-02T00:00:00Z","payload":{"type":"token_usage_record","thread_id":id,"turn_id":"u","response_id":format!("r-{id}"),"usage":{"input_tokens":n,"cached_input_tokens":0,"cache_write_input_tokens":0,"output_tokens":0,"reasoning_output_tokens":0,"total_tokens":n}}}).to_string()+"\n";
        std::fs::write(sources.join("sessions").join(format!("{id}.jsonl")), rows).unwrap();
    }
    let collected = crate::adapters::collect(
        &DiscoveryRequest {
            roots: vec![sources],
        },
        &RunContext::default(),
    );
    let s = crate::usage_store::memory(
        collected,
        "live:synthetic:family".into(),
        crate::pricing_sync::current_at(root.path()).unwrap(),
        None,
    )
    .unwrap();
    let id = |name: &str| {
        s.manifest
            .threads
            .iter()
            .find(|t| t.thread.upstream_id == name)
            .unwrap()
            .thread
            .id
            .clone()
    };
    let query = request(
        serde_json::json!({"action":"compare","comparison":{"kind":"sessions","leftThreadId":id("parent"),"rightThreadId":id("other"),"includeDescendants":true}}),
    );
    let r = execute(query, &s).unwrap();
    let Some(Comparison::Sessions { left, right, .. }) = r.comparison else {
        panic!()
    };
    assert_eq!(left.member_count, 3);
    assert_eq!(left.own.complete_token_total(), Some(100));
    assert_eq!(left.descendants.complete_token_total(), Some(50));
    assert_eq!(left.selected.complete_token_total(), Some(150));
    assert_eq!(right.selected.complete_token_total(), Some(80));
    assert!(!left.partial);
}
