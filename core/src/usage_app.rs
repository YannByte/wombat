//! Shared usage queries for CLI, Web and future hosts.
use crate::adapters::{self, contract::*};
use crate::dto::operation_error;
use crate::usage_app_dto::*;
use crate::usage_store::{self, PricedMeasurement, Snapshot};
use anyhow::Result;
use chrono::{DateTime, Datelike, Duration, Months, NaiveDate, Utc};
use chrono_tz::Tz;
use rust_decimal::{Decimal, prelude::ToPrimitive};
use serde_json::Value;
use std::collections::{BTreeMap, BTreeSet};

mod compact;
mod comparison;
mod conversations;
mod inspection;
pub(crate) use comparison::publication_change;
pub(crate) mod query_work;
#[cfg(test)]
mod report_tests;
mod reports;
pub(crate) mod scope;
pub(crate) mod statistics;
mod stored;
mod summary;
mod working_set;
use conversations::{step_items, thread_items, turn_items};
use reports::{default_report_start, dimension_items, usage_items};
pub(crate) use scope::validate;
use scope::{
    available, date, dimensions, ensure_ready, invalid, local_date, matches, quality, timezone,
};
pub(crate) use summary::Aggregate;
pub(crate) use summary::SummaryIndex;
pub use summary::summarize;
fn summarize_view(snapshot: &Snapshot, rows: &[&PricedMeasurement]) -> Result<UsageSummary> {
    snapshot.summary_index.summarize(rows)
}
use summary::{
    consumption_order, cost_share, known_cost, max_available_tokens, share, unpriced_tokens,
};
pub fn dispatch(args: &Value) -> Result<Value> {
    let request: Request =
        serde_json::from_value(args.clone()).map_err(|e| invalid(format!("无效用量请求：{e}")))?;
    Ok(serde_json::to_value(execute(request)?)?)
}
/// Refresh publishes a whole-source summary, not a report that is then discarded.
pub(crate) fn refresh_response(
    snapshot: &Snapshot,
    rows: &[&PricedMeasurement],
) -> Result<Response> {
    Ok(Response {
        statistics: None,
        inspection: None,
        comparison: None,
        facets: None,
        distribution: None,
        price_update: None,
        freshness: None,
        output_version: 5,
        action: Action::Refresh,
        snapshot_ref: snapshot.manifest.snapshot_ref.clone(),
        scope: Scope::default(),
        available_range: available(rows, chrono_tz::UTC),
        summary: summarize(rows)?,
        items: vec![],
        page: Page {
            offset: 0,
            limit: 50,
            total: 0,
            next_offset: None,
        },
        quality: quality(snapshot, rows.len()),
    })
}
pub fn execute(request: Request) -> Result<Response> {
    validate(&request)?;
    if request.action == Action::Refresh {
        let _lock = usage_store::RefreshLock::acquire()?;
        eprintln!("{}", serde_json::json!({"stage":"读取 Codex 日志"}));
        let collected = adapters::collect(
            &DiscoveryRequest {
                roots: request
                    .roots
                    .unwrap_or_default()
                    .into_iter()
                    .map(Into::into)
                    .collect(),
            },
            &RunContext::default(),
        );
        if collected.sources.iter().any(|s| s.status == "cancelled") {
            return Err(operation_error("CANCELLED", "更新已取消"));
        }
        let successes = collected
            .sources
            .iter()
            .any(|s| matches!(s.status.as_str(), "complete" | "partial"));
        if !successes
            && (!collected.issues.is_empty()
                || collected.sources.iter().any(|s| s.status == "failed"))
        {
            return Err(operation_error(
                "SOURCE_UNREADABLE",
                "没有可读取的来源，保留上次数据",
            ));
        }
        eprintln!("{}", serde_json::json!({"stage":"保存用量"}));
        let snapshot = usage_store::save(collected)?;
        let rows = snapshot.ledger()?;
        let selected = rows.iter().collect::<Vec<_>>();
        return refresh_response(&snapshot, &selected);
    }
    let snapshot = usage_store::load(request.snapshot_id.as_deref())?;
    execute_snapshot(request, &snapshot)
}

