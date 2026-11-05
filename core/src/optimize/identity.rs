//! Bounded, versioned problem/check identities and immutable review evidence.
//! Reuses the exact dependency projection; never hashes bodies or a global log revision.
use super::{cache, evaluation::Input, inputs::RuleInput, registry::Rule};
use crate::optimize_dto::*;

pub(super) const VERSION: u32 = 1;
fn sorted(mut values: Vec<String>) -> Vec<String> {
    values.sort_unstable();
    values.dedup();
    values
}
pub(super) fn scope(input: &Input<'_>) -> AssessmentScope {
    let max = 1024;
    let complete = [
        &input.view.roots,
        &input.view.project_roots,
        &input.view.config_collection.source_roots,
        &input.item.authorized_projects,
    ]
    .iter()
    .all(|v| v.len() <= max)
        && input.item.source_contexts.len() <= max;
    let copy = |values: &[String]| {
        if complete {
            sorted(values.to_vec())
        } else {
            vec![]
        }
    };
    let sources = if complete {
        sorted(
            input
                .item
                .source_contexts
                .iter()
                .map(|s| s.source_instance_id.clone())
                .chain(std::iter::once(input.item.source_instance_id.clone()))
                .collect(),
        )
    } else {
        vec![]
    };
    AssessmentScope {
        source_instance_id: input.source.map(str::to_owned),
        item_project: input.item.project.clone(),
        global: if input.item.source_contexts.is_empty() {
            input.item.project.is_none()
        } else {
            input.item.source_contexts.iter().any(|s| s.global)
        },
        project: input.project.map(str::to_owned),
        source_instances: sources,
        authorized_projects: copy(&input.item.authorized_projects),
        roots: copy(&input.view.roots),
        project_roots: copy(&input.view.project_roots),
        source_roots: copy(&input.view.config_collection.source_roots),
        complete,
    }
}
pub(super) fn methods(rule: Rule, input: &Input<'_>, findings: &[Finding]) -> Vec<MethodVersion> {
    let mut methods = vec![MethodVersion {
        method: rule.dependencies().method.into(),
        version: 1,
    }];
    // These are the collectors actually consumed by the current static algorithms.
    let collector = match rule {
        Rule::LocalReference => Some("pulldown-cmark-0.13.4/markdown-resources-v1"),
        Rule::ExactInstructionBlocks => Some("pulldown-cmark-0.13.4/raw-utf8-v1"),
        Rule::HookTarget => Some("codex-0.160.0/shlex-2.0.1/static-script-v1"),
        _ => None,
    };
    if let Some(method) = collector
        && !methods.iter().any(|v| v.method == method)
    {
        methods.push(MethodVersion {
            method: method.into(),
            version: 1,
        });
    }
    if matches!(rule, Rule::FileSize)
        && let Some(source) = &input.item.bytes_source
    {
        methods.push(MethodVersion {
            method: format!("measurementSource:{source}"),
            version: 1,
        });
    }
    for evidence in findings.iter().filter_map(|f| f.evidence.as_ref()) {
        if !methods.iter().any(|v| v.method == evidence.method) {
            methods.push(MethodVersion {
                method: evidence.method.clone(),
                version: 1,
            });
        }
    }
    methods.sort_unstable_by(|a, b| a.method.cmp(&b.method));
    methods
}
pub(super) fn basis(
    rule: Rule,
    input: &Input<'_>,
    prepared: &RuleInput<'_>,
    revision: Option<String>,
) -> AssessmentBasis {
    let measurement = match prepared {
        RuleInput::Existence { missing } => RuleMeasurement::Existence {
            configured_state: input.item.configured_state.clone(),
            measurement_status: input.item.measurement_status.clone(),
            missing: *missing,
        },
        RuleInput::Numeric {
            basis,
            observed,
            threshold,
            inclusive,
        } => RuleMeasurement::Numeric {
            basis: (*basis).into(),
            observed: *observed,
            threshold: *threshold,
            inclusive: *inclusive,
            standard_max: matches!(rule, Rule::DescriptionSize)
                .then_some(input.parameters.description_standard_max),
            suppressed_by_standard: false,
        },
        RuleInput::Satisfied => RuleMeasurement::Numeric {
            basis: "productReminder".into(),
            observed: input
                .item
                .skill_metadata
                .as_ref()
                .and_then(|m| m.description_characters),
            threshold: super::evaluation::description_characters(input.parameters),
            inclusive: false,
            standard_max: Some(input.parameters.description_standard_max),
            suppressed_by_standard: true,
        },
        RuleInput::Format { status, issues } => RuleMeasurement::SkillMetadata {
            status: status.map(str::to_owned),
            issues: issues.to_vec(),
        },
        RuleInput::Static {
            findings, complete, ..
        } => RuleMeasurement::Static {
            complete: *complete,
            findings: findings.len(),
        },
    };
    let scope = scope(input);
    let mut gaps = vec![];
    if revision.is_none() {
        gaps.push("dependencyIdentityBudgetExceeded".into());
    }
    if !input.item.applies(input.source, input.project)
        || input
            .project
            .is_some_and(|p| !input.view.projects.iter().any(|v| v == p))
    {
        gaps.push("checkScopeUnavailable".into());
    }
    if !scope.complete {
        gaps.push("scopeIdentityBudgetExceeded".into());
    }
    if !input.current_available || !input.item.current || input.item.stale {
        gaps.push("currentVersionUnavailable".into());
    }
    AssessmentBasis {
        version: VERSION,
        dependency_revision: revision,
        scope,
        cutoff: input.view.checked.clone(),
        applicability: input.parameters.applicability.clone(),
        measurement,
        gaps,
    }
}
pub(super) fn finding(input: &Input<'_>, finding: &Finding) -> FindingIdentity {
    let scope = scope(input);
    let gap = if input.item.id.is_empty() || input.item.source_instance_id.is_empty() {
        Some("objectIdentityUnavailable")
    } else if !scope.complete {
        Some("scopeIdentityBudgetExceeded")
    } else if finding.rule == "exactInstructionBlocks" {
        Some("problemLocationContextUnavailable")
    } else {
        None
    };
    if let Some(gap) = gap {
        return FindingIdentity {
            version: VERSION,
            finding_id: None,
            gap: Some(gap.into()),
        };
    }
    let key = match finding.rule.as_str() {
        "localReference" => finding.evidence.as_ref().and_then(|evidence| {
            if evidence.references.is_empty()
                || !evidence.versions.iter().any(|version| {
                    version.item_id == input.item.id && version.path == input.item.path
                })
            {
                return None;
            }
            // The problem is the owner's resource requirement, independent of file hashes,
            // error status, repeated occurrences and line shifts in the owning document.
            let mut targets = evidence
                .references
                .iter()
                .map(|reference| {
                    (
                        reference.base_directory.as_str(),
                        reference.target.as_str(),
                        reference.expected_type.as_deref(),
                    )
                })
                .collect::<Vec<_>>();
            if targets
                .iter()
                .any(|(base, target, _)| base.is_empty() || target.is_empty())
            {
                return None;
            }
            targets.sort_unstable();
            targets.dedup();
            cache::key(
                &("reference-finding-v1", &input.item.id, &scope, targets),
                128 * 1024,
            )
        }),
        "declaredCopyDrift" => finding.evidence.as_ref().and_then(|e| {
            let relation = e.relation.as_ref()?;
            if [
                &relation.source_instance_id,
                &relation.project,
                &relation.declaration_path,
                &relation.relation_id,
            ]
            .iter()
            .any(|s| s.is_empty())
                || e.direction.is_none()
            {
                return None;
            }
            // The declared relation is the stable location; its hash belongs to the assessment.
            cache::key(
                &(
                    "finding-v1",
                    &finding.rule,
                    &input.item.id,
                    &scope,
                    (
                        &relation.source_instance_id,
                        &relation.project,
                        &relation.declaration_path,
                        &relation.relation_id,
                        &relation.kind,
                    ),
                    &e.direction,
                ),
                128 * 1024,
            )
        }),
        "hookTarget" => finding.evidence.as_ref().and_then(|e| {
            let hook = e.hook.as_ref()?;
            if hook.project.is_empty() || hook.native_key.is_empty() || hook.target.is_empty() {
                return None;
            }
            cache::key(
                &(
                    "finding-v1",
                    &finding.rule,
                    &input.item.id,
                    &scope,
                    &hook.project,
                    &hook.native_key,
                    &hook.target,
                ),
                128 * 1024,
            )
        }),
        _ => cache::key(
            &("finding-v1", &finding.rule, &input.item.id, &scope),
            128 * 1024,
        ),
    };
    FindingIdentity {
        version: VERSION,
        gap: key.is_none().then(|| {
            if finding.rule == "localReference" {
                "problemLocationContextUnavailable"
            } else {
                "reliableProblemIdentityUnavailable"
            }
            .into()
        }),
        finding_id: key,
    }
}
pub(super) fn identify(check: &mut RuleAssessment) {
    if !check.basis.gaps.is_empty() {
        check.identity_gap = check.basis.gaps.first().cloned();
        return;
    }
    check.assessment_id = cache::key(
        &(
            "assessment-v1",
            &check.rule,
            &check.rule_version,
            check.rule_semantics_version,
            &check.method_versions,
            &check.item_id,
            &check.content_version,
            &check.basis,
            &check.outcome,
            &check.reason,
            &check.findings,
        ),
        256 * 1024,
    );
    if check.assessment_id.is_none() {
        check.identity_gap = Some("assessmentIdentityBudgetExceeded".into());
    }
}
pub(super) fn comparison(check: &RuleAssessment, input: &Input<'_>) -> AssessmentComparison {
    let Some(baseline) = &input.baseline else {
        return AssessmentComparison {
            status: ComparisonStatus::NotRequested,
            baseline_assessment_id: None,
            reason: None,
        };
    };
    let old = baseline.assessments.iter().find(|c| c.rule == check.rule);
    let result = |status, reason: &str| AssessmentComparison {
        status,
        baseline_assessment_id: old.and_then(|c| c.assessment_id.clone()),
        reason: Some(reason.into()),
    };
    let Some(old) = old else {
        return result(ComparisonStatus::Unknown, "baselineAssessmentUnavailable");
    };
    if old.assessment_id.is_none()
        || check.assessment_id.is_none()
        || old.basis.dependency_revision.is_none()
        || check.basis.dependency_revision.is_none()
        || !old.basis.scope.complete
        || !check.basis.scope.complete
    {
        return result(ComparisonStatus::Unknown, "assessmentIdentityUnavailable");
    }
    if old.rule_version != check.rule_version
        || old.rule_semantics_version != check.rule_semantics_version
        || old.method_versions != check.method_versions
        || old.basis.applicability != check.basis.applicability
        || !same_measurement_parameters(&old.basis.measurement, &check.basis.measurement)
    {
        return result(
            ComparisonStatus::Incomparable,
            "ruleParametersOrMethodChanged",
        );
    }
    if old.basis.scope != check.basis.scope || baseline.scope != &old.basis.scope {
        return result(ComparisonStatus::Incomparable, "assessmentScopeChanged");
    }
    if baseline
        .findings
        .iter()
        .filter(|f| f.rule == check.rule)
        .any(|f| f.identity.finding_id.is_none())
    {
        return result(ComparisonStatus::Unknown, "problemIdentityUnavailable");
    }
    if !input.current_available
        || input.item.stale
        || !input.item.current
        || !matches!(check.outcome, RuleOutcome::Hit | RuleOutcome::Miss)
        || baseline
            .findings
            .iter()
            .filter(|f| f.rule == check.rule)
            .any(|f| !input.view.analysis.complete_finding(&input.item.id, f))
    {
        return result(ComparisonStatus::Unknown, "checkEvidenceIncomplete");
    }
    AssessmentComparison {
        status: ComparisonStatus::Comparable,
        baseline_assessment_id: old.assessment_id.clone(),
        reason: None,
    }
}
fn same_measurement_parameters(old: &RuleMeasurement, current: &RuleMeasurement) -> bool {
    match (old, current) {
        (
            RuleMeasurement::Numeric {
                basis: a,
                threshold: at,
                inclusive: ai,
                standard_max: am,
                ..
            },
            RuleMeasurement::Numeric {
                basis: b,
                threshold: bt,
                inclusive: bi,
                standard_max: bm,
                ..
            },
        ) => (a, at, ai, am) == (b, bt, bi, bm),
        (RuleMeasurement::Existence { .. }, RuleMeasurement::Existence { .. })
        | (RuleMeasurement::SkillMetadata { .. }, RuleMeasurement::SkillMetadata { .. })
        | (RuleMeasurement::Static { .. }, RuleMeasurement::Static { .. }) => true,
        _ => false,
    }
}
pub(super) fn binding(s: &Suggestion) -> DecisionBinding {
    let finding_ids = sorted(
        s.findings
            .iter()
            .filter_map(|f| f.identity.finding_id.clone())
            .collect(),
    );
    let rules: std::collections::BTreeSet<_> = s.findings.iter().map(|f| f.rule.as_str()).collect();
    let checks: Vec<_> = s
        .checks
        .iter()
        .filter(|c| c.outcome == RuleOutcome::Hit && rules.contains(c.rule.as_str()))
        .collect();
    let assessment_ids = sorted(
        checks
            .iter()
            .filter_map(|c| c.assessment_id.clone())
            .collect(),
    );
    let complete = !rules.is_empty()
        && checks.len() == rules.len()
        && checks
            .iter()
            .map(|c| c.rule.as_str())
            .collect::<std::collections::BTreeSet<_>>()
            == rules
        && checks.iter().all(|c| {
            c.assessment_id.is_some()
                && c.basis.dependency_revision.is_some()
                && c.basis.scope.complete
                && !c.findings.is_empty()
        });
    let applicability_id = if !complete {
        None
    } else {
        cache::key(
            &(
                "decision-applicability-v1",
                &s.id,
                &s.item.content_hash,
                checks
                    .iter()
                    .map(|c| {
                        (
                            &c.rule,
                            &c.rule_version,
                            c.rule_semantics_version,
                            &c.method_versions,
                            &c.basis.dependency_revision,
                            &c.basis.scope,
                            &c.basis.applicability,
                        )
                    })
                    .collect::<Vec<_>>(),
            ),
            256 * 1024,
        )
    };
    let scope = checks
        .first()
        .map(|c| c.basis.scope.clone())
        .unwrap_or_else(|| AssessmentScope {
            source_instance_id: None,
            item_project: None,
            global: false,
            project: s.scope_project.clone(),
            source_instances: vec![],
            authorized_projects: vec![],
            roots: vec![],
            project_roots: vec![],
            source_roots: vec![],
            complete: false,
        });
    let identity_basis = if applicability_id.is_none() {
        DecisionIdentityBasis::Unavailable
    } else if s.findings.iter().all(|f| f.identity.finding_id.is_some()) {
        DecisionIdentityBasis::StableProblems
    } else {
        DecisionIdentityBasis::ExactSuggestionVersion
    };
    DecisionBinding {
        version: VERSION,
        identity_basis,
        suggestion_id: s.id.clone(),
        finding_ids,
        assessment_ids,
        content_version: s.item.content_hash.clone(),
        scope,
        gap: applicability_id
            .is_none()
            .then(|| "decisionApplicabilityUnavailable".into()),
        applicability_id,
    }
}
pub(super) fn decision_applies(old: &UserDecision, current: &Suggestion) -> bool {
    old.binding.version == VERSION
        && old
            .binding
            .applicability_id
            .as_ref()
            .zip(binding(current).applicability_id.as_ref())
            .is_some_and(|(a, b)| a == b)
}

#[cfg(test)]
mod tests;
