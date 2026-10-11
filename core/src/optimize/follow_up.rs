//! Post-check use, exact native read-result matches and descriptive usage comparisons.
use crate::{
    adapters::contract::{Operation, Thread},
    config::View,
    config_dto::{Item, Kind},
    optimize_dto::*,
    usage_observations::{self, Projection, UseKind},
};
use chrono::{DateTime, FixedOffset, Utc};
use std::collections::{BTreeMap, BTreeSet};

type Time = DateTime<FixedOffset>;
struct Context<'a> {
    after: Option<Time>,
    project: Option<&'a str>,
    item: Option<&'a Item>,
    revision_hash: &'a str,
    available: bool,
}
impl Context<'_> {
    fn contains(
        &self,
        thread: &Thread,
        at: Option<Time>,
        cutoff: Option<Time>,
        source: Option<&str>,
    ) -> bool {
        self.available
            && source.is_none_or(|source| source == thread.source_instance_id)
            && self.item.is_some_and(|item| {
                item.in_source(&thread.source_instance_id)
                    && (item.kind != Kind::Mcp || mcp_project_matches(item, thread))
            })
            && self
                .project
                .is_none_or(|project| thread.project.as_deref() == Some(project))
            && at.is_none_or(|at| {
                self.after
                    .zip(cutoff)
                    .is_some_and(|(after, cutoff)| at > after && at <= cutoff)
            })
    }
}
fn mcp_project_matches(item: &Item, thread: &Thread) -> bool {
    item.project
        .as_deref()
        .is_none_or(|project| thread.project.as_deref() == Some(project))
}
fn read_path(operation: &Operation, thread: &Thread) -> Option<String> {
    usage_observations::normalized_path(operation.path.as_deref()?, thread.project.as_deref())
}
fn actual_for(item: &Item, use_kind: Option<UseKind>, file_read: bool) -> bool {
    match item.kind {
        Kind::Skill => use_kind == Some(UseKind::SkillRead),
        Kind::Mcp => matches!(use_kind, Some(UseKind::McpTool | UseKind::McpResource)),
        // Reading a rule file remains a distinct observation, not Skill/MCP use.
        Kind::Rule => file_read,
        Kind::Hook => false,
    }
}

