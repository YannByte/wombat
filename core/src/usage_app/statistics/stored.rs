//! Exact task populations reduce persisted aggregates; operation bodies stay cold.
use super::*;
use crate::usage_app::working_set::{Order, WorkingSet};
use crate::usage_store::ledger_projection::{GroupKey, LedgerGroup, Scan};

struct Population {
    tasks: WorkingSet<Option<String>, Aggregate>,
    total: Aggregate,
}
impl Population {
    fn new() -> Result<Self> {
        Ok(Self {
            tasks: WorkingSet::new(Order::Names),
            total: Aggregate::new(&[])?,
        })
    }
    fn add(&mut self, thread: Option<String>, value: &Aggregate) -> Result<()> {
        self.total.combine(value)?;
        self.tasks
            .update(thread, || Aggregate::new(&[]), |task| task.combine(value))
    }
    fn finish(&self) -> Result<TaskPopulation> {
        let mut measured = 0;
        let mut totals = vec![];
        let mut sum = Some(0_u64);
        let mut unassigned = Aggregate::new(&[])?.usage;
        self.tasks.each(None, |id, value| {
            if id.is_none() {
                unassigned = value.usage.clone();
                return Ok(());
            }
            measured += 1;
            if let Some(total) = value.usage.complete_token_total() {
                if totals.len() >= 500_000 {
                    return Err(operation_error(
                        "RESOURCE_LIMIT",
                        "Task quantiles exceed their 4 MiB working budget",
                    ));
                }
                totals.push(total as f64);
                sum = sum
                    .and_then(|n| n.checked_add(total))
                    .filter(|n| *n <= MAX_SAFE_INTEGER);
            }
            Ok(())
        })?;
        let (median_tokens, p90_tokens) = crate::statistics::population_quantiles(&mut totals);
        Ok(TaskPopulation {
            measured_tasks: measured,
            complete_tasks: totals.len(),
            incomplete_tasks: measured - totals.len(),
            complete_task_tokens: sum,
            mean_tokens: sum
                .filter(|_| !totals.is_empty())
                .map(|n| n as f64 / totals.len() as f64),
            median_tokens,
            p90_tokens,
            unassigned_usage: unassigned,
        })
    }
}
fn fields(key: &GroupKey) -> scope::Fields<'_> {
    scope::Fields {
        agent: &key.agent,
        source: &key.source,
        thread: key.thread.as_deref(),
        turn: key.turn.as_deref(),
        model: key.model.as_deref(),
        effort: key.effort.as_deref(),
    }
}
pub(super) fn execute(request: Request, snapshot: &Snapshot, tz: Tz) -> Result<Response> {
    let projects: BTreeMap<_, _> = snapshot
        .manifest
        .threads
        .iter()
        .map(|t| (t.thread.id.as_str(), t.thread.project.as_deref()))
        .collect();
    let mut range = request.scope.clone();
    range.since = None;
    range.until = None;
    let baseline_scope = match &request.comparison {
        Some(ComparisonRequest::Periods {
            baseline_since,
            baseline_until,
            ..
        }) => {
            let mut scope = request.scope.clone();
            scope.since = Some(baseline_since.clone());
            scope.until = Some(baseline_until.clone());
            Some(scope)
        }
        _ => None,
    };
    let mut current = Population::new()?;
    let mut baseline = Population::new()?;
    let mut groups = WorkingSet::<(Option<String>, Option<String>), Aggregate>::new(Order::Groups);
    let (mut first, mut last) = (None, None);
    let selected = |key: &GroupKey, scope: &Scope, date| {
        scope::matches_fields(fields(key), scope, &projects, || date)
    };
    let context_filter = crate::usage_store::ledger_projection::Filter::scope(&range, &projects);
    let ambiguous = |group: &LedgerGroup| group.single_date(tz).is_none();
    snapshot.visit_ledger_filtered(
        tz,
        None,
        None,
        Scan::new(&context_filter, Some(&ambiguous)),
        |group, read| {
            let mut observe = |key: &GroupKey, date: Option<NaiveDate>| {
                if selected(key, &range, date)
                    && let Some(date) = date
                {
                    first = Some(first.map_or(date, |d: NaiveDate| d.min(date)));
                    last = Some(last.map_or(date, |d: NaiveDate| d.max(date)));
                }
            };
            if let Some(date) = group.single_date(tz) {
                observe(&group.key, date);
            } else {
                for row in read()? {
                    observe(&group.key, local_date(&row, tz));
                }
            }
            Ok(())
        },
    )?;
    let mut consume = |key: &GroupKey, value: &Aggregate, date: Option<NaiveDate>| -> Result<()> {
        if selected(key, &request.scope, date) {
            current.add(key.thread.clone(), value)?;
            if let Some(dimension) = &request.presentation {
                let dimension = match dimension {
                    Presentation::Projects => key
                        .thread
                        .as_deref()
                        .and_then(|id| projects.get(id))
                        .copied()
                        .flatten()
                        .map(str::to_owned),
                    Presentation::Models => key.model.clone(),
                    _ => unreachable!(),
                };
                groups.update(
                    (dimension, key.thread.clone()),
                    || Aggregate::new(&[]),
                    |total| total.combine(value),
                )?;
            }
        }
        if baseline_scope
            .as_ref()
            .is_some_and(|scope| selected(key, scope, date))
        {
            baseline.add(key.thread.clone(), value)?;
        }
        Ok(())
    };
    let select = |group: &LedgerGroup| {
        group.single_date(tz).is_none_or(|date| {
            selected(&group.key, &request.scope, date)
                || baseline_scope
                    .as_ref()
                    .is_some_and(|scope| selected(&group.key, scope, date))
        })
    };
    let filter = crate::usage_store::ledger_projection::Filter::scope(&request.scope, &projects);
    let filter = match &baseline_scope {
        Some(scope) => filter.either(crate::usage_store::ledger_projection::Filter::scope(
            scope, &projects,
        )),
        None => filter,
    };
    snapshot.visit_ledger_filtered(
        tz,
        None,
        None,
        Scan::new(&filter, Some(&select)),
        |group, read| {
            if let Some(date) = group.single_date(tz) {
                consume(&group.key, &group.aggregate, date)
            } else {
                for row in read()? {
                    let mut key = group.key.clone();
                    key.turn = row.fact.turn_id.as_deref().map(str::to_owned);
                    consume(&key, &Aggregate::new(&[&row])?, local_date(&row, tz))?;
                }
                Ok(())
            }
        },
    )?;
    let population = current.finish()?;
    let selected_task = if let Some(id) = &request.thread_id {
        let task = current.tasks.get(&Some(id.clone()))?.ok_or_else(|| {
            operation_error(
                "NOT_FOUND",
                "Selected task has no measurements in this population",
            )
        })?;
        let tokens = task.usage.complete_token_total();
        let (mut below, mut ties) = (0, 0);
        if let Some(selected) = tokens {
            current.tasks.each(None, |id, value| {
                if id.is_some()
                    && let Some(n) = value.usage.complete_token_total()
                {
                    below += usize::from(n < selected);
                    ties += usize::from(n == selected);
                }
                Ok(())
            })?;
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
    let growth = baseline_scope
        .map(|scope| {
            baseline
                .finish()
                .map(|base| super::growth(scope, base, &population))
        })
        .transpose()?;
    let offset = request.offset.unwrap_or(0);
    let limit = request.limit.unwrap_or(50);
    let mut total = 0;
    let mut result_groups = vec![];
    let mut active: Option<(Option<String>, Population)> = None;
    let mut finish = |key: Option<String>, pop: Population| -> Result<()> {
        if total >= offset && result_groups.len() < limit {
            result_groups.push(TaskStatisticsGroup {
                key,
                population: pop.finish()?,
            });
        }
        total += 1;
        Ok(())
    };
    groups.each(None, |(dimension, thread), value| {
        if active.as_ref().is_some_and(|(key, _)| key != dimension)
            && let Some((key, pop)) = active.take()
        {
            finish(key, pop)?;
        }
        if active.is_none() {
            active = Some((dimension.clone(), Population::new()?));
        }
        active.as_mut().unwrap().1.add(thread.clone(), value)
    })?;
    if let Some((key, pop)) = active {
        finish(key, pop)?;
    }
    let count = current.total.usage.measurement_count;
    let next = offset.saturating_add(result_groups.len());
    Ok(Response {
        output_version: 5,
        action: Action::Statistics,
        snapshot_ref: snapshot.manifest.snapshot_ref.clone(),
        available_range: AvailableRange {
            since: first.map(|d| d.to_string()),
            until: last.and_then(|d| d.succ_opt()).map(|d| d.to_string()),
        },
        summary: current.total.usage,
        scope: request.scope,
        items: vec![],
        quality: quality(snapshot, count),
        page: Page {
            offset,
            limit,
            total,
            next_offset: (next < total).then_some(next),
        },
        statistics: Some(TaskStatistics {
            selected_task,
            method_version: 1,
            quantile_method: "type_7".into(),
            population,
            dimension: request.presentation,
            groups: result_groups,
            growth,
        }),
        inspection: None,
        comparison: None,
        facets: None,
        distribution: None,
        price_update: None,
        freshness: None,
    })
}
