use super::*;
fn metric(value: Option<u64>, basis: t::Basis) -> t::Count {
    t::Metric {
        value,
        status: if value.is_some() {
            t::MetricStatus::Derived
        } else {
            t::MetricStatus::Unavailable
        },
        basis,
        evidence_refs: vec!["collection:turn".into()],
    }
}
#[test]
fn positive_partial_signals_remain_hits_while_gaps_never_prove_a_miss() {
    let rule = ActivityRule::InspectCallsAfterFailure;
    assert_eq!(
        check(rule, &metric(Some(2), t::Basis::RepeatAfterFailure), true).outcome,
        RuleOutcome::Hit
    );
    assert_eq!(
        check(rule, &metric(Some(0), t::Basis::RepeatAfterFailure), true).outcome,
        RuleOutcome::Insufficient
    );
    assert_eq!(
        check(rule, &metric(Some(0), t::Basis::RepeatAfterFailure), false).outcome,
        RuleOutcome::Miss
    );
    assert_eq!(
        check(rule, &metric(None, t::Basis::ResourceLimit), true).outcome,
        RuleOutcome::Insufficient
    );
    assert_eq!(
        check(
            rule,
            &metric(Some(1), t::Basis::SameRequestObservation),
            false
        )
        .outcome,
        RuleOutcome::Unsupported
    );
}
#[test]
fn count_checks_ignore_optional_duration_gaps_and_request_counts_ignore_time_and_outcomes() {
    use t::RepeatCoverageReason as Gap;
    let optional = [
        Gap::MissingDurations,
        Gap::MissingRecoverySpans,
        Gap::MissingIntervals,
        Gap::MissingWindow,
        Gap::DurationConflicts,
        Gap::ContextBoundaries,
    ];
    for rule in [
        ActivityRule::InspectCallsAfterFailure,
        ActivityRule::InspectRepeatedReads,
        ActivityRule::InspectRepeatedRequests,
    ] {
        assert!(!count_gap(&optional, "complete", rule));
        assert!(count_gap(&[Gap::MissingMatching], "complete", rule));
        assert!(count_gap(&[], "partial", rule));
    }
    assert!(!count_gap(
        &[Gap::MissingStart, Gap::IndeterminateOutcomes],
        "complete",
        ActivityRule::InspectRepeatedRequests
    ));
    assert!(count_gap(
        &[Gap::MissingStart],
        "complete",
        ActivityRule::InspectCallsAfterFailure
    ));
}
#[test]
fn activity_selection_requires_fixed_scope_and_cannot_enter_configuration_or_handling() {
    let valid = Request {
        action: Action::Activity,
        activity: Some(ActivitySelection {
            snapshot_id: "live:fixed".into(),
            thread_id: "task".into(),
            turn_id: "turn".into(),
        }),
        ..Default::default()
    };
    validate(&valid).unwrap();
    let t::Request::Summary {
        snapshot_id, mode, ..
    } = timing_request(&valid).unwrap()
    else {
        panic!()
    };
    assert_eq!(snapshot_id.as_deref(), Some("live:fixed"));
    assert_eq!(mode, t::Mode::Cached);
    for request in [
        Request {
            read_view: Some("config:other".into()),
            ..valid.clone()
        },
        Request {
            project_roots: Some(vec![]),
            ..valid.clone()
        },
        Request {
            rule_overrides: Some(Default::default()),
            ..valid.clone()
        },
        Request {
            baseline_id: Some("not-an-activity-input".into()),
            ..valid.clone()
        },
        Request {
            action: Action::Keep,
            ..valid.clone()
        },
        Request {
            activity: None,
            ..valid.clone()
        },
    ] {
        assert!(validate(&request).is_err());
    }
    let mut empty = valid;
    empty.activity.as_mut().unwrap().snapshot_id.clear();
    assert!(validate(&empty).is_ok());
}

