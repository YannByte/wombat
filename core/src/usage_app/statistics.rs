//! Aggregate ledger measurements once per selected population, without reading operation bodies.
use super::*;
mod stored;

pub(super) fn validate(request: &Request) -> Result<()> {
    if request.action != Action::Statistics {
        return Ok(());
    }
    if request
        .thread_id
        .as_ref()
        .is_some_and(|id| id.is_empty() || request.scope.thread_id.is_some())
    {
        return Err(invalid("任务分布的总体范围不能同时筛选单个任务"));
    }
    if request.group.is_some()
        || request.sort.is_some()
        || request.turn_id.is_some()
        || request.matched_only.is_some()
        || request
            .presentation
            .as_ref()
            .is_some_and(|p| !matches!(p, Presentation::Projects | Presentation::Models))
        || matches!(request.comparison, Some(ComparisonRequest::Sessions { .. }))
    {
        return Err(invalid(
            "任务统计只接受范围、项目或模型维度、分页及等长周期比较",
        ));
    }
    Ok(())
}

pub(crate) fn population(rows: &[&PricedMeasurement]) -> Result<TaskPopulation> {
    let mut tasks = BTreeMap::<&str, Vec<&PricedMeasurement>>::new();
    let mut unassigned = vec![];
    for row in rows {
        if let Some(id) = row.fact.thread_id.as_deref() {
            tasks.entry(id).or_default().push(*row);
        } else {
            unassigned.push(*row);
        }
    }
    let mut totals = Vec::with_capacity(tasks.len());
    let mut sum = Some(0_u64);
    for task in tasks.values() {
        if let Some(total) = super::summary::complete_total(task)? {
            totals.push(total as f64);
            sum = sum
                .and_then(|s| s.checked_add(total))
                .filter(|s| *s <= MAX_SAFE_INTEGER);
        }
    }
    totals.sort_unstable_by(f64::total_cmp);
    Ok(TaskPopulation {
        measured_tasks: tasks.len(),
        complete_tasks: totals.len(),
        incomplete_tasks: tasks.len() - totals.len(),
        complete_task_tokens: sum,
        mean_tokens: sum
            .filter(|_| !totals.is_empty())
            .map(|s| s as f64 / totals.len() as f64),
        median_tokens: crate::statistics::quantile(&totals, 0.5),
        p90_tokens: crate::statistics::quantile(&totals, 0.9),
        unassigned_usage: summarize(&unassigned)?,
    })
}

fn growth(baseline_scope: Scope, baseline: TaskPopulation, current: &TaskPopulation) -> TaskGrowth {
    let delta_n = current.measured_tasks as i64 - baseline.measured_tasks as i64;
    let complete = baseline.incomplete_tasks == 0 && current.incomplete_tasks == 0;
    let means = baseline
        .mean_tokens
        .zip(current.mean_tokens)
        .filter(|_| complete);
    TaskGrowth {
        baseline_scope,
        task_count_delta: delta_n,
        mean_tokens_delta: means.map(|(a, b)| b - a),
        task_count_contribution: means.map(|(a, b)| delta_n as f64 * (a + b) / 2.0),
        per_task_contribution: means.map(|(a, b)| {
            (b - a) * (baseline.measured_tasks + current.measured_tasks) as f64 / 2.0
        }),
        attributed_token_delta: baseline
            .complete_task_tokens
            .zip(current.complete_task_tokens)
            .filter(|_| complete)
            .map(|(a, b)| b as i64 - a as i64),
        unassigned_token_delta: baseline
            .unassigned_usage
            .complete_token_total()
            .zip(current.unassigned_usage.complete_token_total())
            .map(|(a, b)| b as i64 - a as i64),
        baseline,
    }
}

