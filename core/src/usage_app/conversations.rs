//! Thread, turn and step queries preserve their native identities and ledger attribution.
use super::*;
#[derive(serde::Serialize)]
pub(super) struct ThreadSummary {
    pub usage: UsageSummary,
    pub models: Vec<String>,
    pub reasoning_efforts: Vec<String>,
    pub turns: Option<usize>,
    pub latest: Option<String>,
}
pub(super) fn prepare_thread(rows: &[&PricedMeasurement]) -> Result<ThreadSummary> {
    let (models, reasoning_efforts) = dimensions(rows);
    let turns = rows
        .iter()
        .filter_map(|r| r.fact.turn_id.as_deref())
        .collect::<BTreeSet<_>>()
        .len();
    Ok(ThreadSummary {
        usage: summarize(rows)?,
        models,
        reasoning_efforts,
        turns: (turns > 0).then_some(turns),
        latest: rows
            .iter()
            .filter_map(|r| r.fact.timestamp.as_ref())
            .max()
            .cloned(),
    })
}
pub(super) fn group_measurements<'a>(
    rows: &[&'a PricedMeasurement],
    key: impl Fn(&'a PricedMeasurement) -> Option<&'a str>,
) -> std::collections::HashMap<&'a str, Vec<&'a PricedMeasurement>> {
    let mut grouped = std::collections::HashMap::<_, Vec<_>>::new();
    for &row in rows {
        if let Some(id) = key(row) {
            grouped.entry(id).or_default().push(row);
        }
    }
    grouped
}
pub(super) fn thread_items(
    snapshot: &Snapshot,
    rows: &[&PricedMeasurement],
    selected: &[&PricedMeasurement],
    request: &Request,
) -> Result<Vec<Item>> {
    let search = request.search.as_deref().unwrap_or("").to_lowercase();
    let scoped = request.scope.project_unknown == Some(true)
        || request.scope.since.is_some()
        || request.scope.until.is_some()
        || request.scope.model.is_some()
        || request.scope.reasoning_effort.is_some()
        || request.scope.model_unknown == Some(true)
        || request.scope.effort_unknown == Some(true)
        || request.scope.undated == Some(true);
    let full_by_thread = group_measurements(rows, |r| r.fact.thread_id.as_deref());
    let same = rows.len() == selected.len()
        && rows.iter().zip(selected).all(|(a, b)| std::ptr::eq(*a, *b));
    let matched_groups =
        (!same).then(|| group_measurements(selected, |r| r.fact.thread_id.as_deref()));
    let matched_by_thread = matched_groups.as_ref().unwrap_or(&full_by_thread);
    let mut result = vec![];
    for entry in &snapshot.manifest.threads {
        let t = &entry.thread;
        if request
            .scope
            .thread_id
            .as_ref()
            .is_some_and(|id| *id != t.id)
            || request
                .scope
                .project
                .as_ref()
                .is_some_and(|p| t.project.as_ref() != Some(p))
            || request
                .scope
                .agent_kind
                .as_ref()
                .is_some_and(|a| *a != t.agent_kind)
            || request
                .scope
                .source_instance_id
                .as_ref()
                .is_some_and(|s| *s != t.source_instance_id)
        {
            continue;
        }
        if request.scope.project_unknown == Some(true) && t.project.is_some() {
            continue;
        }
        if !search.is_empty()
            && !t.id.to_lowercase().contains(&search)
            && !t.upstream_id.to_lowercase().contains(&search)
            && !t
                .title
                .as_deref()
                .unwrap_or("")
                .to_lowercase()
                .contains(&search)
            && !t
                .project
                .as_deref()
                .unwrap_or("")
                .to_lowercase()
                .contains(&search)
        {
            continue;
        }
        let matched = matched_by_thread
            .get(t.id.as_str())
            .map(Vec::as_slice)
            .unwrap_or_default();
        if scoped && matched.is_empty() {
            continue;
        }
        let full = full_by_thread
            .get(t.id.as_str())
            .map(Vec::as_slice)
            .unwrap_or_default();
        let prepared = snapshot
            .summary_index
            .thread(&t.id)
            .filter(|s| s.usage.measurement_count == full.len());
        let same = full.len() == matched.len()
            && full.iter().zip(matched).all(|(a, b)| std::ptr::eq(*a, *b));
        let (models, reasoning_efforts) = prepared.map_or_else(
            || dimensions(full),
            |s| (s.models.clone(), s.reasoning_efforts.clone()),
        );
        let thread_usage = match prepared {
            Some(s) => s.usage.clone(),
            None => summarize_view(snapshot, full)?,
        };
        let matched_usage = if same {
            thread_usage.clone()
        } else {
            summarize_view(snapshot, matched)?
        };
        let (matched_turn_count, latest) = if let Some(prepared) = prepared.filter(|_| same) {
            (prepared.turns, prepared.latest.clone())
        } else {
            let turns = matched
                .iter()
                .filter_map(|r| r.fact.turn_id.as_deref())
                .collect::<BTreeSet<_>>()
                .len();
            (
                (turns > 0).then_some(turns),
                matched
                    .iter()
                    .filter_map(|r| r.fact.timestamp.as_ref())
                    .max()
                    .cloned(),
            )
        };
        result.push(Item::Thread {
            upstream_id: Some(t.upstream_id.clone()),
            matched_last_activity_at: latest
                .as_ref()
                .into_iter()
                .chain((!scoped).then_some(t.last_activity_at.as_ref()).flatten())
                .max()
                .cloned(),
            id: t.id.clone(),
            agent_kind: t.agent_kind.clone(),
            source_instance_id: t.source_instance_id.clone(),
            title: t.title.clone(),
            project: t.project.clone(),
            started_at: t.started_at.clone(),
            last_activity_at: t.last_activity_at.clone(),
            models,
            reasoning_efforts,
            matched_turn_count,
            matched_usage,
            thread_usage,
        });
    }
    result.sort_by(|a, b| {
        let Item::Thread {
            id: aid,
            matched_usage: au,
            matched_last_activity_at: at,
            ..
        } = a
        else {
            unreachable!()
        };
        let Item::Thread {
            id: bid,
            matched_usage: bu,
            matched_last_activity_at: bt,
            ..
        } = b
        else {
            unreachable!()
        };
        if request.sort == Some(Sort::Recent) {
            bt.cmp(at).then(aid.cmp(bid))
        } else {
            consumption_order(au, bu, &request.sort)
                .then(bt.cmp(at))
                .then(aid.cmp(bid))
        }
    });
    Ok(result)
}
pub(super) fn turn_items(
    snapshot: &Snapshot,
    full: &[&PricedMeasurement],
    selected: &[&PricedMeasurement],
    request: &Request,
    total: &UsageSummary,
) -> Result<Vec<Item>> {
    let id = request.thread_id.as_deref().unwrap();
    let entry = snapshot
        .manifest
        .threads
        .iter()
        .find(|t| t.thread.id == id)
        .ok_or_else(|| operation_error("NOT_FOUND", "未找到对话"))?;
    let full_by_turn = group_measurements(full, |r| {
        Some(r.fact.turn_id.as_deref().unwrap_or("unassigned"))
    });
    let matched_by_turn = group_measurements(selected, |r| {
        Some(r.fact.turn_id.as_deref().unwrap_or("unassigned"))
    });
    let mut result = vec![];
    for (turn_id, turn) in &entry.turns {
        let current = full_by_turn
            .get(turn_id.as_str())
            .map(Vec::as_slice)
            .unwrap_or_default();
        let matched = matched_by_turn
            .get(turn_id.as_str())
            .map(Vec::as_slice)
            .unwrap_or_default();
        if request.matched_only == Some(true) && matched.is_empty() {
            continue;
        }
        let usage = summarize(current)?;
        let ratio = share(usage.complete_token_total(), total.complete_token_total());
        let (models, reasoning_efforts) = dimensions(current);
        result.push(Item::Turn {
            cost_share: cost_share(&usage, total),
            id: turn_id.clone(),
            thread_id: id.into(),
            ordinal: turn.turn.as_ref().map(|t| t.ordinal),
            started_at: turn.turn.as_ref().and_then(|t| t.started_at.clone()),
            ended_at: turn.turn.as_ref().and_then(|t| t.ended_at.clone()),
            last_activity_at: current
                .iter()
                .filter_map(|r| r.fact.timestamp.as_ref())
                .chain(turn.turn.as_ref().into_iter().flat_map(|t| {
                    [
                        t.started_at.as_ref(),
                        t.ended_at.as_ref(),
                        t.last_activity_at.as_ref(),
                    ]
                    .into_iter()
                    .flatten()
                }))
                .max()
                .cloned(),
            status: turn
                .turn
                .as_ref()
                .map(|t| t.status.clone())
                .unwrap_or_else(|| "unknown".into()),
            models,
            reasoning_efforts,
            usage,
            matched_usage: summarize(matched)?,
            share: ratio,
        });
    }
    result.sort_by(|a, b| {
        let Item::Turn {
            id: ai,
            usage: au,
            started_at: at,
            ordinal: ao,
            last_activity_at: al,
            ..
        } = a
        else {
            unreachable!()
        };
        let Item::Turn {
            id: bi,
            usage: bu,
            started_at: bt,
            ordinal: bo,
            last_activity_at: bl,
            ..
        } = b
        else {
            unreachable!()
        };
        if request.sort == Some(Sort::Recent) {
            bl.cmp(al).then(ai.cmp(bi))
        } else if request.sort == Some(Sort::Time) {
            at.cmp(bt).then(ao.cmp(bo)).then(ai.cmp(bi))
        } else {
            consumption_order(au, bu, &request.sort)
                .then(at.cmp(bt))
                .then(ai.cmp(bi))
        }
    });
    Ok(result)
}
pub(super) fn step_items(
    data: usage_store::TurnData,
    total: &UsageSummary,
    request: &Request,
    matched: &BTreeSet<&str>,
) -> Result<Vec<Item>> {
    let mut items = vec![];
    for row in data.measurements {
        let usage = summarize(&[&row])?;
        let r = std::sync::Arc::unwrap_or_clone(row.fact);
        items.push(Item::Measurement {
            matches_scope: matched.contains(r.id.as_str()),
            cost_share: cost_share(&usage, total),
            id: r.id,
            thread_id: r.thread_id.as_deref().map(str::to_owned),
            turn_id: r.turn_id.as_deref().map(str::to_owned),
            timestamp: r.timestamp,
            model: r.model.raw.as_deref().map(str::to_owned),
            reasoning_effort: r.reasoning_effort.as_deref().map(str::to_owned),
            share: share(usage.complete_token_total(), total.complete_token_total()),
            usage,
            sequence: r.sequence,
            time_precision: r.time_precision.to_string(),
        });
    }
    for op in data.operations {
        items.push(Item::Operation {
            id: op.id,
            thread_id: op.thread_id.to_string(),
            turn_id: op.turn_id.as_deref().map(str::to_owned),
            timestamp: op.timestamp,
            name: op.name.to_string(),
            status: op.status.to_string(),
            sequence: op.sequence,
            time_precision: op.time_precision.to_string(),
            operation_type: op.kind.to_string(),
            exit_code: op.exit_code,
            duration_ms: op.duration_ms,
            path: op.path,
            server: op.server.as_deref().map(str::to_owned),
            tool: op.tool.as_deref().map(str::to_owned),
        });
    }
    fn key(item: &Item) -> (&Option<String>, u64, &str) {
        match item {
            Item::Measurement {
                timestamp,
                sequence,
                id,
                ..
            }
            | Item::Operation {
                timestamp,
                sequence,
                id,
                ..
            } => (timestamp, *sequence, id),
            _ => unreachable!(),
        }
    }
    items.sort_by(|a, b| {
        if matches!(request.sort, Some(Sort::Tokens | Sort::Cost)) {
            match (a, b) {
                (Item::Measurement { usage: au, .. }, Item::Measurement { usage: bu, .. }) => {
                    return consumption_order(au, bu, &request.sort).then(key(a).cmp(&key(b)));
                }
                (Item::Measurement { .. }, _) => return std::cmp::Ordering::Less,
                (_, Item::Measurement { .. }) => return std::cmp::Ordering::Greater,
                _ => {}
            }
        }
        key(a).cmp(&key(b))
    });
    Ok(items)
}