pub(crate) fn execute_snapshot(request: Request, snapshot: &Snapshot) -> Result<Response> {
    let _budget = query_work::Budget::enter();
    if !matches!(
        request.action,
        Action::Usage | Action::Threads | Action::Statistics
    ) && let Some(details) = snapshot.detailed_view()?
    {
        return execute_snapshot(request, &details);
    }
    ensure_ready(&request, snapshot)?;
    validate(&request)?;
    let mut window = snapshot
        .is_live()
        .then(|| crate::query_cache::PageWindow::for_request(&request))
        .flatten();
    let planned = window
        .as_ref()
        .map_or_else(|| request.clone(), |w| w.request(&request));
    let mut key = if snapshot.is_live() {
        // Default ranges roll over at midnight in the request timezone.
        Some(format!(
            "{}:{}",
            Utc::now()
                .with_timezone(&timezone(&request.scope)?)
                .date_naive(),
            serde_json::to_string(&planned)?
        ))
    } else {
        None
    };
    if let Some(key) = &key
        && let Some(result) = snapshot.query_cache.lock().unwrap().get(key)
    {
        return Ok(match &window {
            Some(window) => window.project(result),
            None => result,
        });
    }
    // Duplicate misses on one immutable view share the completed result. Cache
    if snapshot.expired_detail_lease() {
        return Err(operation_error(
            "VIEW_EXPIRED",
            "Live data lease expired; reacquire the view",
        ));
    }
    // hits above remain available while a different request is being reduced.
    let _work = query_work::lock(&snapshot.query_work)?;
    if let Some(key) = &key
        && let Some(result) = snapshot.query_cache.lock().unwrap().get(key)
    {
        return Ok(match &window {
            Some(window) => window.project(result),
            None => result,
        });
    }
    let _permit = query_work::acquire()?;
    let compact = request.compact == Some(true);
    let mut result = match execute_uncached(planned, snapshot) {
        Ok(result) => result,
        Err(error)
            if window.is_some()
                && error
                    .downcast_ref::<crate::dto::OperationError>()
                    .is_some_and(|e| matches!(e.code, "RESOURCE_LIMIT" | "OUTPUT_LIMIT")) =>
        {
            window = None;
            key = None;
            execute_uncached(request, snapshot)?
        }
        Err(error) => return Err(query_work::query_error(error)),
    };
    if compact {
        compact::apply(&mut result);
    }
    if let Some(key) = key
        && let Some(payload) = crate::query_cache::QueryCache::encode(&key, &result)
    {
        snapshot.query_cache.lock().unwrap().insert(key, payload);
    }
    Ok(match window {
        Some(window) => window.project(result),
        None => result,
    })
}

