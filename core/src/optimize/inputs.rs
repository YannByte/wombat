//! Typed per-rule measurements and analysis; no I/O or global revision dependencies.
use super::{
    cache,
    evaluation::{Input, agents_bytes, description_characters},
    registry::{Evidence, Rule},
};
use crate::{config_dto::Item, optimize_dto::*};
use std::collections::BTreeSet;

#[derive(serde::Serialize)]
enum Measurement<'a> {
    Existence(&'a str),
    Bytes(Option<&'a str>),
    Metadata(Option<&'a str>, Option<u64>),
    Body(
        &'a str,
        Option<&'a crate::config_dto::ContentEstimate>,
        Option<&'a str>,
    ),
    Analysis(String),
}
fn set(values: &[String]) -> Vec<&str> {
    let mut values: Vec<_> = values.iter().map(String::as_str).collect();
    values.sort_unstable();
    values.dedup();
    values
}
pub(super) fn key(rule: Rule, input: &Input<'_>, prepared: &RuleInput<'_>) -> Option<String> {
    let dependencies = rule.dependencies();
    let max_rows = dependencies.max_dependency_rows;
    let view = input.view;
    let item = input.item;
    if [
        &view.roots,
        &view.project_roots,
        &view.projects,
        &view.config_collection.source_roots,
        &item.authorized_projects,
    ]
    .iter()
    .any(|values| values.len() > max_rows)
        || item.source_contexts.len() > max_rows
    {
        return None;
    }
    if input
        .baseline
        .as_ref()
        .is_some_and(|baseline| baseline.findings.len() > max_rows)
    {
        return None;
    }
    let baseline_projects: Option<BTreeSet<_>> = input.baseline.as_ref().map(|baseline| {
        baseline
            .findings
            .iter()
            .filter(|finding| finding.rule == rule.id())
            .filter_map(|finding| {
                finding
                    .evidence
                    .as_ref()?
                    .hook
                    .as_ref()
                    .map(|hook| hook.project.as_str())
            })
            .collect()
    });
    let mut selected_projects: Vec<_> = view
        .projects
        .iter()
        .filter(|project| {
            input.project.is_none_or(|scope| scope == project.as_str())
                && item.applies(None, Some(project))
                && (!matches!(dependencies.evidence, Evidence::HostTargets)
                    || baseline_projects
                        .as_ref()
                        .is_none_or(|original| original.contains(project.as_str())))
        })
        .map(String::as_str)
        .collect();
    selected_projects.sort_unstable();
    selected_projects.dedup();
    let measurement = match dependencies.evidence {
        Evidence::Existence => Measurement::Existence(&item.configured_state),
        Evidence::FileMeasurement => Measurement::Bytes(item.bytes_source.as_deref()),
        Evidence::SkillMetadata => Measurement::Metadata(
            item.skill_metadata.as_ref().map(|m| m.status.as_str()),
            if matches!(rule, Rule::DescriptionSize | Rule::DescriptionStandard) {
                item.skill_metadata
                    .as_ref()
                    .and_then(|m| m.description_characters)
            } else {
                None
            },
        ),
        Evidence::BodyMeasurement => Measurement::Body(
            &item.body_estimate_status,
            item.body_token_estimate.as_ref(),
            item.skill_metadata.as_ref().map(|m| m.status.as_str()),
        ),
        Evidence::References | Evidence::Blocks | Evidence::Relations | Evidence::HostTargets => {
            Measurement::Analysis(view.analysis.rule_dependency_revision(
                rule.id(),
                item,
                &view.items,
                &selected_projects,
                dependencies.max_key_bytes / 2,
            )?)
        }
    };
    let mut sources: Vec<_> = item
        .source_contexts
        .iter()
        .map(|source| {
            (
                &source.inventory_id,
                &source.source_instance_id,
                source.global,
                &source.content_hash,
                &source.configured_state,
            )
        })
        .collect();
    sources.sort_unstable();
    sources.dedup();
    // Only the target's contextual registrations matter, never host observation timestamps.
    let mut host = vec![];
    let mut host_rows = 0;
    if matches!(dependencies.evidence, Evidence::HostTargets) {
        if view.hook_registry.contexts.len() > max_rows {
            return None;
        }
        let mut candidates = 0usize;
        for context in &view.hook_registry.contexts {
            candidates = candidates.checked_add(context.registrations.len())?;
            if candidates > dependencies.max_candidate_rows {
                return None;
            }
        }
        for context in view
            .hook_registry
            .contexts
            .iter()
            .filter(|context| selected_projects.contains(&context.project.as_str()))
        {
            if host.len() >= max_rows || context.registrations.len() > max_rows {
                return None;
            }
            let mut registrations = vec![];
            for registration in context
                .registrations
                .iter()
                .filter(|registration| registration.item_id == item.id)
            {
                host_rows += 1;
                if host_rows > max_rows {
                    return None;
                }
                registrations.push((
                    &registration.item_id,
                    &registration.native_key,
                    &registration.content_hash,
                    &registration.registration_hash,
                    registration.enabled,
                    match registration.trust {
                        crate::config_dto::HookTrust::Managed => 0,
                        crate::config_dto::HookTrust::Untrusted => 1,
                        crate::config_dto::HookTrust::Trusted => 2,
                        crate::config_dto::HookTrust::Modified => 3,
                    },
                    match registration.handler {
                        crate::config_dto::HookHandler::Command => 0,
                        crate::config_dto::HookHandler::McpTool => 1,
                        crate::config_dto::HookHandler::Prompt => 2,
                        crate::config_dto::HookHandler::Agent => 3,
                    },
                    &registration.source,
                    &registration.plugin_id,
                ));
            }
            registrations.sort_unstable();
            host.push((&context.project, context.complete, registrations));
        }
        host.sort_unstable();
    }
    cache::key(
        &(
            "pure-rule-key-v1",
            rule.id(),
            &dependencies,
            (
                &input.parameters.version,
                &input.parameters.applicability,
                if matches!(rule, Rule::DescriptionSize) {
                    Some((
                        description_characters(input.parameters),
                        input.parameters.description_standard_max,
                    ))
                } else {
                    None
                },
            ),
            (
                &item.id,
                &item.kind,
                &item.path,
                &item.project,
                &item.native_key,
                &item.source_instance_id,
                &item.content_hash,
                &item.measurement_status,
            ),
            (set(&item.authorized_projects), sources),
            (
                set(&view.roots),
                set(&view.config_collection.source_roots),
                set(&view.project_roots),
                set(&view.projects),
                input.project,
                input.source,
            ),
            (measurement, prepared),
            matches!(dependencies.evidence, Evidence::HostTargets).then_some((
                &view.hook_registry.native_version,
                &view.hook_registry.status,
                host,
            )),
        ),
        dependencies.max_key_bytes / 2,
    )
}

fn known_body(item: &Item) -> Option<u64> {
    item.body_token_estimate
        .as_ref()
        .filter(|e| {
            item.body_estimate_status == "estimated"
                && e.payload == "skillBody"
                && e.method == "tiktoken-rs-0.12.0/o200k_base/ordinary-v1"
                && e.encoding == "o200k_base"
                && e.tokenizer_version.as_deref() == Some("tiktoken-rs-0.12.0")
                && e.applicability == "referenceEncodingOnly"
                && !e.content_hash.is_empty()
        })
        .map(|e| e.tokens)
}
/// Prepared inputs contain only the measurements and bounded analysis a rule consumes.
#[derive(serde::Serialize)]
pub(super) enum RuleInput<'a> {
    Existence {
        missing: bool,
    },
    Numeric {
        basis: &'static str,
        observed: Option<u64>,
        threshold: u64,
        inclusive: bool,
    },
    Format {
        status: Option<&'a str>,
        issues: &'a [String],
    },
    Satisfied,
    Static {
        findings: Vec<&'a Finding>,
        complete: Option<bool>,
        unsupported_reason: &'static str,
    },
}
pub(super) fn prepare<'a>(rule: Rule, input: &'a Input<'_>) -> RuleInput<'a> {
    let item = input.item;
    let rules = input.parameters;
    match rule {
        Rule::MissingInstruction => RuleInput::Existence {
            missing: item.configured_state == "missing" && item.measurement_status == "missing",
        },
        Rule::FileSize => RuleInput::Numeric {
            basis: "productReminder",
            observed: item.bytes,
            threshold: agents_bytes(rules),
            inclusive: false,
        },
        Rule::SkillFormat => RuleInput::Format {
            status: item.skill_metadata.as_ref().map(|m| m.status.as_str()),
            issues: item
                .skill_metadata
                .as_ref()
                .map_or(&[], |m| m.issues.as_slice()),
        },
        Rule::DescriptionStandard => RuleInput::Numeric {
            basis: "agentSkillsSpecification",
            observed: item
                .skill_metadata
                .as_ref()
                .and_then(|m| m.description_characters),
            threshold: rules.description_standard_max,
            inclusive: false,
        },
        Rule::DescriptionSize => {
            let observed = item
                .skill_metadata
                .as_ref()
                .and_then(|m| m.description_characters);
            if observed.is_some_and(|n| n > rules.description_standard_max) {
                RuleInput::Satisfied
            } else {
                RuleInput::Numeric {
                    basis: "productReminder",
                    observed,
                    threshold: description_characters(rules),
                    inclusive: false,
                }
            }
        }
        Rule::BodyTokens => RuleInput::Numeric {
            basis: "agentSkillsRecommendation",
            observed: item
                .skill_metadata
                .as_ref()
                .filter(|m| m.status == "parsed")
                .and_then(|_| known_body(item)),
            threshold: rules.body_tokens,
            inclusive: true,
        },
        Rule::LocalReference
        | Rule::ExactInstructionBlocks
        | Rule::DeclaredCopyDrift
        | Rule::HookTarget => prepare_static(rule, input),
    }
}
fn prepare_static<'a>(rule: Rule, input: &'a Input<'_>) -> RuleInput<'a> {
    let item = input.item;
    let view = input.view;
    let baseline_projects: Option<BTreeSet<_>> = input.baseline.as_ref().map(|b| {
        b.findings
            .iter()
            .filter(|f| f.rule == rule.id())
            .filter_map(|f| {
                f.evidence
                    .as_ref()?
                    .hook
                    .as_ref()
                    .map(|h| h.project.as_str())
            })
            .collect()
    });
    let included = |finding: &&Finding| {
        finding.rule == rule.id()
            && finding
                .evidence
                .as_ref()
                .and_then(|e| e.hook.as_ref())
                .is_none_or(|h| {
                    input.project.is_none_or(|p| p == h.project)
                        && baseline_projects
                            .as_ref()
                            .is_none_or(|p| p.contains(h.project.as_str()))
                })
    };
    let findings = view
        .analysis
        .findings
        .get(&item.id)
        .into_iter()
        .flatten()
        .filter(included)
        .collect();
    let complete = match rule {
        Rule::ExactInstructionBlocks => Some(view.analysis.blocks_complete(&item.id)),
        Rule::LocalReference => Some(
            view.analysis
                .reference_checks
                .get(&item.id)
                .is_some_and(|c| c.complete),
        ),
        Rule::DeclaredCopyDrift => view.analysis.copy_complete(&item.id),
        Rule::HookTarget => {
            if !cfg!(unix)
                || matches!(
                    view.hook_registry.status,
                    crate::config_dto::HookRegistryStatus::Unavailable
                )
            {
                None
            } else {
                let projects: Vec<_> = view
                    .projects
                    .iter()
                    .filter(|p| {
                        item.applies(None, Some(p))
                            && input.project.is_none_or(|scope| scope == *p)
                            && baseline_projects
                                .as_ref()
                                .is_none_or(|original| original.contains(p.as_str()))
                    })
                    .collect();
                Some(
                    !projects.is_empty()
                        && baseline_projects
                            .as_ref()
                            .is_none_or(|p| p.len() == projects.len())
                        && projects.iter().all(|p| {
                            view.analysis
                                .hook_checks
                                .get(&(item.id.clone(), (*p).clone()))
                                == Some(&true)
                        }),
                )
            }
        }
        _ => unreachable!("only static rules consume static analysis"),
    };
    RuleInput::Static {
        findings,
        complete,
        unsupported_reason: if matches!(rule, Rule::DeclaredCopyDrift) {
            "copyRelationNotDeclared"
        } else {
            "verifiedHostAdapterUnavailable"
        },
    }
}