fn outcomes(success: u64, failed: u64, partial: bool) -> t::OutcomeStatistics {
    let mut value =
        serde_json::json!({"method":"terminal_success_failure_subset_v1","partial":partial});
    for (key, count) in [
        ("determinateOperations", success + failed),
        ("succeeded", success),
        ("failed", failed),
        ("interrupted", 0),
        ("rejected", 0),
        ("nonterminal", 0),
        ("indeterminate", 0),
        ("conflicting", 0),
        ("identityGapRecords", 0),
        ("unclassified", 0),
    ] {
        value[key] =
            serde_json::to_value(metric(Some(count), t::Basis::DeterminateTerminalOutcomes))
                .unwrap();
    }
    value["failureRatio"] = serde_json::json!({"value":if success+failed>0 {Some(failed as f64/(success+failed) as f64)} else {None},"status":if success+failed>0 {"derived"} else {"unavailable"},"basis":if success+failed>0 {"determinate_terminal_outcomes"} else {"no_candidates"},"evidenceRefs":["collection:turn"]});
    serde_json::from_value(value).unwrap()
}
#[test]
fn failure_share_inspection_uses_explicit_sample_and_ratio_policy_including_partial_hits() {
    let hit = failure_share(&outcomes(3, 2, true));
    assert_eq!(hit.outcome, RuleOutcome::Hit);
    assert!(hit.partial);
    assert_eq!(hit.observed.value, Some(2));
    assert_eq!(hit.failure_policy.unwrap().minimum_determinate, 5);
    assert_eq!(hit.outcomes.unwrap().failure_ratio.value, Some(0.4));
    assert_eq!(
        failure_share(&outcomes(7, 3, false)).outcome,
        RuleOutcome::Miss
    );
    assert_eq!(
        failure_share(&outcomes(7, 3, true)).outcome,
        RuleOutcome::Miss
    );
    for value in [
        outcomes(0, 0, false),
        outcomes(0, 1, false),
        outcomes(0, 4, true),
    ] {
        let small = failure_share(&value);
        assert_eq!(small.outcome, RuleOutcome::Insufficient);
        assert!(matches!(
            small.reason,
            Some(ActivityReason::ActivitySampleTooSmall)
        ));
    }
}
#[test]
fn failure_share_missing_or_unsupported_measures_do_not_create_advice_or_faults() {
    let mut value = outcomes(3, 2, true);
    value.determinate_operations = metric(None, t::Basis::ResourceLimit);
    let missing = failure_share(&value);
    assert_eq!(missing.outcome, RuleOutcome::Insufficient);
    assert!(matches!(
        missing.reason,
        Some(ActivityReason::ActivityMeasureUnavailable)
    ));
    let mut value = outcomes(3, 2, false);
    value.failed.basis = t::Basis::NativeRecord;
    assert_eq!(failure_share(&value).outcome, RuleOutcome::Unsupported);
}

#[test]
fn input_change_advice_uses_scoped_delta_and_keeps_partial_positive_facts() {
    let mut change = t::InputChange {
        method: t::InputChangeMethod::RequestInputObservationChangeV1,
        availability: t::Capability {
            support: t::Support::Supported,
            reason: t::Basis::RequestInput,
        },
        statistics: Some(t::InputChangeStatistics {
            candidates: 2,
            ordered_samples: 2,
            non_request_scoped: 0,
            missing_input: 0,
            unassociated: 0,
            numeric_range: 0,
            comparable_stages: 1,
            increasing_stages: 1,
            decreasing_stages: 0,
            unchanged_stages: 0,
            maximum_increase: metric(Some(16384), t::Basis::RequestInput),
            largest_increase: None,
            stages: vec![],
            details_omitted: true,
            partial: false,
        }),
    };
    let positive = input_change(&change, "partial");
    assert_eq!(positive.outcome, RuleOutcome::Hit);
    assert!(positive.partial);
    assert_eq!(positive.input_policy.unwrap().minimum_increase, 16384);
    change.statistics.as_mut().unwrap().maximum_increase.value = Some(16383);
    assert_eq!(input_change(&change, "complete").outcome, RuleOutcome::Miss);
    change.statistics.as_mut().unwrap().maximum_increase.status = t::MetricStatus::Observed;
    assert_eq!(
        input_change(&change, "complete").outcome,
        RuleOutcome::Unsupported
    );
    change.statistics.as_mut().unwrap().comparable_stages = 0;
    assert_eq!(
        input_change(&change, "complete").outcome,
        RuleOutcome::Insufficient
    );
    change.statistics = None;
    change.availability = t::Capability {
        support: t::Support::Unavailable,
        reason: t::Basis::ResourceLimit,
    };
    let result = input_change(&change, "complete");
    assert_eq!(result.outcome, RuleOutcome::Insufficient);
    assert_eq!(result.observed.basis, t::Basis::ResourceLimit);
    assert!(result.observed.value.is_none());
}
