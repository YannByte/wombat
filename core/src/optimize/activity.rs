//! Pure inspection checks consume shared analysis; no source reads or review-store writes.
use crate::{dto::operation_error, optimize_dto::*, timing_dto as t};
use anyhow::Result;
mod rules;

pub(crate) fn validate(request: &Request) -> Result<()> {
    if request.action != Action::Activity {
        return if request.activity.is_none() {
            Ok(())
        } else {
            Err(operation_error(
                "INVALID_ARGUMENT",
                "Activity scope requires the activity action",
            ))
        };
    }
    if request.activity.is_none()
        || request.baseline_id.is_some()
        || request.baseline.is_some()
        || request.read_view.is_some()
        || request.project_roots.is_some()
        || request.project.is_some()
        || request.decision_revision.is_some()
        || request.suggestion_id.is_some()
        || request.item_id.is_some()
        || request.decision_reason.is_some()
        || request.category.is_some()
        || request.offset.is_some()
        || request.limit.is_some()
        || request.rule_overrides.is_some()
    {
        return Err(operation_error(
            "INVALID_ARGUMENT",
            "Activity requires one fixed turn without configuration or handling inputs",
        ));
    }
    crate::timing::validate(&timing_request(request)?)
}
pub(crate) fn timing_request(request: &Request) -> Result<t::Request> {
    let selected = request
        .activity
        .as_ref()
        .ok_or_else(|| operation_error("INVALID_ARGUMENT", "Missing activity scope"))?;
    Ok(t::Request::Summary {
        thread_id: selected.thread_id.clone(),
        turn_id: selected.turn_id.clone(),
        snapshot_id: (!selected.snapshot_id.is_empty()).then(|| selected.snapshot_id.clone()),
        roots: request.roots.clone().unwrap_or_default(),
        scope: Some(t::Scope {
            source_instance_id: request.source_instance_id.clone(),
            agent_kind: Some("codex".into()),
        }),
        mode: t::Mode::Cached,
        privacy_profile: t::PrivacyProfile::Local,
    })
}
/// A positive observation is useful even with source gaps. A miss needs complete matching coverage.
fn check(rule: ActivityRule, observed: &t::Count, partial: bool) -> ActivityCheck {
    let definition = rule.definition();
    let supported_basis =
        observed.basis == definition.basis && matches!(observed.status, t::MetricStatus::Derived);
    let (outcome, reason) = if observed.value.is_some() && !supported_basis {
        (
            RuleOutcome::Unsupported,
            Some(ActivityReason::ActivityBasisUnsupported),
        )
    } else if observed.value.is_some_and(|n| n > 0) {
        (RuleOutcome::Hit, None)
    } else if observed.value == Some(0) && !partial {
        (RuleOutcome::Miss, None)
    } else {
        (
            RuleOutcome::Insufficient,
            Some(if observed.value.is_none() {
                ActivityReason::ActivityMeasureUnavailable
            } else {
                ActivityReason::ActivityCoverageIncomplete
            }),
        )
    };
    ActivityCheck {
        input_change: None,
        input_policy: None,
        outcomes: None,
        failure_policy: None,
        rule,
        version: definition.version,
        method: definition.method.into(),
        outcome,
        observed: observed.clone(),
        partial,
        reason,
    }
}
/// A deterministic inspection threshold over the captured determinate subset, not a fault detector.
fn failure_share(observed: &t::OutcomeStatistics) -> ActivityCheck {
    let policy = FailureSharePolicy::default();
    let rule = ActivityRule::InspectFailureShare;
    let definition = rule.definition();
    let (outcome, reason) = match observed.determinate_operations.value {
        None => (
            RuleOutcome::Insufficient,
            Some(ActivityReason::ActivityMeasureUnavailable),
        ),
        Some(_)
            if observed.determinate_operations.status != t::MetricStatus::Derived
                || observed.determinate_operations.basis != definition.basis
                || observed.failed.status != t::MetricStatus::Derived
                || observed.failed.basis != definition.basis =>
        {
            (
                RuleOutcome::Unsupported,
                Some(ActivityReason::ActivityBasisUnsupported),
            )
        }
        Some(n) if n < u64::from(policy.minimum_determinate) => (
            RuleOutcome::Insufficient,
            Some(ActivityReason::ActivitySampleTooSmall),
        ),
        Some(_) => match (observed.failed.value, observed.failure_ratio.value) {
            (Some(failed), Some(ratio))
                if observed.failure_ratio.status == t::MetricStatus::Derived
                    && observed.failure_ratio.basis == definition.basis =>
            {
                (
                    if failed >= u64::from(policy.minimum_failures) && ratio >= policy.minimum_ratio
                    {
                        RuleOutcome::Hit
                    } else {
                        RuleOutcome::Miss
                    },
                    None,
                )
            }
            _ => (
                RuleOutcome::Insufficient,
                Some(ActivityReason::ActivityMeasureUnavailable),
            ),
        },
    };
    ActivityCheck {
        input_change: None,
        input_policy: None,
        rule,
        version: definition.version,
        method: definition.method.into(),
        outcome,
        observed: observed.failed.clone(),
        partial: observed.partial,
        reason,
        outcomes: Some(observed.clone()),
        failure_policy: Some(policy),
    }
}
/// A source-order increase is an inspection hint; it does not establish monotonic growth or waste.
fn input_change(observed: &t::InputChange, source_status: &str) -> ActivityCheck {
    let policy = InputChangePolicy::default();
    let rule = ActivityRule::InspectInputChange;
    let definition = rule.definition();
    let stats = observed.statistics.as_ref();
    let measure = stats
        .map(|s| s.maximum_increase.clone())
        .unwrap_or_else(|| t::Metric {
            value: None,
            status: t::MetricStatus::Unavailable,
            basis: observed.availability.reason,
            evidence_refs: vec![],
        });
    let (outcome, reason) = if observed.availability.support != t::Support::Supported {
        (
            RuleOutcome::Insufficient,
            Some(ActivityReason::ActivityMeasureUnavailable),
        )
    } else if stats.is_none_or(|s| s.comparable_stages == 0) {
        (
            RuleOutcome::Insufficient,
            Some(ActivityReason::ActivitySampleTooSmall),
        )
    } else if measure.status != t::MetricStatus::Derived || measure.basis != definition.basis {
        (
            RuleOutcome::Unsupported,
            Some(ActivityReason::ActivityBasisUnsupported),
        )
    } else {
        match measure.value {
            Some(n) => (
                if n >= u64::from(policy.minimum_increase) {
                    RuleOutcome::Hit
                } else {
                    RuleOutcome::Miss
                },
                None,
            ),
            None => (
                RuleOutcome::Insufficient,
                Some(ActivityReason::ActivityMeasureUnavailable),
            ),
        }
    };
    ActivityCheck {
        rule,
        version: definition.version,
        method: definition.method.into(),
        outcome,
        reason,
        observed: measure,
        partial: source_status != "complete" || stats.is_none_or(|s| s.partial),
        input_change: Some(observed.clone()),
        input_policy: Some(policy),
        outcomes: None,
        failure_policy: None,
    }
}
fn count_gap(reasons: &[t::RepeatCoverageReason], source_status: &str, rule: ActivityRule) -> bool {
    use t::RepeatCoverageReason as Gap;
    source_status != "complete"
        || reasons.iter().any(|reason| match reason {
            Gap::MissingDurations
            | Gap::MissingRecoverySpans
            | Gap::MissingIntervals
            | Gap::MissingWindow
            | Gap::DurationConflicts
            | Gap::NumericRange
            | Gap::ContextBoundaries => false,
            Gap::MissingStart | Gap::IndeterminateOutcomes => {
                rule != ActivityRule::InspectRepeatedRequests
            }
            _ => true,
        })
}
pub(crate) fn evaluate(summary: &t::LocalResponse) -> ActivityResult {
    let repeats = &summary.time.repeated_behavior;
    let checks: Vec<_> = rules::RULES
        .into_iter()
        .map(|rule| {
            let observed = match rule {
                ActivityRule::InspectCallsAfterFailure => &repeats.after_failure.count,
                ActivityRule::InspectRepeatedReads => &repeats.repeated_read.count,
                ActivityRule::InspectRepeatedRequests => &repeats.same_request_observation_count,
                ActivityRule::InspectFailureShare => return failure_share(&summary.work.outcomes),
                ActivityRule::InspectInputChange => {
                    return input_change(
                        &summary.context.input_change,
                        &summary.coverage.source_status,
                    );
                }
            };
            check(
                rule,
                observed,
                count_gap(
                    &repeats.coverage.reason_codes,
                    &summary.coverage.source_status,
                    rule,
                ),
            )
        })
        .collect();
    // Keep check facts independent. The broader request hint adds no advice when a confirmed link is available.
    let confirmed = checks[..2].iter().any(|c| c.outcome == RuleOutcome::Hit);
    let advice = checks
        .iter()
        .filter(|c| {
            c.outcome == RuleOutcome::Hit
                && (!confirmed || c.rule != ActivityRule::InspectRepeatedRequests)
        })
        .map(|c| c.rule)
        .collect();
    ActivityResult {
        format_version: 3,
        read_view: summary.read_view.clone(),
        scope: summary.scope.clone(),
        analysis_method: summary.method_version.clone(),
        freshness: summary.freshness.clone(),
        source_status: summary.coverage.source_status.clone(),
        coverage: repeats.coverage.clone(),
        checks,
        advice,
    }
}
pub(crate) fn response(result: t::Response) -> Result<Response> {
    let t::Response::Local(summary) = result else {
        return Err(operation_error(
            "INVALID_FACTS",
            "Activity requires local turn analysis",
        ));
    };
    let activity = evaluate(&summary);
    let mut response = super::capabilities();
    response.action = Action::Activity;
    response.usage_revision = Some(summary.read_view.snapshot_id.clone());
    response.checked_at = summary
        .freshness
        .checked_at
        .clone()
        .unwrap_or_else(|| summary.read_view.created_at.clone());
    response.result_status = if activity.checks.iter().any(|check| {
        check.partial || !matches!(check.outcome, RuleOutcome::Hit | RuleOutcome::Miss)
    }) {
        "partial"
    } else {
        "complete"
    }
    .into();
    response.activity = Some(activity);
    if serde_json::to_vec(&response)?.len() > 256 * 1024 {
        return Err(operation_error(
            "RESOURCE_LIMIT",
            "Activity inspection output exceeds its budget",
        ));
    }
    Ok(response)
}
#[cfg(test)]
mod tests;