pub(super) fn execute(mut request: Request, snapshot: &Snapshot) -> Result<Response> {
    let tz = timezone(&request.scope)?;
    request.scope.timezone = Some(tz.to_string());
    if request.scope.since.is_none()
        && request.scope.until.is_none()
        && request.scope.all_time != Some(true)
        && request.scope.thread_id.is_none()
        && request.scope.undated != Some(true)
    {
        let today = Utc::now().with_timezone(&tz).date_naive();
        request.scope.since = Some(default_report_start(today, &Group::Day)?.to_string());
        request.scope.until = today.succ_opt().map(|d| d.to_string());
    }
    if snapshot.has_stored_ledger() {
        return stored::execute(request, snapshot, tz);
    }
    let live = snapshot.live_scope_ledger(&request.scope);
    let owned = if live.is_some() {
        vec![]
    } else {
        snapshot.ledger()?
    };
    let all = live.unwrap_or_else(|| owned.iter().collect());
    let projects: BTreeMap<_, _> = snapshot
        .manifest
        .threads
        .iter()
        .map(|t| (t.thread.id.as_str(), t.thread.project.as_deref()))
        .collect();
    let select = |scope: &Scope| {
        all.iter()
            .copied()
            .filter(|r| matches(r, scope, tz, &projects))
            .collect::<Vec<_>>()
    };
    let rows = select(&request.scope);
    let population = population(&rows)?;
    let selected_task = if let Some(id) = &request.thread_id {
        let selected = rows
            .iter()
            .copied()
            .filter(|r| r.fact.thread_id.as_deref() == Some(id.as_str()))
            .collect::<Vec<_>>();
        if selected.is_empty() {
            return Err(operation_error(
                "NOT_FOUND",
                "Selected task has no measurements in this population",
            ));
        }
        let tokens = super::summary::complete_total(&selected)?;
        let mut tasks = BTreeMap::<&str, Vec<&PricedMeasurement>>::new();
        for row in &rows {
            if let Some(id) = row.fact.thread_id.as_deref() {
                tasks.entry(id).or_default().push(*row);
            }
        }
        let mut below = 0;
        let mut ties = 0;
        if let Some(selected) = tokens {
            for task in tasks.values() {
                if let Some(total) = super::summary::complete_total(task)? {
                    below += usize::from(total < selected);
                    ties += usize::from(total == selected);
                }
            }
        }
        Some(TaskTypicality {
            thread_id: id.clone(),
            tokens,
            complete_population_tasks: population.complete_tasks,
            percentile_rank: tokens
                .filter(|_| population.complete_tasks > 0)
                .map(|_| (below as f64 + ties as f64 / 2.0) / population.complete_tasks as f64),
            above_p90: tokens.zip(population.p90_tokens).map(|(n, p)| n as f64 > p),
        })
    } else {
        None
    };
    let growth = if let Some(ComparisonRequest::Periods {
        baseline_since,
        baseline_until,
        ..
    }) = &request.comparison
    {
        let mut scope = request.scope.clone();
        scope.since = Some(baseline_since.clone());
        scope.until = Some(baseline_until.clone());
        Some(growth(
            scope.clone(),
            self::population(&select(&scope))?,
            &population,
        ))
    } else {
        None
    };
    let mut grouped = BTreeMap::<Option<String>, Vec<&PricedMeasurement>>::new();
    if let Some(dimension) = &request.presentation {
        for row in &rows {
            let key = match dimension {
                Presentation::Projects => row
                    .fact
                    .thread_id
                    .as_deref()
                    .and_then(|id| projects.get(id))
                    .copied()
                    .flatten(),
                Presentation::Models => row.fact.model.raw.as_deref(),
                _ => unreachable!(),
            }
            .map(str::to_owned);
            grouped.entry(key).or_default().push(*row);
        }
    }
    let total = grouped.len();
    let offset = request.offset.unwrap_or(0);
    let limit = request.limit.unwrap_or(50);
    let groups = grouped
        .into_iter()
        .skip(offset)
        .take(limit)
        .map(|(key, rows)| {
            Ok(TaskStatisticsGroup {
                key,
                population: self::population(&rows)?,
            })
        })
        .collect::<Result<Vec<_>>>()?;
    let next = offset.saturating_add(groups.len());
    let mut range_scope = request.scope.clone();
    range_scope.since = None;
    range_scope.until = None;
    Ok(Response {
        statistics: Some(TaskStatistics {
            selected_task,
            method_version: 1,
            quantile_method: "type_7".into(),
            population,
            dimension: request.presentation,
            groups,
            growth,
        }),
        inspection: None,
        comparison: None,
        facets: None,
        distribution: None,
        price_update: None,
        freshness: None,
        output_version: 5,
        action: Action::Statistics,
        snapshot_ref: snapshot.manifest.snapshot_ref.clone(),
        available_range: available(&select(&range_scope), tz),
        summary: summarize(&rows)?,
        scope: request.scope,
        items: vec![],
        quality: quality(snapshot, rows.len()),
        page: Page {
            offset,
            limit,
            total,
            next_offset: (next < total).then_some(next),
        },
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn ledger_population_counts_tasks_not_records_and_pages_only_groups() {
        let snapshot = super::super::inspection::tests::snapshot(
            &[
                ("1", "a", "2026-10-01T00:00:00Z", 100),
                ("2", "a", "2026-10-01T01:00:00Z", 50),
                ("3", "b", "2026-10-01T02:00:00Z", 0),
                ("4", "c", "2026-10-01T03:00:00Z", 300),
            ],
            "statistics",
        );
        let request=serde_json::from_value(serde_json::json!({"action":"statistics","scope":{"allTime":true},"presentation":"projects","limit":1,"offset":1})).unwrap();
        let response = execute(request, &snapshot).unwrap();
        let stats = response.statistics.unwrap();
        assert_eq!(stats.population.measured_tasks, 3);
        assert_eq!(stats.population.complete_tasks, 3);
        assert_eq!(stats.population.complete_task_tokens, Some(450));
        assert_eq!(stats.population.mean_tokens, Some(150.0));
        assert_eq!(stats.population.median_tokens, Some(150.0));
        assert_eq!(stats.population.p90_tokens, Some(270.0));
        assert_eq!(response.page.total, 3);
        assert_eq!(response.page.next_offset, Some(2));
        assert_eq!(stats.groups.len(), 1);
        assert_eq!(stats.groups[0].key.as_deref(), Some("/b"));
    }
    #[test]
    fn selected_population_rank() {
        let snapshot = super::super::inspection::tests::snapshot(
            &[
                ("1", "a", "2026-10-01T00:00:00Z", 100),
                ("2", "b", "2026-10-01T00:00:00Z", 100),
                ("3", "c", "2026-10-01T00:00:00Z", 300),
            ],
            "rank",
        );
        let request = serde_json::from_value(serde_json::json!({"action":"statistics","threadId":"c","scope":{"allTime":true},"presentation":"projects","limit":1})).unwrap();
        let response = execute(request, &snapshot).unwrap();
        let stats = response.statistics.unwrap();
        assert_eq!(stats.population.measured_tasks, 3);
        let selected = stats.selected_task.unwrap();
        assert_eq!(selected.tokens, Some(300));
        assert_eq!(selected.percentile_rank, Some(5.0 / 6.0));
        assert_eq!(selected.above_p90, Some(true));
        let missing = serde_json::from_value(
            serde_json::json!({"action":"statistics","threadId":"absent","scope":{"allTime":true}}),
        )
        .unwrap();
        assert!(execute(missing, &snapshot).is_err());
    }
    #[test]
    fn partial_and_unassigned_rows_are_separate_from_complete_task_quantiles() {
        let snapshot = super::super::inspection::tests::snapshot(
            &[
                ("1", "a", "2026-10-01T00:00:00Z", 100),
                ("2", "b", "2026-10-01T00:00:00Z", 200),
            ],
            "partial-statistics",
        );
        let mut rows = snapshot.ledger().unwrap();
        let fact = std::sync::Arc::make_mut(&mut rows[1].fact);
        fact.tokens.total = None;
        fact.tokens.raw_input = None;
        let result = population(&rows.iter().collect::<Vec<_>>()).unwrap();
        assert_eq!(result.measured_tasks, 2);
        assert_eq!(result.incomplete_tasks, 1);
        assert_eq!(result.mean_tokens, Some(100.0));
        let fact = std::sync::Arc::make_mut(&mut rows[0].fact);
        fact.thread_id = None;
        let result = population(&rows.iter().collect::<Vec<_>>()).unwrap();
        assert_eq!(result.measured_tasks, 1);
        assert_eq!(result.complete_tasks, 0);
        assert_eq!(result.mean_tokens, None);
        assert_eq!(result.unassigned_usage.complete_token_total(), Some(100));
    }
    #[test]
    fn invalid_statistics_queries_fail_before_reading_a_snapshot() {
        for value in [
            serde_json::json!({"action":"statistics","presentation":"details"}),
            serde_json::json!({"action":"statistics","comparison":{"kind":"sessions","leftThreadId":"a","rightThreadId":"b"}}),
            serde_json::json!({"action":"statistics","scope":{"since":"2026-10-01","until":"2026-10-03"},"comparison":{"kind":"periods","baselineSince":"2026-09-30","baselineUntil":"2026-10-01","dimension":"thread"}}),
        ] {
            let request: Request = serde_json::from_value(value).unwrap();
            assert!(super::super::validate(&request).is_err());
        }
    }
    fn pop(n: usize, total: u64) -> TaskPopulation {
        TaskPopulation {
            measured_tasks: n,
            complete_tasks: n,
            incomplete_tasks: 0,
            complete_task_tokens: Some(total),
            mean_tokens: (n > 0).then(|| total as f64 / n as f64),
            median_tokens: None,
            p90_tokens: None,
            unassigned_usage: summarize(&[]).unwrap(),
        }
    }
    #[test]
    fn growth_reconciles_count_and_intensity_in_both_directions() {
        for (a, b) in [
            (pop(2, 200), pop(4, 600)),
            (pop(4, 600), pop(2, 200)),
            (pop(3, 900), pop(3, 600)),
        ] {
            let g = growth(Scope::default(), a, &b);
            assert_eq!(
                g.task_count_contribution.unwrap() + g.per_task_contribution.unwrap(),
                g.attributed_token_delta.unwrap() as f64
            );
        }
    }
    #[test]
    fn empty_or_partial_populations_do_not_invent_means_or_contributions() {
        let g = growth(Scope::default(), pop(0, 0), &pop(3, 600));
        assert_eq!(g.attributed_token_delta, Some(600));
        assert!(g.task_count_contribution.is_none());
        let mut partial = pop(3, 600);
        partial.incomplete_tasks = 1;
        let g = growth(Scope::default(), pop(2, 200), &partial);
        assert!(g.attributed_token_delta.is_none());
        assert!(g.mean_tokens_delta.is_none());
    }
}
