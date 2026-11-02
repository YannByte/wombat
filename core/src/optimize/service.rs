//! Pure checks and explicit decisions. A retained read view supplies the recheck baseline.
use super::{
    capabilities,
    detection::{inspect_for, parameters},
    evaluation, store,
};
use crate::{config::View, dto::operation_error, optimize_dto::*};
use anyhow::Result;
use std::path::Path;
pub(crate) fn execute(
    r: Request,
    id: String,
    view: &View,
    _baseline: Option<&View>,
) -> Result<Response> {
    let path = crate::storage::data_home()?.join("user-v1/reviews.sqlite3");
    run(r, id, view, None, &path)
}
#[cfg(test)]
pub(super) fn execute_at(r: Request, id: String, view: &View, path: &Path) -> Result<Response> {
    run(r, id, view, None, path)
}
fn detect(view: &View, r: &Request, rules: &RuleParameters) -> (Vec<Suggestion>, bool) {
    let (mut rows, partial) = inspect_for(
        view,
        rules,
        r.project.as_deref(),
        r.source_instance_id.as_deref(),
    );
    for s in &mut rows {
        s.scope_project = r.project.clone();
        s.id = crate::hash(format!("{}:{}", s.id, r.project.as_deref().unwrap_or("")));
    }
    (rows, partial)
}
pub(super) fn run(
    r: Request,
    id: String,
    view: &View,
    _baseline: Option<&View>,
    path: &Path,
) -> Result<Response> {
    super::activity::validate(&r)?;
    if r.project
        .as_ref()
        .is_some_and(|p| !view.projects.contains(p))
    {
        return Err(operation_error(
            "PROJECT_NOT_AUTHORIZED",
            "Project is outside this view",
        ));
    }
    let offset = r.offset.unwrap_or(0);
    let limit = r.limit.unwrap_or(50);
    if limit == 0 || limit > 200 || offset > 9_007_199_254_740_991 {
        return Err(operation_error("INVALID_ARGUMENT", "Invalid page"));
    }
    let rules = parameters(r.rule_overrides.clone())?;
    let (mut current, coverage_partial) = detect(view, &r, &rules);
    let baseline_id = |s: &Suggestion| -> Result<String> {
        Ok(crate::hash(serde_json::to_vec(&(
            &s.id,
            &s.item,
            &s.findings,
            &s.checks,
            &r.project,
            &r.source_instance_id,
            &rules,
        ))?))
    };
    for s in &mut current {
        s.baseline_id = Some(baseline_id(s)?);
    }
    if matches!(r.action, Action::Keep | Action::NotApplicable) {
        let selected = current
            .iter()
            .find(|s| Some(&s.id) == r.suggestion_id.as_ref())
            .ok_or_else(|| operation_error("NOT_FOUND", "Selected finding is unavailable"))?;
        store::decide(path, &r, selected)?;
    }
    let revision = store::read(path, &mut current, r.project.as_deref())?;
    if !matches!(r.action, Action::Keep | Action::NotApplicable)
        && r.decision_revision
            .as_ref()
            .is_some_and(|expected| expected != &revision)
    {
        return Err(operation_error(
            "DECISION_CHANGED",
            "Decision revision changed",
        ));
    }
    let mut checks = Vec::new();
    if r.action == Action::Recheck {
        let old = r.baseline.as_deref().ok_or_else(|| {
            operation_error("INVALID_ARGUMENT", "Recheck requires the original finding")
        })?;
        if old.review_format_version != 1 {
            return Err(operation_error(
                "UNSUPPORTED_VERSION",
                "Unsupported original finding format; save a new check report",
            ));
        }
        let original_scope = &old
            .checks
            .first()
            .ok_or_else(|| {
                operation_error("EVIDENCE_UNAVAILABLE", "Original check scope is missing")
            })?
            .basis
            .scope;
        if original_scope
            .roots
            .iter()
            .collect::<std::collections::BTreeSet<_>>()
            != view.roots.iter().collect()
            || original_scope
                .project_roots
                .iter()
                .collect::<std::collections::BTreeSet<_>>()
                != view.project_roots.iter().collect()
        {
            return Err(operation_error(
                "INVALID_ARGUMENT",
                "Recheck must retain the original source and project roots",
            ));
        }
        if old.baseline_id.as_ref() != r.baseline_id.as_ref()
            || old.baseline_id.as_deref() != Some(baseline_id(old)?.as_str())
            || r.suggestion_id.as_ref().is_some_and(|id| id != &old.id)
            || old.scope_project != r.project
        {
            return Err(operation_error(
                "INVALID_ARGUMENT",
                "Original finding scope or rule parameters changed",
            ));
        }
        for old in [old] {
            let actual = view.items.iter().find(|i| {
                i.id == old.item.id
                    && i.applies(r.source_instance_id.as_deref(), r.project.as_deref())
            });
            let scope = &old
                .checks
                .first()
                .ok_or_else(|| {
                    operation_error("EVIDENCE_UNAVAILABLE", "Original check is incomplete")
                })?
                .basis
                .scope;
            let mut outcomes = evaluation::evaluate(&evaluation::Input {
                view,
                item: actual.unwrap_or(&old.item),
                parameters: &rules,
                project: r.project.as_deref(),
                source: r.source_instance_id.as_deref(),
                baseline: Some(evaluation::Baseline {
                    findings: &old.findings,
                    assessments: &old.checks,
                    scope,
                }),
                current_available: actual.is_some(),
            });
            for prior in old
                .checks
                .iter()
                .filter(|c| old.findings.iter().any(|f| f.rule == c.rule))
            {
                if outcomes.iter().any(|c| c.rule == prior.rule) {
                    continue;
                }
                let mut gap = prior.clone();
                gap.outcome = RuleOutcome::Insufficient;
                gap.reason = Some("currentRuleNotApplicable".into());
                gap.assessment_id = None;
                gap.identity_gap = gap.reason.clone();
                gap.findings.clear();
                gap.content_version = actual.map(|i| i.content_hash.clone()).unwrap_or_default();
                gap.checked_at = view.checked.clone();
                gap.basis.cutoff = view.checked.clone();
                gap.basis.dependency_revision = None;
                gap.basis.gaps.push("currentRuleNotApplicable".into());
                match &mut gap.basis.measurement {
                    RuleMeasurement::Numeric { observed, .. } => *observed = None,
                    RuleMeasurement::SkillMetadata { status, issues } => {
                        *status = None;
                        issues.clear();
                    }
                    RuleMeasurement::Static { complete, findings } => {
                        *complete = None;
                        *findings = 0;
                    }
                    RuleMeasurement::Existence {
                        configured_state,
                        measurement_status,
                        missing,
                    } => {
                        *configured_state = "unknown".into();
                        *measurement_status = "unavailable".into();
                        *missing = false;
                    }
                }
                gap.comparison = AssessmentComparison {
                    status: ComparisonStatus::Unknown,
                    baseline_assessment_id: prior.assessment_id.clone(),
                    reason: gap.reason.clone(),
                };
                outcomes.push(gap);
            }
            checks.extend(outcomes);
        }
    } else if r.action == Action::Checks {
        let items: Vec<_> = view
            .items
            .iter()
            .filter(|i| {
                i.applies(r.source_instance_id.as_deref(), r.project.as_deref())
                    && r.item_id.as_ref().is_none_or(|id| id == &i.id)
            })
            .collect();
        if r.item_id.is_some() && items.is_empty() {
            return Err(operation_error("NOT_FOUND", "Check target is unavailable"));
        }
        for item in items.iter().skip(offset).take(limit) {
            checks.extend(evaluation::evaluate(
                &evaluation::Input::initial(view, item, &rules, r.project.as_deref())
                    .with_source(r.source_instance_id.as_deref()),
            ));
        }
    }
    let pending = current.len();
    if r.action == Action::Detail {
        current.retain(|s| Some(&s.id) == r.suggestion_id.as_ref());
        if current.is_empty() {
            return Err(operation_error("NOT_FOUND", "Finding is unavailable"));
        }
    }
    if let Some(category) = &r.category {
        current.retain(|s| &s.category == category);
    }
    let total = current.len();
    let partial = coverage_partial
        || current
            .iter()
            .flat_map(|s| &s.checks)
            .chain(&checks)
            .any(|c| {
                matches!(
                    c.outcome,
                    RuleOutcome::Insufficient | RuleOutcome::Error | RuleOutcome::Unsupported
                ) || matches!(
                    c.comparison.status,
                    ComparisonStatus::Unknown | ComparisonStatus::Incomparable
                )
            });
    let mut out = capabilities();
    out.action = r.action;
    out.read_view = Some(id);
    out.config_revision = view.revision.clone();
    out.usage_revision = view
        .snapshot
        .as_ref()
        .map(|s| s.manifest.snapshot_ref.snapshot_id.clone());
    out.decision_revision = revision;
    out.checked_at = view.checked.clone();
    out.pending = pending;
    out.page = crate::usage_app_dto::Page {
        offset,
        limit,
        total,
        next_offset: (offset.saturating_add(limit) < total).then_some(offset.saturating_add(limit)),
    };
    out.suggestions = current.into_iter().skip(offset).take(limit).collect();
    out.checks = checks;
    out.issues = view.issues.clone();
    out.result_status = if partial { "partial" } else { "complete" }.into();
    out.rule_parameters = rules;
    Ok(out)
}
