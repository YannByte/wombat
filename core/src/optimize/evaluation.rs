//! Pure rule assessments over one captured inventory and its bounded static analysis.
use super::inputs::{RuleInput, prepare};
use super::{cache, registry::Rule};
use crate::{config::View, config_dto::Item, optimize_dto::*};

pub(super) struct Baseline<'a> {
    pub findings: &'a [Finding],
    pub assessments: &'a [RuleAssessment],
    pub scope: &'a AssessmentScope,
}
pub(super) struct Input<'a> {
    pub view: &'a View,
    pub item: &'a Item,
    pub parameters: &'a RuleParameters,
    pub project: Option<&'a str>,
    pub source: Option<&'a str>,
    pub baseline: Option<Baseline<'a>>,
    pub current_available: bool,
}
impl<'a> Input<'a> {
    pub(super) fn with_source(mut self, source: Option<&'a str>) -> Self {
        self.source = source;
        self
    }
    pub(super) fn initial(
        view: &'a View,
        item: &'a Item,
        parameters: &'a RuleParameters,
        project: Option<&'a str>,
    ) -> Self {
        Self {
            view,
            item,
            parameters,
            project,
            source: None,
            baseline: None,
            current_available: true,
        }
    }
}

#[derive(Clone, serde::Serialize)]
pub(super) enum Assessment {
    Hit(Vec<Finding>),
    Miss,
    Insufficient(&'static str),
    Unsupported(&'static str),
}
#[derive(Debug)]
enum EvaluationError {
    InvalidAnalysisRange,
}

pub(super) fn evaluate(input: &Input<'_>) -> Vec<RuleAssessment> {
    evaluate_cached(input, cache::shared())
}
fn evaluate_cached(
    input: &Input<'_>,
    cache: &std::sync::Mutex<cache::JudgmentCache>,
) -> Vec<RuleAssessment> {
    Rule::ALL
        .into_iter()
        .filter(|rule| rule.applies(&input.item.kind))
        .filter(|rule| {
            input
                .baseline
                .as_ref()
                .is_none_or(|b| b.findings.iter().any(|f| f.rule == rule.id()))
        })
        .map(|rule| {
            let prepared = prepare(rule, input);
            let revision = super::inputs::key(rule, input, &prepared);
            let basis = super::identity::basis(rule, input, &prepared, revision.clone());
            let (outcome, reason, findings) =
                match assess_fixed(rule, input, cache, prepared, revision) {
                    Ok(Assessment::Hit(findings)) => (RuleOutcome::Hit, None, findings),
                    Ok(Assessment::Miss) => (RuleOutcome::Miss, None, vec![]),
                    Ok(Assessment::Insufficient(reason)) => {
                        (RuleOutcome::Insufficient, Some(reason), vec![])
                    }
                    Ok(Assessment::Unsupported(reason)) => {
                        (RuleOutcome::Unsupported, Some(reason), vec![])
                    }
                    Err(EvaluationError::InvalidAnalysisRange) => {
                        (RuleOutcome::Error, Some("invalidAnalysisEvidence"), vec![])
                    }
                };
            let findings: Vec<Finding> = findings
                .into_iter()
                .map(|mut f| {
                    f.identity = super::identity::finding(input, &f);
                    f
                })
                .collect();
            let mut check = RuleAssessment {
                assessment_id: None,
                identity_gap: None,
                rule_semantics_version: rule.dependencies().semantics_version,
                method_versions: super::identity::methods(rule, input, &findings),
                basis,
                comparison: AssessmentComparison {
                    status: ComparisonStatus::NotRequested,
                    baseline_assessment_id: None,
                    reason: None,
                },
                rule: rule.id().into(),
                rule_version: input.parameters.version.clone(),
                item_id: input.item.id.clone(),
                content_version: input.item.content_hash.clone(),
                checked_at: input.view.checked.clone(),
                outcome,
                reason: reason.map(str::to_owned),
                findings,
            };
            super::identity::identify(&mut check);
            check.comparison = super::identity::comparison(&check, input);
            check
        })
        .collect()
}

#[cfg(test)]
fn assess(
    rule: Rule,
    input: &Input<'_>,
    cache: &std::sync::Mutex<cache::JudgmentCache>,
) -> Result<Assessment, EvaluationError> {
    let prepared = prepare(rule, input);
    let key = super::inputs::key(rule, input, &prepared);
    assess_fixed(rule, input, cache, prepared, key)
}
fn assess_fixed(
    rule: Rule,
    input: &Input<'_>,
    cache: &std::sync::Mutex<cache::JudgmentCache>,
    prepared: RuleInput<'_>,
    key: Option<String>,
) -> Result<Assessment, EvaluationError> {
    let item = input.item;
    if !item.applies(input.source, input.project)
        || input
            .project
            .is_some_and(|p| !input.view.projects.iter().any(|v| v == p))
    {
        return Ok(Assessment::Insufficient("checkScopeUnavailable"));
    }
    if !input.current_available
        || !item.current
        || item.stale
        || input.baseline.is_some() && item.measurement_status == "missing"
    {
        return Ok(Assessment::Insufficient("currentVersionUnavailable"));
    }
    if !rule.dependencies().evidence.accepts(item) {
        return Ok(Assessment::Insufficient("checkEvidenceIncomplete"));
    }
    let cached = key
        .as_deref()
        .and_then(|key| cache.lock().unwrap_or_else(|e| e.into_inner()).get(key));
    let result = match cached {
        Some(judgment) => judgment,
        None => {
            let judgment = assess_prepared(rule, prepared)?;
            if let Some(entry) = key.and_then(|key| cache::Entry::new(key, &judgment)) {
                cache
                    .lock()
                    .unwrap_or_else(|e| e.into_inner())
                    .insert(entry);
            }
            judgment
        }
    };
    Ok(result)
}
pub(super) fn agents_bytes(p: &RuleParameters) -> u64 {
    p.overrides.agents_bytes.unwrap_or(p.agents_bytes_default)
}
pub(super) fn description_characters(p: &RuleParameters) -> u64 {
    p.overrides
        .description_characters
        .unwrap_or(p.description_characters_default)
}
fn finding(
    rule: Rule,
    basis: &str,
    observed: Option<u64>,
    threshold: Option<u64>,
    codes: Vec<String>,
) -> Finding {
    Finding {
        identity: Default::default(),
        rule: rule.id().into(),
        status: "failed".into(),
        observed,
        threshold,
        evidence_codes: codes,
        basis: Some(basis.into()),
        evidence: None,
    }
}
fn numeric(
    rule: Rule,
    basis: &str,
    observed: Option<u64>,
    threshold: u64,
    inclusive: bool,
) -> Assessment {
    match observed {
        Some(n) if n > threshold || inclusive && n == threshold => {
            Assessment::Hit(vec![finding(rule, basis, Some(n), Some(threshold), vec![])])
        }
        Some(_) => Assessment::Miss,
        None => Assessment::Insufficient("checkEvidenceIncomplete"),
    }
}
fn assess_prepared(rule: Rule, input: RuleInput<'_>) -> Result<Assessment, EvaluationError> {
    Ok(match input {
        RuleInput::Existence { missing: true } => {
            let mut f = finding(rule, "authorizedExistenceCheck", None, None, vec![]);
            f.status = "informational".into();
            Assessment::Hit(vec![f])
        }
        RuleInput::Existence { missing: false } | RuleInput::Satisfied => Assessment::Miss,
        RuleInput::Numeric {
            basis,
            observed,
            threshold,
            inclusive,
        } => numeric(rule, basis, observed, threshold, inclusive),
        RuleInput::Format {
            status: Some("invalid"),
            issues,
        } => Assessment::Hit(vec![finding(
            rule,
            "fieldFormat",
            None,
            None,
            issues.to_vec(),
        )]),
        RuleInput::Format {
            status: Some("parsed"),
            ..
        } => Assessment::Miss,
        RuleInput::Format { .. } => Assessment::Insufficient("checkEvidenceIncomplete"),
        RuleInput::Static {
            findings,
            complete,
            unsupported_reason,
        } => {
            for f in &findings {
                check_intervals(f)?;
            }
            if !findings.is_empty() {
                Assessment::Hit(findings.into_iter().cloned().collect())
            } else {
                match complete {
                    Some(true) => Assessment::Miss,
                    Some(false) => Assessment::Insufficient("checkEvidenceIncomplete"),
                    None => Assessment::Unsupported(unsupported_reason),
                }
            }
        }
    })
}
fn check_intervals(finding: &Finding) -> Result<(), EvaluationError> {
    // Source-derived positions are intervals. Invalid intervals cannot establish
    // a static hit and fail only the rule consuming that analysis.
    if let Some(e) = &finding.evidence {
        for p in &e.positions {
            p.end_byte
                .checked_sub(p.start_byte)
                .ok_or(EvaluationError::InvalidAnalysisRange)?;
            p.end_line
                .checked_sub(p.start_line)
                .ok_or(EvaluationError::InvalidAnalysisRange)?;
        }
        for r in &e.references {
            r.end_byte
                .checked_sub(r.start_byte)
                .ok_or(EvaluationError::InvalidAnalysisRange)?;
            r.end_line
                .checked_sub(r.start_line)
                .ok_or(EvaluationError::InvalidAnalysisRange)?;
        }
    }
    Ok(())
}

#[cfg(test)]
#[path = "cache_tests.rs"]
mod cache_tests;
#[cfg(test)]
mod prepared_tests {
    use super::*;
    #[test]
    fn static_constraint_requires_complete_analysis_even_when_no_issue_was_found() {
        for rule in [
            Rule::LocalReference,
            Rule::ExactInstructionBlocks,
            Rule::DeclaredCopyDrift,
            Rule::HookTarget,
        ] {
            assert!(matches!(
                assess_prepared(
                    rule,
                    RuleInput::Static {
                        findings: vec![],
                        complete: Some(true),
                        unsupported_reason: "analysisUnavailable",
                    }
                )
                .unwrap(),
                Assessment::Miss
            ));
            assert!(matches!(
                assess_prepared(
                    rule,
                    RuleInput::Static {
                        findings: vec![],
                        complete: Some(false),
                        unsupported_reason: "analysisUnavailable",
                    }
                )
                .unwrap(),
                Assessment::Insufficient("checkEvidenceIncomplete")
            ));
            assert!(matches!(
                assess_prepared(
                    rule,
                    RuleInput::Static {
                        findings: vec![],
                        complete: None,
                        unsupported_reason: "analysisUnavailable",
                    }
                )
                .unwrap(),
                Assessment::Unsupported("analysisUnavailable")
            ));
        }
    }
}
