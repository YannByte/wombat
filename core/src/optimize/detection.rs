//! Static rule thresholds and finding composition; no decision persistence.
use crate::{config::View, config_dto::Kind, dto::operation_error, optimize_dto::*};
use anyhow::Result;
pub(crate) fn parameters(overrides: Option<RuleOverrides>) -> Result<RuleParameters> {
    let overrides = overrides.unwrap_or_default();
    if overrides
        .agents_bytes
        .is_some_and(|n| n == 0 || n > 9_007_199_254_740_991)
        || overrides.description_characters.is_some_and(|n| n > 1024)
    {
        return Err(operation_error("INVALID_ARGUMENT", "整理提醒值无效"));
    }
    Ok(RuleParameters {
        overrides,
        ..Default::default()
    })
}
#[cfg(test)]
pub(crate) fn detect(view: &View, rules: &RuleParameters) -> Vec<Suggestion> {
    detect_for(view, rules, None, None)
}
#[cfg(test)]
pub(crate) fn detect_for(
    view: &View,
    rules: &RuleParameters,
    project: Option<&str>,
    source: Option<&str>,
) -> Vec<Suggestion> {
    inspect_for(view, rules, project, source).0
}
pub(super) fn inspect_for(
    view: &View,
    rules: &RuleParameters,
    project: Option<&str>,
    source: Option<&str>,
) -> (Vec<Suggestion>, bool) {
    let mut partial = !view.issues.is_empty();
    let mut suggestions: Vec<_> = view
        .items
        .iter()
        .filter(|item| item.applies(source, project))
        .filter_map(|item| {
            let input =
                super::evaluation::Input::initial(view, item, rules, project).with_source(source);
            let checks = super::evaluation::evaluate(&input);
            partial |= checks.iter().any(|c| {
                matches!(
                    c.outcome,
                    RuleOutcome::Insufficient | RuleOutcome::Unsupported | RuleOutcome::Error
                )
            });
            let mut findings: Vec<_> = checks
                .iter()
                .filter(|c| c.outcome == RuleOutcome::Hit)
                .filter(|c| {
                    !matches!(
                        c.rule.as_str(),
                        "localReference"
                            | "exactInstructionBlocks"
                            | "declaredCopyDrift"
                            | "hookTarget"
                    )
                })
                .flat_map(|c| c.findings.clone())
                .collect();
            // Preserve the static analyzer's evidence ordering and existing suggestion keys.
            for finding in view.analysis.findings.get(&item.id).into_iter().flatten() {
                if matches!(
                    finding.rule.as_str(),
                    "localReference"
                        | "exactInstructionBlocks"
                        | "declaredCopyDrift"
                        | "hookTarget"
                ) && checks.iter().any(|c| {
                    c.outcome == RuleOutcome::Hit
                        && c.rule == finding.rule
                        && project.is_none_or(|p| {
                            finding
                                .evidence
                                .as_ref()
                                .and_then(|e| e.hook.as_ref())
                                .is_none_or(|h| h.project == p)
                        })
                }) {
                    let mut finding = finding.clone();
                    finding.identity = super::identity::finding(&input, &finding);
                    findings.push(finding);
                }
            }
            if findings.is_empty() {
                return None;
            }
            let category = if findings.iter().all(|finding| finding.status == "advisory") {
                Category::Review
            } else if findings.iter().any(|f| {
                f.status != "advisory"
                    && matches!(
                        f.rule.as_str(),
                        "skillFormat"
                            | "localReference"
                            | "descriptionStandard"
                            | "declaredCopyDrift"
                            | "hookTarget"
                    )
            }) {
                Category::Repair
            } else {
                Category::Trim
            };
            let id = crate::hash(
                serde_json::to_vec(&(
                    &item.id,
                    &item.content_hash,
                    &rules.version,
                    rules
                        .overrides
                        .agents_bytes
                        .unwrap_or(rules.agents_bytes_default),
                    rules
                        .overrides
                        .description_characters
                        .unwrap_or(rules.description_characters_default),
                    rules.body_tokens,
                    rules.description_standard_max,
                    group_findings(&findings),
                ))
                .ok()?,
            );
            let suggestion = Suggestion {
                baseline_id: None,
                review_format_version: 1,
                scope_project: None,
                id,
                item: item.clone(),
                category,
                decision: None,
                checks,
                findings,
                checked_at: view.checked.clone(),
                rule_version: rules.version.clone(),
                rule_parameters: Some(rules.clone()),
            };
            Some(suggestion)
        })
        .collect();
    suggestions.sort_by(|a, b| {
        let priority = |s: &Suggestion| {
            if s.findings.iter().any(|f| {
                f.status != "advisory"
                    && matches!(
                        f.rule.as_str(),
                        "skillFormat" | "hookTarget" | "localReference"
                    )
            }) {
                0
            } else if s.findings.iter().any(|f| f.rule == "descriptionStandard") {
                1
            } else if matches!(s.category, Category::Repair | Category::Trim) {
                2
            } else {
                3
            }
        };
        priority(a)
            .cmp(&priority(b))
            .then_with(|| {
                // The verified invocation-count basis currently exists only for MCP.
                // Treat other bases as unknown to keep the ordering transitive.
                let calls = |s: &Suggestion| {
                    if s.item.kind == Kind::Mcp {
                        s.item.usage_count
                    } else {
                        None
                    }
                };
                calls(b).cmp(&calls(a))
            })
            .then_with(|| a.id.cmp(&b.id))
    });
    (suggestions, partial)
}

// Group/version keys do not depend on the new problem identity representation.
fn group_findings(findings: &[Finding]) -> Vec<GroupFinding<'_>> {
    findings
        .iter()
        .map(|f| GroupFinding {
            rule: &f.rule,
            status: &f.status,
            observed: f.observed,
            threshold: f.threshold,
            evidence_codes: &f.evidence_codes,
            basis: &f.basis,
            evidence: &f.evidence,
        })
        .collect()
}
#[derive(serde::Serialize)]
#[serde(rename_all = "camelCase")]
struct GroupFinding<'a> {
    rule: &'a str,
    status: &'a str,
    observed: Option<u64>,
    threshold: Option<u64>,
    evidence_codes: &'a [String],
    basis: &'a Option<String>,
    evidence: &'a Option<StaticEvidence>,
}