fn execute_uncached(mut request: Request, snapshot: &Snapshot) -> Result<Response> {
    if request.action == Action::Statistics {
        return statistics::execute(request, snapshot);
    }
    if request.action == Action::Compare {
        return comparison::execute(request, snapshot);
    }
    if inspection::is_action(&request.action) {
        return inspection::execute(request, snapshot);
    }
    if let Some(id) = &request.thread_id {
        if request.scope.thread_id.as_ref().is_some_and(|s| s != id) {
            return Err(invalid("对话筛选身份冲突"));
        }
        request.scope.thread_id = Some(id.clone());
    }
    if snapshot.has_stored_ledger() && matches!(request.action, Action::Usage | Action::Threads) {
        return stored::execute(request, snapshot);
    }
    let tz = timezone(&request.scope)?;
    request.scope.timezone = Some(tz.to_string());
    // Detail queries read only the requested thread/turn shard.
    let live_rows = if matches!(request.action, Action::Usage | Action::Threads) {
        snapshot.live_scope_ledger(&request.scope)
    } else if matches!(request.action, Action::Turns | Action::Steps) {
        snapshot.live_detail_rows(
            request.thread_id.as_deref().unwrap(),
            request.turn_id.as_deref(),
        )?
    } else {
        None
    };
    let owned_rows = if live_rows.is_some() {
        vec![]
    } else if snapshot.has_stored_ledger()
        && matches!(request.action, Action::Turns | Action::Steps)
    {
        snapshot.stored_measurements(request.thread_id.as_deref(), request.turn_id.as_deref())?
    } else if matches!(request.action, Action::Turns | Action::Steps)
        && snapshot.manifest.schema_version == 3
    {
        let id = request.thread_id.as_deref().unwrap();
        let thread = snapshot
            .manifest
            .threads
            .iter()
            .find(|t| t.thread.id == id)
            .ok_or_else(|| operation_error("NOT_FOUND", "未找到对话"))?;
        if request.action == Action::Steps {
            snapshot
                .turn(id, request.turn_id.as_deref().unwrap())?
                .measurements
        } else {
            let mut rows = vec![];
            for turn_id in thread.turns.keys() {
                rows.extend(snapshot.turn(id, turn_id)?.measurements);
            }
            rows
        }
    } else {
        snapshot.ledger()?
    };
    let rows = live_rows.unwrap_or_else(|| owned_rows.iter().collect());
    let projects = snapshot
        .manifest
        .threads
        .iter()
        .map(|t| (t.thread.id.as_str(), t.thread.project.as_deref()))
        .collect::<BTreeMap<_, _>>();
    let mut range_scope = request.scope.clone();
    range_scope.since = None;
    range_scope.until = None;
    let range_rows = rows
        .iter()
        .copied()
        .filter(|r| scope::matches_view(snapshot, r, &range_scope, tz, &projects))
        .collect::<Vec<_>>();
    let available_range = scope::available_view(snapshot, &range_rows, tz);
    // Resolve automatic report ranges here, shared by all entry points.
    if request.action == Action::Usage
        && request.scope.since.is_none()
        && request.scope.until.is_none()
        && request.scope.undated != Some(true)
        && request.scope.thread_id.is_none()
        && request.scope.all_time != Some(true)
    {
        let today = Utc::now().with_timezone(&tz).date_naive();
        request.scope.since = Some(
            default_report_start(today, request.group.as_ref().unwrap_or(&Group::Day))?.to_string(),
        );
        request.scope.until = today.succ_opt().map(|d| d.to_string());
    }
    let search_ids = request.search.as_ref().map(|q| {
        let needle = q.to_lowercase();
        snapshot
            .manifest
            .threads
            .iter()
            .filter(|t| {
                t.thread.id.to_lowercase().contains(&needle)
                    || t.thread.upstream_id.to_lowercase().contains(&needle)
                    || t.thread
                        .title
                        .as_deref()
                        .unwrap_or("")
                        .to_lowercase()
                        .contains(&needle)
                    || t.thread
                        .project
                        .as_deref()
                        .unwrap_or("")
                        .to_lowercase()
                        .contains(&needle)
            })
            .map(|t| t.thread.id.as_str())
            .collect::<BTreeSet<_>>()
    });
    let selected = rows
        .iter()
        .copied()
        .filter(|r| {
            scope::matches_view(snapshot, r, &request.scope, tz, &projects)
                && search_ids.as_ref().is_none_or(|ids| {
                    r.fact
                        .thread_id
                        .as_deref()
                        .is_some_and(|id| ids.contains(id))
                })
        })
        .collect::<Vec<_>>();
    let mut summary = summarize_view(snapshot, &selected)?;
    let dimensional = matches!(
        request.presentation,
        Some(Presentation::Projects | Presentation::Models)
    );
    let items = match request.action {
        Action::Usage if dimensional => {
            dimension_items(snapshot, &selected, &request, &projects, &summary)?
        }
        Action::Usage => usage_items(
            snapshot,
            &selected,
            &summary,
            &request.scope,
            request.group.as_ref().unwrap_or(&Group::Day),
            tz,
            request.presentation != Some(Presentation::Distribution),
        )?,
        Action::Threads => thread_items(snapshot, &rows, &selected, &request)?,
        Action::Turns => {
            let all = rows
                .iter()
                .copied()
                .filter(|r| r.fact.thread_id.as_deref() == request.thread_id.as_deref())
                .collect::<Vec<_>>();
            summary = summarize(&all)?;
            turn_items(snapshot, &all, &selected, &request, &summary)?
        }
        Action::Steps => {
            let id = request.thread_id.as_deref().unwrap();
            let data = snapshot.turn(id, request.turn_id.as_deref().unwrap())?;
            summary = summarize(&data.measurements.iter().collect::<Vec<_>>())?;
            step_items(
                data,
                &summary,
                &request,
                &selected.iter().map(|r| r.fact.id.as_str()).collect(),
            )?
        }
        Action::Refresh
        | Action::Compare
        | Action::Investigate
        | Action::Trajectory
        | Action::Resources
        | Action::Review
        | Action::Context
        | Action::Statistics => unreachable!(),
    };
    let facets = (request.action == Action::Usage).then(|| {
        let (models, reasoning_efforts) = dimensions(&rows);
        Facets {
            discovered_thread_count: Some(snapshot.manifest.threads.len()),
            directories: projects
                .values()
                .filter_map(|p| p.map(str::to_owned))
                .collect::<BTreeSet<_>>()
                .into_iter()
                .collect(),
            has_unassigned: snapshot
                .manifest
                .threads
                .iter()
                .any(|t| t.thread.project.is_none())
                || rows.iter().any(|r| {
                    r.fact
                        .thread_id
                        .as_deref()
                        .and_then(|id| projects.get(id))
                        .copied()
                        .flatten()
                        .is_none()
                }),
            models,
            reasoning_efforts,
            agents: rows
                .iter()
                .map(|r| r.fact.agent_kind.as_ref())
                .chain(
                    snapshot
                        .manifest
                        .threads
                        .iter()
                        .map(|t| t.thread.agent_kind.as_str()),
                )
                .collect::<BTreeSet<_>>()
                .into_iter()
                .map(str::to_owned)
                .collect(),
        }
    });
    finish_response(
        request,
        snapshot,
        summary,
        available_range,
        items,
        facets,
        selected.len(),
    )
}