/// Each fixed view uses (recheck time, view.checked], never the current clock.
/// Per-record projections retain identity/time/target/dispatch gaps alongside
/// canonical observed counts. Positive observations do not establish adoption.
pub(super) fn observe(
    suggestions: &[Suggestion],
    view: &View,
    source: Option<&str>,
) -> anyhow::Result<Vec<FollowUpObservation>> {
    let mut out = vec![];
    let mut files = BTreeMap::<(String, String), Vec<usize>>::new();
    let mut servers = BTreeMap::<(String, String), Vec<usize>>::new();
    let mut contexts = vec![];
    let cutoff = DateTime::parse_from_rfc3339(&view.checked).ok();
    for suggestion in suggestions
        .iter()
        .filter(|suggestion| suggestion.status == "verified")
    {
        let Some(record) = &suggestion.record_id else {
            continue;
        };
        let after = DateTime::parse_from_rfc3339(&suggestion.checked_at).ok();
        let item = view
            .items
            .iter()
            .find(|item| item.id == suggestion.item.id && item.current && !item.stale);
        let available = source.is_none_or(|source| item.is_some_and(|item| item.in_source(source)))
            && after
                .zip(cutoff)
                .is_some_and(|(after, cutoff)| after <= cutoff)
            && item.is_some_and(|item| item.kind != Kind::Hook)
            && view
                .snapshot
                .as_ref()
                .is_some_and(|snapshot| snapshot.is_live())
            && matches!(
                view.history_status.as_str(),
                "current" | "fixed" | "partial"
            );
        let index = out.len();
        out.push(FollowUpObservation {
            matching_read_records: item
                .filter(|i| matches!(i.kind, Kind::Rule | Kind::Skill))
                .map(|_| 0),
            matching_load_records: item.filter(|i| i.kind == Kind::Rule).map(|_| 0),
            usage_comparison: None,
            usage_comparison_unavailable: Some("observation_unavailable".into()),
            record_id: record.clone(),
            suggestion_id: suggestion.id.clone(),
            status: if available {
                FollowUpStatus::NoObservedRecords
            } else {
                FollowUpStatus::Unavailable
            },
            after: suggestion.checked_at.clone(),
            observed_at: view.checked.clone(),
            observed_records: None,
            use_basis: Some(usage_observations::basis(
                None,
                if suggestion.item.kind == Kind::Rule {
                    crate::config_dto::UseUnit::RuleRead
                } else {
                    crate::config_dto::UseUnit::ObjectUse
                },
                crate::config_dto::UseScope {
                    source_instance_ids: item
                        .unwrap_or(&suggestion.item)
                        .source_ids()
                        .filter(|id| source.is_none_or(|source| source == *id))
                        .map(str::to_owned)
                        .collect::<BTreeSet<_>>()
                        .into_iter()
                        .collect(),
                    project: suggestion.scope_project.clone(),
                    thread_id: None,
                    agent_kind: None,
                    window: crate::config_dto::UseWindow::FollowUp {
                        after: suggestion.checked_at.clone(),
                        through: view.checked.clone(),
                    },
                },
                &view.checked,
                view.snapshot.as_deref(),
                true,
            )),
            last_record_at: None,
            usage_revision: view
                .snapshot
                .as_ref()
                .map(|snapshot| snapshot.manifest.snapshot_ref.snapshot_id.clone()),
            absence_observable: false,
        });
        contexts.push(Context {
            after,
            project: suggestion.scope_project.as_deref(),
            item,
            revision_hash: &suggestion.item.content_hash,
            available,
        });
        if !available {
            continue;
        }
        let item = item.unwrap();
        let entries = match item.kind {
            Kind::Rule | Kind::Skill => &mut files,
            Kind::Mcp => &mut servers,
            Kind::Hook => continue,
        };
        for source_id in item
            .source_ids()
            .filter(|id| source.is_none_or(|source| source == *id))
        {
            let key = if item.kind == Kind::Mcp {
                item.native_key.as_deref().unwrap_or("")
            } else {
                &item.path
            };
            let indices = entries.entry((source_id.into(), key.into())).or_default();
            if !indices.contains(&index) {
                indices.push(index);
            }
        }
    }
    let Some(snapshot) = &view.snapshot else {
        return Ok(out);
    };
    if files.is_empty() && servers.is_empty() {
        return Ok(out);
    }
    // Current declarations outside the suggestion page also constrain ownership.
    let mut file_items = BTreeMap::<(String, String), Vec<&Item>>::new();
    let mut mcp_items = BTreeMap::<(String, String), Vec<&Item>>::new();
    for item in view.items.iter().filter(|item| item.current && !item.stale) {
        let (tracked, owners, key) = match item.kind {
            Kind::Mcp => (
                &servers,
                &mut mcp_items,
                item.native_key.as_deref().unwrap_or(""),
            ),
            Kind::Rule | Kind::Skill => (&files, &mut file_items, item.path.as_str()),
            Kind::Hook => continue,
        };
        for source_id in item.source_ids() {
            let key = (source_id.to_owned(), key.to_owned());
            if tracked.contains_key(&key) {
                owners.entry(key).or_default().push(item);
            }
        }
    }
    let threads: BTreeMap<_, _> = snapshot
        .manifest
        .threads
        .iter()
        .map(|thread| (thread.thread.id.as_str(), &thread.thread))
        .collect();
    let mut projections = (0..out.len())
        .map(|_| Projection::default())
        .collect::<Vec<_>>();
    let mut latest = vec![None; out.len()];
    let operations = snapshot.operation_facts()?;
    let conflicts = usage_observations::target_conflicts(
        || operations.iter().map(AsRef::as_ref),
        |op| {
            threads
                .get(op.thread_id.as_ref())
                .and_then(|thread| thread.project.as_deref())
        },
        |op| {
            let Some(thread) = threads.get(op.thread_id.as_ref()) else {
                return false;
            };
            let at = op
                .timestamp
                .as_deref()
                .and_then(|time| DateTime::parse_from_rfc3339(time).ok());
            if !contexts
                .iter()
                .any(|context| context.contains(thread, at, cutoff, source))
            {
                return false;
            }
            if let Some(reads) = usage_observations::read_targets(op) {
                reads.unbound
                    || reads.paths.iter().any(|raw| {
                        reads
                            .resolve(raw, thread.project.as_deref())
                            .is_some_and(|path| {
                                files.contains_key(&(thread.source_instance_id.clone(), path))
                            })
                    })
            } else {
                op.server.as_ref().is_some_and(|server| {
                    servers.contains_key(&(thread.source_instance_id.clone(), server.to_string()))
                })
            }
        },
    );
    let mut seen = BTreeSet::new();
    let mut seen_gaps = BTreeSet::new();
    for operation in &operations {
        let Some(thread) = threads.get(operation.thread_id.as_ref()) else {
            continue;
        };
        let at = operation
            .timestamp
            .as_deref()
            .and_then(|time| DateTime::parse_from_rfc3339(time).ok());
        let use_kind = usage_observations::use_kind(operation);
        let candidate_read = usage_observations::is_skill_read_candidate(operation);
        let reads = usage_observations::read_targets(operation);
        let file_read = reads.is_some()
            || operation.name.as_ref() == "read_file"
                && matches!(operation.kind.as_ref(), "tool" | "skillRead");
        let unknown_mcp = matches!(operation.kind.as_ref(), "mcpConflict" | "mcpUnclassified");
        if operation.kind.as_ref() == "instructionLoad" {
            for (index, context) in contexts.iter().enumerate() {
                if context.contains(thread, at, cutoff, source)
                    && context.item.is_some_and(|item| {
                        item.kind == Kind::Rule
                            && read_path(operation, thread).as_deref() == Some(item.path.as_str())
                    })
                    && operation.text_result.as_ref().is_some_and(|result| {
                        result.method_version == 1
                            && !result.conflicting
                            && result.hash.as_deref() == Some(context.revision_hash)
                    })
                    && at.is_some()
                    // Native instruction injection is not a tool dispatch and need not
                    // have a call/item ID. The adapter's canonical identity deduplicates loads.
                    && !operation.thread_id.is_empty()
                    && !operation.id.is_empty()
                    && seen.insert(((operation.thread_id.as_ref(), operation.id.as_str()), index))
                {
                    out[index].matching_load_records =
                        out[index].matching_load_records.map(|n| n + 1);
                }
            }
        }
        if use_kind.is_none() && !candidate_read && !file_read && !unknown_mcp {
            continue;
        }
        let mcp = matches!(use_kind, Some(UseKind::McpTool | UseKind::McpResource)) || unknown_mcp;
        let mut keys = BTreeSet::new();
        if mcp {
            keys.insert(operation.server.as_deref().map(str::to_owned));
        } else if let Some(reads) = &reads {
            if reads.unbound {
                keys.insert(None);
            }
            for raw in &reads.paths {
                keys.insert(reads.resolve(raw, thread.project.as_deref()));
            }
        } else {
            keys.insert(read_path(operation, thread));
        }
        for key in keys {
            let candidates = key.as_ref().and_then(|key| {
                let map = if mcp { &servers } else { &files };
                map.get(&(thread.source_instance_id.clone(), key.clone()))
            });
            let relevant = |context: &Context<'_>| {
                context.item.is_some_and(|item| match item.kind {
                    Kind::Skill => use_kind == Some(UseKind::SkillRead) || candidate_read,
                    Kind::Rule => file_read,
                    Kind::Mcp => mcp,
                    Kind::Hook => false,
                })
            };
            let indices: Vec<_> = match candidates {
                Some(candidates) => candidates
                    .iter()
                    .copied()
                    .filter(|index| {
                        contexts[*index].contains(thread, at, cutoff, source)
                            && relevant(&contexts[*index])
                    })
                    .collect(),
                None if key.is_none() => contexts
                    .iter()
                    .enumerate()
                    .filter(|(_, context)| {
                        context.contains(thread, at, cutoff, source) && relevant(context)
                    })
                    .map(|(index, _)| index)
                    .collect(),
                None => vec![],
            };
            if indices.is_empty() {
                continue;
            }
            let owners = key.as_ref().and_then(|key| {
                let owners = if mcp { &mcp_items } else { &file_items };
                owners.get(&(thread.source_instance_id.clone(), key.clone()))
            });
            let owner = contexts[indices[0]].item.unwrap().id.as_str();
            let ambiguous = indices
                .iter()
                .any(|index| contexts[*index].item.unwrap().id != owner)
                || owners.is_some_and(|owners| {
                    owners
                        .iter()
                        .any(|item| (!mcp || mcp_project_matches(item, thread)) && item.id != owner)
                });
            for index in indices {
                if at.is_none() {
                    projections[index].note_time_gap(operation);
                }
                let projection = &mut projections[index];
                if key.is_none()
                    || ambiguous
                    || unknown_mcp
                    || usage_observations::operation_identity(operation)
                        .is_some_and(|identity| conflicts.contains(&identity))
                {
                    if usage_observations::operation_identity(operation)
                        .is_none_or(|identity| seen_gaps.insert((identity, index)))
                    {
                        projection.coverage.target_gaps += 1;
                    }
                    continue;
                }
                let item = contexts[index].item.unwrap();
                if reads.as_ref().is_some_and(|reads| reads.candidate)
                    && matches!(item.kind, Kind::Skill | Kind::Rule)
                {
                    if usage_observations::operation_identity(operation)
                        .is_none_or(|identity| seen_gaps.insert((identity, index)))
                    {
                        projection.coverage.dispatch_gaps += 1;
                    }
                    continue;
                }
                if !actual_for(item, use_kind, file_read) {
                    continue;
                }
                let Some(at) = at else {
                    continue;
                };
                if let Some(identity) = usage_observations::operation_identity(operation)
                    && !seen.insert((identity, index))
                {
                    continue;
                }
                projection.observe_with_time_accounted(operation);
                if file_read
                    && reads
                        .as_ref()
                        .is_none_or(|reads| !reads.unbound && reads.paths.len() == 1)
                    && matches!(item.kind, Kind::Rule | Kind::Skill)
                    && !contexts[index].revision_hash.is_empty()
                    && operation.text_result.as_ref().is_some_and(|result| {
                        result.method_version == 1
                            && !result.conflicting
                            && result.hash.as_deref() == Some(contexts[index].revision_hash)
                            && result
                                .observed_at
                                .as_deref()
                                .and_then(|at| DateTime::parse_from_rfc3339(at).ok())
                                .is_some_and(|at| {
                                    contexts[index]
                                        .after
                                        .zip(cutoff)
                                        .is_some_and(|(after, cutoff)| at > after && at <= cutoff)
                                })
                    })
                {
                    out[index].matching_read_records =
                        out[index].matching_read_records.map(|n| n + 1);
                }
                if latest[index].is_none_or(|last| at > last) {
                    latest[index] = Some(at);
                    out[index].last_record_at = Some(at.with_timezone(&Utc).to_rfc3339());
                }
            }
        }
    }
    let usage_rows = snapshot.ledger();
    for (index, projection) in projections.iter().enumerate() {
        if !contexts[index].available {
            continue;
        }
        let basis = out[index]
            .use_basis
            .as_ref()
            .expect("fixed observation basis");
        out[index].use_basis = Some(usage_observations::basis(
            Some(projection),
            basis.unit.clone(),
            basis.scope.clone(),
            &view.checked,
            view.snapshot.as_deref(),
            true,
        ));
        let count = projection.observed_count();
        out[index].observed_records = Some(count);
        out[index].status = if out[index].matching_load_records.is_some_and(|n| n > 0) {
            FollowUpStatus::LoadedContentMatched
        } else if out[index].matching_read_records.is_some_and(|n| n > 0) {
            FollowUpStatus::ReadContentMatched
        } else if count > 0 {
            FollowUpStatus::VersionUnknown
        } else {
            FollowUpStatus::NoObservedRecords
        };
        let suggestion = suggestions
            .iter()
            .find(|s| s.id == out[index].suggestion_id)
            .expect("selected suggestion");
        match usage_rows
            .as_ref()
            .map_err(|_| ())
            .and_then(|rows| compare_usage(suggestion, view, source, rows).map_err(|_| ()))
        {
            Ok(comparison) => {
                out[index].usage_comparison = comparison;
                out[index].usage_comparison_unavailable = out[index]
                    .usage_comparison
                    .is_none()
                    .then(|| "empty_follow_up_window".into());
            }
            Err(_) => {
                out[index].usage_comparison_unavailable = Some("usage_facts_unavailable".into())
            }
        }
    }
    Ok(out)
}
fn compare_usage(
    suggestion: &Suggestion,
    view: &View,
    source: Option<&str>,
    rows: &[crate::usage_store::PricedMeasurement],
) -> anyhow::Result<Option<FollowUpUsage>> {
    let Some(snapshot) = &view.snapshot else {
        return Ok(None);
    };
    let change = DateTime::parse_from_rfc3339(&suggestion.checked_at)?.with_timezone(&Utc);
    let through = DateTime::parse_from_rfc3339(&view.checked)?.with_timezone(&Utc);
    let duration = through - change;
    if duration <= chrono::Duration::zero() {
        return Ok(None);
    }
    let start = change
        .checked_sub_signed(duration)
        .ok_or_else(|| crate::dto::operation_error("INVALID_FACTS", "Follow-up window overflow"))?;
    let threads: BTreeMap<_, _> = snapshot
        .manifest
        .threads
        .iter()
        .map(|t| (t.thread.id.as_str(), &t.thread))
        .collect();
    let mut before = vec![];
    let mut after = vec![];
    let mut undated = 0;
    for row in rows {
        if !suggestion.item.in_source(&row.fact.source_instance_id)
            || source.is_some_and(|s| s != row.fact.source_instance_id.as_ref())
            || suggestion.scope_project.as_ref().is_some_and(|project| {
                row.fact
                    .thread_id
                    .as_deref()
                    .and_then(|id| threads.get(id))
                    .and_then(|thread| thread.project.as_ref())
                    != Some(project)
            })
        {
            continue;
        }
        let Some(at) = row
            .fact
            .timestamp
            .as_deref()
            .and_then(|at| DateTime::parse_from_rfc3339(at).ok())
            .map(|at| at.with_timezone(&Utc))
        else {
            undated += 1;
            continue;
        };
        if at > start && at <= change {
            before.push(row);
        } else if at > change && at <= through {
            after.push(row);
        }
    }
    let baseline_usage = crate::usage_app::summarize(&before)?;
    let current_usage = crate::usage_app::summarize(&after)?;
    let incomplete_usage = baseline_usage.complete_token_total().is_none()
        || current_usage.complete_token_total().is_none();
    Ok(Some(FollowUpUsage {
        source_instance_ids: suggestion
            .item
            .source_ids()
            .filter(|id| source.is_none_or(|source| source == *id))
            .map(str::to_owned)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect(),
        method_version: 1,
        scope: crate::usage_app_dto::Scope {
            project: suggestion.scope_project.clone(),
            source_instance_id: source.map(str::to_owned),
            timezone: Some("UTC".into()),
            ..Default::default()
        },
        baseline_start: start.to_rfc3339(),
        change_at: change.to_rfc3339(),
        observed_through: through.to_rfc3339(),
        baseline: crate::usage_app::statistics::population(&before)?,
        current: crate::usage_app::statistics::population(&after)?,
        baseline_usage,
        current_usage,
        undated_records: undated,
        partial: incomplete_usage
            || undated > 0
            || !snapshot.manifest.issues.is_empty()
            || snapshot
                .manifest
                .sources
                .iter()
                .any(|s| s.status != "complete"),
    }))
}
