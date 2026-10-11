//! Historical configuration observations; current inventory is only a target index.
use super::*;

pub(super) struct HistoricalEvidence {
    pub evidence: Vec<Evidence>,
    pub uncertain_items: BTreeSet<String>,
}

pub(super) fn project_history(
    r: &Request,
    scope: &Scope,
    tz: Tz,
    view: &View,
    items: &mut [Item],
    result: &mut Response,
) -> Result<HistoricalEvidence> {
    let mut evidence = vec![];
    let mut uncertain_items = BTreeSet::new();
    let mut related = BTreeMap::<Option<String>, (usize, BTreeSet<usize>)>::new();
    if let Some(snapshot) = &view.snapshot {
        let threads = snapshot
            .manifest
            .threads
            .iter()
            .map(|t| (t.thread.id.as_str(), &t.thread))
            .collect::<BTreeMap<_, _>>();
        let projects = threads
            .iter()
            .map(|(id, t)| (*id, t.project.as_deref()))
            .collect::<BTreeMap<_, _>>();
        let owned_ledger = snapshot.ledger()?;
        let ledger = owned_ledger.iter().collect::<Vec<_>>();
        let mut turns = BTreeMap::<(&str, &str), Vec<usize>>::new();
        for (n, row) in ledger.iter().enumerate() {
            if let (Some(t), Some(u)) = (&row.fact.thread_id, &row.fact.turn_id) {
                turns.entry((t, u)).or_default().push(n);
            }
        }
        let mut paths = BTreeMap::<(String, String), Vec<usize>>::new();
        let mut servers = BTreeMap::<(String, String), Vec<usize>>::new();
        for (n, item) in items.iter().enumerate() {
            for source in item.source_ids() {
                if item.kind == Kind::Mcp {
                    servers
                        .entry((
                            source.to_owned(),
                            item.native_key.clone().unwrap_or_default(),
                        ))
                        .or_default()
                        .push(n);
                } else if let Some(path) = usage_observations::normalized_path(&item.path, None) {
                    paths.entry((source.to_owned(), path)).or_default().push(n);
                }
            }
        }
        let operations = snapshot.operation_facts()?;
        observe_available_skills(&operations, scope, &threads, &paths, items);
        let mut item_rows = vec![BTreeSet::<usize>::new(); items.len()];
        let mut item_turns = vec![BTreeSet::<String>::new(); items.len()];
        let mut item_tasks = vec![BTreeSet::<String>::new(); items.len()];
        let mut item_uses = (0..items.len())
            .map(|_| Projection::default())
            .collect::<Vec<_>>();
        let mut unassigned_sources = BTreeSet::<String>::new();
        let mut unassigned_skill_sources = BTreeSet::<String>::new();
        let mut unassigned_rule_sources = BTreeSet::<String>::new();
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
                if scope
                    .source_instance_id
                    .as_ref()
                    .is_some_and(|id| id != &thread.source_instance_id)
                    || scope.thread_id.as_ref().is_some_and(|id| id != &thread.id)
                {
                    return false;
                }
                if let Some(reads) = usage_observations::read_targets(op) {
                    (reads.unbound && !paths.is_empty())
                        || reads.paths.iter().any(|raw| {
                            reads
                                .resolve(raw, thread.project.as_deref())
                                .is_some_and(|path| {
                                    paths.contains_key(&(thread.source_instance_id.clone(), path))
                                })
                        })
                } else {
                    op.server.as_ref().is_some_and(|server| {
                        servers
                            .contains_key(&(thread.source_instance_id.clone(), server.to_string()))
                    })
                }
            },
        );
        let mut seen = BTreeSet::new();
        for op in operations.iter() {
            let Some(thread) = threads.get(op.thread_id.as_ref()) else {
                continue;
            };
            if scope
                .source_instance_id
                .as_ref()
                .is_some_and(|id| id != &thread.source_instance_id)
                || scope.thread_id.as_ref().is_some_and(|t| t != &thread.id)
            {
                continue;
            }
            let instruction_load = op.kind.as_ref() == "instructionLoad";
            let skill_available = op.kind.as_ref() == "skillAvailable";
            let use_kind = usage_observations::use_kind(op);
            let candidate_read = usage_observations::is_skill_read_candidate(op);
            let reads = usage_observations::read_targets(op);
            let file_read = instruction_load
                || op.kind.as_ref() == "skillRead"
                || (op.name.as_ref() == "read_file"
                    && matches!(op.kind.as_ref(), "tool" | "skillRead"));
            if matches!(op.kind.as_ref(), "mcpConflict" | "mcpUnclassified") {
                if scope
                    .project
                    .as_ref()
                    .is_none_or(|p| thread.project.as_ref() == Some(p))
                    && (in_time(op.timestamp.as_deref(), scope, tz)
                        || usage_observations::time_basis(op) == TimeBasis::Unknown)
                {
                    unassigned_sources.insert(thread.source_instance_id.clone());
                }
                continue;
            }
            let mut read_paths = BTreeSet::new();
            let mut unbound_read = false;
            if let Some(reads) = &reads {
                unbound_read = reads.unbound && (use_kind.is_some() || candidate_read);
                for raw in &reads.paths {
                    if let Some(path) = reads.resolve(raw, thread.project.as_deref()) {
                        read_paths.insert(path);
                    } else if usage_observations::is_skill_file(raw) {
                        unbound_read = true;
                    }
                }
            }
            if unbound_read
                && (in_time(op.timestamp.as_deref(), scope, tz)
                    || usage_observations::time_basis(op) == TimeBasis::Unknown)
                && scope
                    .project
                    .as_ref()
                    .is_none_or(|p| thread.project.as_ref() == Some(p))
            {
                unassigned_skill_sources.insert(thread.source_instance_id.clone());
            }
            if reads.as_ref().is_some_and(|reads| {
                reads.unbound
                    || reads
                        .paths
                        .iter()
                        .any(|raw| reads.resolve(raw, thread.project.as_deref()).is_none())
            }) && (in_time(op.timestamp.as_deref(), scope, tz)
                || usage_observations::time_basis(op) == TimeBasis::Unknown)
                && scope
                    .project
                    .as_ref()
                    .is_none_or(|project| thread.project.as_ref() == Some(project))
            {
                unassigned_rule_sources.insert(thread.source_instance_id.clone());
            }
            if read_paths.is_empty() {
                read_paths.insert(String::new());
            }
            for read_path in &read_paths {
                let candidates = if reads.is_some() {
                    paths.get(&(thread.source_instance_id.clone(), read_path.clone()))
                } else if file_read || skill_available {
                    normalized_operation_path(op.path.as_deref(), thread.project.as_deref())
                        .and_then(|path| paths.get(&(thread.source_instance_id.clone(), path)))
                } else if matches!(op.kind.as_ref(), "mcpTool" | "mcpResource") {
                    op.server.as_ref().and_then(|s| {
                        servers.get(&(thread.source_instance_id.clone(), s.to_string()))
                    })
                } else {
                    None
                };
                let Some(candidates) = candidates else {
                    if (use_kind.is_some() || candidate_read)
                        && (in_time(op.timestamp.as_deref(), scope, tz)
                            || usage_observations::time_basis(op) == TimeBasis::Unknown)
                        && scope
                            .project
                            .as_ref()
                            .is_none_or(|p| thread.project.as_ref() == Some(p))
                    {
                        if matches!(op.kind.as_ref(), "mcpTool" | "mcpResource")
                            && op.server.is_none()
                        {
                            unassigned_sources.insert(thread.source_instance_id.clone());
                        } else if file_read
                            && normalized_operation_path(
                                op.path.as_deref(),
                                thread.project.as_deref(),
                            )
                            .is_none()
                        {
                            unassigned_skill_sources.insert(thread.source_instance_id.clone());
                        }
                        result.coverage.issues.push(Issue {
                            code: "usageOperationTargetUnknown".into(),
                            path: None,
                        });
                    }
                    continue;
                };
                let candidates = candidates
                    .iter()
                    .copied()
                    .filter(|n| {
                        file_read
                            || skill_available
                            || items[*n]
                                .project
                                .as_ref()
                                .is_none_or(|p| thread.project.as_ref() == Some(p))
                    })
                    .collect::<Vec<_>>();
                if candidates.len() != 1 {
                    if (use_kind.is_some() || candidate_read)
                        && scope
                            .project
                            .as_ref()
                            .is_none_or(|p| thread.project.as_ref() == Some(p))
                        && (in_time(op.timestamp.as_deref(), scope, tz)
                            || usage_observations::time_basis(op) == TimeBasis::Unknown)
                    {
                        for index in candidates {
                            item_uses[index].coverage.target_gaps += 1;
                        }
                    }
                    continue;
                }
                let n = candidates[0];
                let projection_evidence =
                    !skill_available && (reads.is_some() || use_kind.is_some());
                if projection_evidence
                    && scope
                        .project
                        .as_ref()
                        .is_none_or(|project| thread.project.as_ref() == Some(project))
                    && usage_observations::time_basis(op) == TimeBasis::Unknown
                {
                    item_uses[n].note_time_gap(op);
                }
                if !in_time(op.timestamp.as_deref(), scope, tz) {
                    continue;
                }
                if let Some(identity) = usage_observations::operation_identity(op)
                    && !seen.insert((identity, n))
                {
                    continue;
                }
                if usage_observations::operation_identity(op)
                    .is_some_and(|identity| conflicts.contains(&identity))
                {
                    item_uses[n].coverage.target_gaps += 1;
                    continue;
                }
                let item = &mut items[n];
                if reads.as_ref().is_some_and(|reads| reads.candidate) {
                    if scope
                        .project
                        .as_ref()
                        .is_none_or(|p| thread.project.as_ref() == Some(p))
                    {
                        item_uses[n].coverage.dispatch_gaps += 1;
                        if item.kind == Kind::Rule {
                            result.coverage.issues.push(Issue {
                                code: "ruleReadDispatchUnknown".into(),
                                path: Some(item.path.clone()),
                            });
                        }
                        if r.item_id.as_ref() == Some(&item.id) {
                            evidence.push(Evidence {
                                id: op.id.clone(),
                                source_instance_id: Some(thread.source_instance_id.clone()),
                                item_id: item.id.clone(),
                                thread_id: thread.id.clone(),
                                turn_id: op.turn_id.as_deref().map(str::to_owned),
                                title: thread.title.clone(),
                                project: thread.project.clone(),
                                timestamp: op.timestamp.clone(),
                                event_type: if item.kind == Kind::Rule {
                                    "rule_read_candidate"
                                } else {
                                    "skill_read_candidate"
                                }
                                .into(),
                                outcome: "unknown".into(),
                                association: "operationDispatchUnknown".into(),
                                usage: None,
                            });
                        }
                    }
                    continue;
                }
                if skill_available {
                    if r.item_id.as_ref() == Some(&item.id) {
                        evidence.push(Evidence {
                            id: op.id.clone(),
                            source_instance_id: Some(thread.source_instance_id.clone()),
                            item_id: item.id.clone(),
                            thread_id: thread.id.clone(),
                            turn_id: op.turn_id.as_deref().map(str::to_owned),
                            title: thread.title.clone(),
                            project: thread.project.clone(),
                            timestamp: op.timestamp.clone(),
                            event_type: "skill_available".into(),
                            outcome: "observed".into(),
                            association: "hostAvailability".into(),
                            usage: None,
                        });
                    }
                    continue;
                }
                let observed_use =
                    use_kind.is_some() && matches!(item.kind, Kind::Skill | Kind::Mcp);
                let full_scope = Scope {
                    project: None,
                    ..scope.clone()
                };
                let indices = op
                    .turn_id
                    .as_deref()
                    .and_then(|u| turns.get(&(thread.id.as_str(), u)))
                    .into_iter()
                    .flatten()
                    .copied()
                    .filter(|i| matches_row(ledger[*i], &full_scope, tz, &projects))
                    .collect::<BTreeSet<_>>();
                if r.item_id.as_ref() == Some(&item.id) {
                    let entry = related.entry(thread.project.clone()).or_default();
                    entry.0 += 1;
                    entry.1.extend(&indices);
                }
                if scope
                    .project
                    .as_ref()
                    .is_some_and(|p| thread.project.as_ref() != Some(p))
                {
                    continue;
                }
                if op.timestamp > item.last_record_at {
                    item.last_record_at = op.timestamp.clone();
                }
                let typ = if item.kind == Kind::Mcp {
                    if op.kind.as_ref() == "mcpResource" {
                        item.counts.resource_reads += 1;
                    } else {
                        item.counts.tool_calls += 1;
                    }
                    item.observation = Observation::Used;
                    if op.kind.as_ref() == "mcpResource" {
                        "resource_read"
                    } else {
                        "tool_call"
                    }
                } else if item.kind == Kind::Skill {
                    if file_read {
                        item.counts.file_reads += 1;
                    }
                    if observed_use {
                        item.observation = Observation::Used;
                    }
                    "file_read"
                } else {
                    item.counts.file_reads += u64::from(
                        !instruction_load && usage_observations::operation_identity(op).is_some(),
                    );
                    if op.status.as_ref() == "completed" {
                        item.observation = Observation::LoadedOnly;
                    }
                    if instruction_load {
                        "instruction_load"
                    } else {
                        "file_read"
                    }
                };
                match op.status.as_ref() {
                    "completed" => item.counts.succeeded += 1,
                    "failed" => item.counts.failed += 1,
                    _ => item.counts.outcome_unknown += 1,
                };
                if let Some(context) = item
                    .source_contexts
                    .iter_mut()
                    .find(|c| c.source_instance_id == thread.source_instance_id)
                {
                    if op.timestamp > context.last_record_at {
                        context.last_record_at = op.timestamp.clone();
                    }
                    if item.kind == Kind::Mcp {
                        if op.kind.as_ref() == "mcpResource" {
                            context.counts.resource_reads += 1;
                        } else {
                            context.counts.tool_calls += 1;
                        }
                        context.observation = Observation::Used;
                    } else if item.kind == Kind::Skill {
                        if file_read {
                            context.counts.file_reads += 1;
                        }
                        if observed_use {
                            context.observation = Observation::Used;
                        }
                    } else {
                        context.counts.file_reads += u64::from(
                            !instruction_load
                                && usage_observations::operation_identity(op).is_some(),
                        );
                        if op.status.as_ref() == "completed" {
                            context.observation = Observation::LoadedOnly;
                        }
                    }
                    match op.status.as_ref() {
                        "completed" => context.counts.succeeded += 1,
                        "failed" => context.counts.failed += 1,
                        _ => context.counts.outcome_unknown += 1,
                    }
                }
                item_rows[n].extend(&indices);
                item_tasks[n].insert(thread.id.clone());
                if observed_use || item.kind == Kind::Rule && !instruction_load {
                    item_uses[n].observe_with_time_accounted(op);
                }
                if let Some(u) = &op.turn_id {
                    item_turns[n].insert(format!("{}:{u}", thread.id));
                }
                if r.item_id.as_ref() == Some(&item.id) {
                    evidence.push(Evidence {
                        id: op.id.clone(),
                        source_instance_id: Some(thread.source_instance_id.clone()),
                        item_id: item.id.clone(),
                        thread_id: thread.id.clone(),
                        turn_id: op.turn_id.as_deref().map(str::to_owned),
                        title: thread.title.clone(),
                        project: thread.project.clone(),
                        timestamp: op.timestamp.clone(),
                        event_type: typ.into(),
                        outcome: op.status.to_string(),
                        association: if op.text_result.as_ref().is_some_and(|r| {
                            !r.conflicting
                                && r.method_version == 1
                                && r.hash.as_deref() == Some(item.content_hash.as_str())
                        }) && reads
                            .as_ref()
                            .is_none_or(|reads| !reads.unbound && reads.paths.len() == 1)
                        {
                            if instruction_load {
                                "loadedContentMatchesCurrent"
                            } else if file_read {
                                "readContentMatchesCurrent"
                            } else {
                                "identityOnlyVersionUnknown"
                            }
                        } else {
                            "identityOnlyVersionUnknown"
                        }
                        .into(),
                        usage: usage(&indices.iter().map(|i| ledger[*i]).collect::<Vec<_>>())?,
                    });
                }
            }
        }
        let mut union = BTreeSet::<usize>::new();
        for (n, item) in items.iter_mut().enumerate() {
            item.related_turns = item_turns[n].len();
            item.related_tasks = item_tasks[n].len();
            if matches!(item.kind, Kind::Skill | Kind::Mcp) {
                let projection = &mut item_uses[n];
                if item.kind == Kind::Mcp
                    && item
                        .source_ids()
                        .any(|source| unassigned_sources.contains(source))
                {
                    projection.coverage.target_gaps += 1;
                }
                if item.kind == Kind::Skill
                    && item
                        .source_ids()
                        .any(|source| unassigned_skill_sources.contains(source))
                {
                    projection.coverage.target_gaps += 1;
                }
                item.usage_count = if snapshot.is_live()
                    && matches!(
                        view.history_status.as_str(),
                        "current" | "fixed" | "partial"
                    ) {
                    Some(projection.observed_count())
                } else {
                    None
                };
                item.related_turns = projection.related_turns();
                item.related_tasks = projection.related_tasks();
                for (gap, code) in [
                    (projection.coverage.dispatch_gaps, "DispatchUnknown"),
                    (projection.coverage.identity_gaps, "IdentityUnknown"),
                    (projection.coverage.target_gaps, "TargetUnknown"),
                    (projection.coverage.time_gaps, "TimeUnknown"),
                    (projection.coverage.turn_gaps, "TurnUnknown"),
                ] {
                    if gap > 0 {
                        uncertain_items.insert(item.id.clone());
                        result.coverage.issues.push(Issue {
                            code: format!(
                                "usageCountV{}{code}",
                                usage_observations::METHOD_VERSION
                            ),
                            path: Some(item.path.clone()),
                        });
                    }
                }
            }
            if item.kind == Kind::Rule
                && item
                    .source_ids()
                    .any(|source| unassigned_rule_sources.contains(source))
            {
                item_uses[n].coverage.target_gaps += 1;
            }
            if item.kind != Kind::Hook
                && snapshot.is_live()
                && matches!(
                    view.history_status.as_str(),
                    "current" | "fixed" | "partial"
                )
            {
                item.use_basis = Some(usage_observations::basis(
                    Some(&item_uses[n]),
                    if item.kind == Kind::Rule {
                        UseUnit::RuleLoadOrRead
                    } else {
                        UseUnit::ObjectUse
                    },
                    {
                        let mut selected = use_scope(item, scope);
                        if let Some(id) = &scope.thread_id {
                            selected.source_instance_ids.retain(|source| {
                                threads
                                    .get(id.as_str())
                                    .is_some_and(|thread| &thread.source_instance_id == source)
                            });
                        }
                        selected
                    },
                    &view.checked,
                    Some(snapshot),
                    scope.all_time != Some(true),
                ));
            }
            item.usage = usage(&item_rows[n].iter().map(|i| ledger[*i]).collect::<Vec<_>>())?;
            union.extend(item_rows[n].iter().copied());
        }
        result.summary.usage = usage(&union.into_iter().map(|i| ledger[i]).collect::<Vec<_>>())?;
        result.related_scopes = related
            .into_iter()
            .map(|(project, (evidence_count, ids))| {
                Ok(RelatedScope {
                    project,
                    evidence_count,
                    usage: usage(&ids.into_iter().map(|i| ledger[i]).collect::<Vec<_>>())?,
                })
            })
            .collect::<Result<_>>()?;
    }
    Ok(HistoricalEvidence {
        evidence,
        uncertain_items,
    })
}