pub(super) fn finish_response(
    request: Request,
    snapshot: &Snapshot,
    summary: UsageSummary,
    available_range: AvailableRange,
    mut items: Vec<Item>,
    facets: Option<Facets>,
    count: usize,
) -> Result<Response> {
    let dimensional = matches!(
        request.presentation,
        Some(Presentation::Projects | Presentation::Models)
    );
    let limit = request.limit.unwrap_or(50);
    let located_turn = request.locate_turn_id.as_ref().and_then(|id| {
        items
            .iter()
            .position(|item| matches!(item, Item::Turn { id: current, .. } if current == id))
    });
    let located_operation = if let Some(id) = &request.locate_operation_id {
        Some(
            items
                .iter()
                .position(|item| matches!(item,Item::Operation{id:current,..} if current==id))
                .ok_or_else(|| operation_error("NOT_FOUND", "所选固定轮次不存在此操作"))?,
        )
    } else {
        None
    };
    let offset = located_operation.or(located_turn).or_else(|| request.locate_thread_id.as_ref().and_then(|id| items.iter().position(|item| matches!(item, Item::Thread { id: current, upstream_id, .. } if current == id || upstream_id.as_ref() == Some(id))))).map(|index| index / limit * limit).unwrap_or(request.offset.unwrap_or(0));
    let mut period_count = None;
    let distribution = if request.action == Action::Usage && !dimensional {
        let mut groups: Vec<Vec<Item>> = vec![];
        for item in items {
            if matches!(
                item,
                Item::Usage {
                    is_subtotal: true,
                    ..
                }
            ) {
                groups.push(vec![]);
            }
            groups.last_mut().unwrap().push(item);
        }
        fn group_usage(group: &[Item]) -> &UsageSummary {
            match &group[0] {
                Item::Usage { usage, .. } => usage,
                _ => unreachable!(),
            }
        }
        if matches!(request.sort, Some(Sort::Tokens | Sort::Cost)) {
            groups.sort_by(|a, b| consumption_order(group_usage(a), group_usage(b), &request.sort));
        }
        let max_tokens = max_available_tokens(groups.iter().map(|g| group_usage(g)));
        let max_cost = groups
            .iter()
            .filter_map(|g| known_cost(group_usage(g)))
            .max();
        let mut stats = Distribution {
            unpriced_tokens: summary.unpriced_tokens,
            max_tokens,
            token_basis: TokenBasis::AnalyzedTotals,
            max_cost: max_cost.map(|v| v.to_string()),
            peak_token_dates: vec![],
            peak_cost_dates: vec![],
            peak_token_scopes: vec![],
            peak_cost_scopes: vec![],
        };
        if request.presentation.is_some() {
            period_count = Some(groups.len());
        }
        items = vec![];
        for (index, group) in groups.into_iter().enumerate() {
            for mut item in group {
                if let Item::Usage {
                    date,
                    scope,
                    is_subtotal,
                    usage,
                    share: ratio,
                    cost_share: money_ratio,
                    ..
                } = &mut item
                {
                    *ratio = share(usage.complete_token_total(), summary.complete_token_total());
                    *money_ratio = cost_share(usage, &summary);
                    if *is_subtotal {
                        if max_tokens.is_some() && usage.available_token_subtotal() == max_tokens {
                            stats.peak_token_dates.push(date.clone());
                            stats.peak_token_scopes.push(scope.clone());
                        }
                        if max_cost.is_some_and(|v| v > Decimal::ZERO)
                            && known_cost(usage) == max_cost
                        {
                            stats.peak_cost_dates.push(date.clone());
                            stats.peak_cost_scopes.push(scope.clone());
                        }
                    } else if request.presentation == Some(Presentation::Distribution) {
                        continue;
                    }
                }
                if period_count.is_none() || (index >= offset && index - offset < limit) {
                    items.push(item);
                }
            }
        }
        Some(stats)
    } else {
        None
    };
    let total = period_count.unwrap_or(items.len());
    if period_count.is_none() {
        items = items.into_iter().skip(offset).take(limit).collect();
    }
    let next = offset.saturating_add(if period_count.is_some() {
        total.saturating_sub(offset).min(limit)
    } else {
        items.len()
    });
    Ok(Response {
        statistics: None,
        inspection: None,
        comparison: None,
        facets,
        distribution,
        price_update: None,
        freshness: None,
        output_version: 5,
        action: request.action,
        snapshot_ref: snapshot.manifest.snapshot_ref.clone(),
        scope: request.scope,
        available_range,
        summary,
        items,
        page: Page {
            offset,
            limit,
            total,
            next_offset: (next < total).then_some(next),
        },
        quality: if request.compact == Some(true) {
            compact::quality(snapshot, count)
        } else {
            quality(snapshot, count)
        },
    })
}

#[cfg(test)]
mod page_cache_tests;