fn observe_available_skills(
    operations: &[std::sync::Arc<crate::adapters::contract::Operation>],
    scope: &Scope,
    threads: &BTreeMap<&str, &crate::adapters::contract::Thread>,
    paths: &BTreeMap<(String, String), Vec<usize>>,
    items: &mut [Item],
) {
    // Availability is a current host observation, independent of the history date window.
    // Use only the newest catalog in the selected project for each source.
    let mut latest_catalogs = BTreeMap::<String, ((Option<String>, u64, String), String)>::new();
    for op in operations.iter() {
        if op.kind.as_ref() != "skillCatalog" {
            continue;
        }
        let Some(thread) = threads.get(op.thread_id.as_ref()) else {
            continue;
        };
        if scope
            .source_instance_id
            .as_ref()
            .is_some_and(|id| id != &thread.source_instance_id)
            || scope
                .project
                .as_ref()
                .is_some_and(|project| thread.project.as_ref() != Some(project))
        {
            continue;
        }
        let order = (op.timestamp.clone(), op.sequence, op.id.clone());
        let occurrence = catalog_occurrence(op);
        latest_catalogs
            .entry(thread.source_instance_id.clone())
            .and_modify(|current| {
                if order > current.0 {
                    *current = (order.clone(), occurrence.clone());
                }
            })
            .or_insert((order, occurrence));
    }
    for op in operations.iter() {
        if op.kind.as_ref() != "skillAvailable" {
            continue;
        }
        let Some(thread) = threads.get(op.thread_id.as_ref()) else {
            continue;
        };
        if latest_catalogs
            .get(&thread.source_instance_id)
            .is_none_or(|(_, latest)| latest != &catalog_occurrence(op))
        {
            continue;
        }
        let Some(path) = normalized_operation_path(op.path.as_deref(), thread.project.as_deref())
        else {
            continue;
        };
        let Some(candidates) = paths.get(&(thread.source_instance_id.clone(), path)) else {
            continue;
        };
        for index in candidates {
            if items[*index].kind != Kind::Skill {
                continue;
            }
            items[*index].configured_state = "enabled".into();
            if let Some(context) = items[*index]
                .source_contexts
                .iter_mut()
                .find(|context| context.source_instance_id == thread.source_instance_id)
            {
                context.configured_state = "enabled".into();
            }
        }
    }
}
