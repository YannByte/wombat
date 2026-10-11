//! Fold committed sufficient statistics without materializing the canonical ledger.
//! A timezone boundary reads only the members of that bounded canonical frame.
use super::*;
use crate::usage_store::ledger_projection::GroupKey;
use bitvec::{order::Lsb0, vec::BitVec};
mod working_set;
use working_set::{Order, WorkingSet};

#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct Accumulator {
    value: Aggregate,
}
impl Accumulator {
    fn new() -> Result<Self> {
        Ok(Self {
            value: Aggregate::new(&[])?,
        })
    }
    fn add(&mut self, value: &Aggregate) -> Result<()> {
        self.value.combine(value)
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
#[derive(Clone, serde::Serialize, serde::Deserialize)]
struct Task {
    full: Accumulator,
    matched: Accumulator,
    models: BTreeSet<String>,
    efforts: BTreeSet<String>,
    latest: Option<String>,
    set_bytes: usize,
    matched_bits: Option<BitVec<u8, Lsb0>>,
}
impl Task {
    fn new() -> Result<Self> {
        Ok(Self {
            full: Accumulator::new()?,
            matched: Accumulator::new()?,
            models: BTreeSet::new(),
            efforts: BTreeSet::new(),
            latest: None,
            set_bytes: 0,
            matched_bits: None,
        })
    }
    fn include(&mut self, model: &Option<String>, effort: &Option<String>) -> Result<()> {
        for (set, value) in [(&mut self.models, model), (&mut self.efforts, effort)] {
            if let Some(value) = value
                && set.insert(value.clone())
            {
                self.set_bytes = self
                    .set_bytes
                    .checked_add(3 * value.len() + 256)
                    .filter(|b| *b <= 4 * 1024 * 1024)
                    .ok_or_else(|| {
                        operation_error(
                            "RESOURCE_LIMIT",
                            "Task dimensions exceed their working budget",
                        )
                    })?;
            }
        }
        Ok(())
    }
    fn combine(&mut self, other: &Self) -> Result<()> {
        if let Some(bits) = &other.matched_bits {
            if let Some(current) = &mut self.matched_bits {
                anyhow::ensure!(current.len() == bits.len(), "Task turn dictionary mismatch");
                *current |= bits.as_bitslice();
            } else {
                self.matched_bits = Some(bits.clone());
            }
        }
        self.full.add(&other.full.value)?;
        self.matched.add(&other.matched.value)?;
        for model in &other.models {
            self.include(&Some(model.clone()), &None)?;
        }
        for effort in &other.efforts {
            self.include(&None, &Some(effort.clone()))?;
        }
        self.latest = self
            .latest
            .take()
            .into_iter()
            .chain(other.latest.clone())
            .max();
        Ok(())
    }
}
type Period = Option<(i32, i32)>;
type Dimension = (Option<String>, Option<String>);

struct Items {
    values: Vec<Item>,
    bytes: usize,
}
impl Items {
    fn push(&mut self, item: Item) -> Result<()> {
        let bytes = serde_json::to_vec(&item)?
            .len()
            .checked_mul(3)
            .and_then(|b| b.checked_add(std::mem::size_of::<Item>() + 128))
            .ok_or_else(|| operation_error("RESOURCE_LIMIT", "Query result size overflow"))?;
        self.bytes = self
            .bytes
            .checked_add(bytes)
            .filter(|b| *b <= 48 * 1024 * 1024)
            .ok_or_else(|| {
                operation_error("RESOURCE_LIMIT", "Query result exceeds its working budget")
            })?;
        self.values.push(item);
        Ok(())
    }
}
pub(super) fn execute(mut request: Request, snapshot: &Snapshot) -> Result<Response> {
    let tz = timezone(&request.scope)?;
    request.scope.timezone = Some(tz.to_string());
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
    let projects: BTreeMap<_, _> = snapshot
        .manifest
        .threads
        .iter()
        .map(|t| (t.thread.id.as_str(), t.thread.project.as_deref()))
        .collect();
    let search_ids = request.search.as_ref().map(|q| {
        let q = q.to_lowercase();
        snapshot
            .manifest
            .threads
            .iter()
            .filter(|t| {
                [
                    Some(t.thread.id.as_str()),
                    Some(t.thread.upstream_id.as_str()),
                    t.thread.title.as_deref(),
                    t.thread.project.as_deref(),
                ]
                .into_iter()
                .flatten()
                .any(|s| s.to_lowercase().contains(&q))
            })
            .map(|t| t.thread.id.as_str())
            .collect::<BTreeSet<_>>()
    });
    let mut range_scope = request.scope.clone();
    range_scope.since = None;
    range_scope.until = None;
    let mut first = None;
    let mut last = None;
    let mut total = Accumulator::new()?;
    let mut periods = WorkingSet::<Period, Accumulator>::new(Order::Period);
    let mut details = WorkingSet::<(Period, Dimension), Accumulator>::new(Order::Dimensions);
    let mut dimensions_by_key = WorkingSet::<Option<String>, Accumulator>::new(Order::Names);
    let mut tasks = WorkingSet::<String, Task>::new(Order::Names);
    let thread_entries: BTreeMap<_, _> = snapshot
        .manifest
        .threads
        .iter()
        .map(|entry| (entry.thread.id.as_str(), entry))
        .collect();
    let mut current_turn_keys: Option<Vec<&str>> = None;
    let mut current_task: Option<(String, Task)> = None;
    let mut matched_turns = WorkingSet::<(String, String), u8>::new(Order::Names);
    let mut models = BTreeSet::new();
    let mut efforts = BTreeSet::new();
    let mut agents = BTreeSet::new();
    let mut unassigned = false;
    let dimensional = matches!(
        request.presentation,
        Some(Presentation::Projects | Presentation::Models)
    );
    let mut consume = |key: &GroupKey,
                       aggregate: &Aggregate,
                       date: Option<NaiveDate>,
                       latest: &Option<String>,
                       turns: &[Option<&str>]|
     -> Result<()> {
        let project = key
            .thread
            .as_deref()
            .and_then(|id| projects.get(id))
            .copied()
            .flatten();
        if request
            .scope
            .project
            .as_deref()
            .is_none_or(|p| project == Some(p))
            && (request.scope.project_unknown != Some(true) || project.is_none())
        {
            if let Some(model) = &key.model {
                models.insert(model.clone());
            }
            if let Some(effort) = &key.effort {
                efforts.insert(effort.clone());
            }
            agents.insert(key.agent.clone());
            unassigned |= project.is_none();
        }
        if scope::matches_fields(fields(key), &range_scope, &projects, || date)
            && let Some(date) = date
        {
            first = Some(first.map_or(date, |d: NaiveDate| d.min(date)));
            last = Some(last.map_or(date, |d: NaiveDate| d.max(date)));
        }
        let selected = scope::matches_fields(fields(key), &request.scope, &projects, || date)
            && search_ids
                .as_ref()
                .is_none_or(|ids| key.thread.as_deref().is_some_and(|id| ids.contains(id)));
        if request.action == Action::Threads
            && let Some(id) = &key.thread
        {
            if current_task
                .as_ref()
                .is_some_and(|(current, _)| current != id)
            {
                let (previous, value) = current_task.take().unwrap();
                tasks.update(previous, Task::new, |task| task.combine(&value))?;
            }
            if current_task.is_none() {
                let mut task = Task::new()?;
                current_turn_keys = thread_entries
                    .get(id.as_str())
                    .filter(|entry| {
                        entry
                            .turns
                            .len()
                            .saturating_mul(std::mem::size_of::<&str>())
                            <= 4 * 1024 * 1024
                    })
                    .map(|entry| entry.turns.keys().map(String::as_str).collect());
                if let Some(keys) = &current_turn_keys {
                    task.matched_bits = Some(BitVec::repeat(false, keys.len()));
                }
                current_task = Some((id.clone(), task));
            }
            let task = &mut current_task.as_mut().unwrap().1;
            task.full.add(aggregate)?;
            task.include(&key.model, &key.effort)?;
            if selected {
                task.matched.add(aggregate)?;
                task.latest = task.latest.take().into_iter().chain(latest.clone()).max();
            }
            if selected {
                for turn in turns.iter().flatten() {
                    if let (Some(keys), Some(bits)) = (&current_turn_keys, &mut task.matched_bits) {
                        let position = keys.binary_search(turn).map_err(|_| {
                            operation_error(
                                "INDEX_CORRUPT",
                                "Aggregate turn missing from its fixed dictionary",
                            )
                        })?;
                        bits.set(position, true);
                    } else {
                        matched_turns.insert((id.clone(), (*turn).to_owned()))?;
                    }
                }
            }
        }
        if !selected {
            return Ok(());
        }
        total.add(aggregate)?;
        if request.action == Action::Usage {
            if dimensional {
                let dimension = if request.presentation == Some(Presentation::Projects) {
                    project.map(str::to_owned)
                } else {
                    key.model.clone()
                };
                dimensions_by_key.update(dimension, Accumulator::new, |a| a.add(aggregate))?;
            } else {
                let period = date
                    .map(|d| reports::bucket(d, request.group.as_ref().unwrap_or(&Group::Day)))
                    .transpose()?
                    .map(|(a, b)| (a.num_days_from_ce(), b.num_days_from_ce()));
                periods.update(period, Accumulator::new, |a| a.add(aggregate))?;
                if request.presentation != Some(Presentation::Distribution) {
                    details.update(
                        (period, (key.model.clone(), key.effort.clone())),
                        Accumulator::new,
                        |a| a.add(aggregate),
                    )?;
                }
            }
        }
        Ok(())
    };
    snapshot.visit_ledger_groups(tz, None, None, |group, read| {
        if let Some(a) = group.single_date(tz) {
            consume(
                &group.key,
                &group.aggregate,
                a,
                &group.latest_text,
                &group
                    .members
                    .iter()
                    .map(|(id, _)| id.as_deref())
                    .collect::<Vec<_>>(),
            )
        } else {
            for row in read()? {
                let value = Aggregate::new(&[&row])?;
                consume(
                    &group.key,
                    &value,
                    local_date(&row, tz),
                    &row.fact.timestamp,
                    &[row.fact.turn_id.as_deref()],
                )?;
            }
            Ok(())
        }
    })?;
    if let Some((id, value)) = current_task {
        tasks.update(id, Task::new, |task| task.combine(&value))?;
    }
    let summary = total.value.usage;
    let count = summary.measurement_count;
    let available = AvailableRange {
        since: first.map(|d| d.to_string()),
        until: last.and_then(|d| d.succ_opt()).map(|d| d.to_string()),
    };
    let mut items = Items {
        values: vec![],
        bytes: 0,
    };
    if request.action == Action::Usage && dimensional {
        dimensions_by_key.each(None, |key, aggregate| {
            let mut subset = request.scope.clone();
            let by_project = request.presentation == Some(Presentation::Projects);
            if by_project {
                subset.project = key.clone();
                subset.project_unknown = key.is_none().then_some(true);
            } else {
                subset.model = key.clone();
                subset.model_unknown = key.is_none().then_some(true);
            }
            let usage = aggregate.value.usage.clone();
            items.push(Item::Usage {
                date: None,
                end_date: None,
                is_subtotal: true,
                model: (!by_project).then_some(key.clone()).flatten(),
                reasoning_effort: None,
                share: share(usage.complete_token_total(), summary.complete_token_total()),
                cost_share: cost_share(&usage, &summary),
                usage,
                scope: subset,
            })?;
            Ok(())
        })?;
        items.values.sort_by(|a, b| {
            let Item::Usage { usage: a, .. } = a else {
                unreachable!()
            };
            let Item::Usage { usage: b, .. } = b else {
                unreachable!()
            };
            consumption_order(a, b, &request.sort)
        });
    } else if request.action == Action::Usage {
        periods.each(None, |period, subtotal| {
            let mut subset = request.scope.clone();
            if let Some((start, end)) = period {
                let start = NaiveDate::from_num_days_from_ce_opt(*start)
                    .ok_or_else(|| invalid("Invalid period start"))?
                    .to_string();
                let end = NaiveDate::from_num_days_from_ce_opt(*end)
                    .ok_or_else(|| invalid("Invalid period end"))?
                    .to_string();
                subset.since = Some(
                    subset
                        .since
                        .as_ref()
                        .map_or(start.clone(), |s| s.max(&start).clone()),
                );
                subset.until = Some(
                    subset
                        .until
                        .as_ref()
                        .map_or(end.clone(), |s| s.min(&end).clone()),
                );
            } else {
                subset.undated = Some(true);
                subset.since = None;
                subset.until = None;
            }
            let date = subset.since.clone();
            let end_date = subset
                .until
                .as_ref()
                .and_then(|d| scope::date(d).ok()?.pred_opt())
                .map(|d| d.to_string());
            items.push(Item::Usage {
                date: date.clone(),
                end_date: end_date.clone(),
                is_subtotal: true,
                model: None,
                reasoning_effort: None,
                usage: subtotal.value.usage.clone(),
                scope: subset.clone(),
                share: None,
                cost_share: None,
            })?;
            let prefix = serde_json::to_string(period)?;
            details.each(Some(&prefix), |(_, (model, effort)), aggregate| {
                let mut specific = subset.clone();
                specific.model = model.clone();
                specific.model_unknown = model.is_none().then_some(true);
                specific.reasoning_effort = effort.clone();
                specific.effort_unknown = effort.is_none().then_some(true);
                items.push(Item::Usage {
                    date: date.clone(),
                    end_date: end_date.clone(),
                    is_subtotal: false,
                    model: model.clone(),
                    reasoning_effort: effort.clone(),
                    usage: aggregate.value.usage.clone(),
                    scope: specific,
                    share: None,
                    cost_share: None,
                })?;
                Ok(())
            })?;
            Ok(())
        })?;
    } else {
        let scoped = request.scope.project_unknown == Some(true)
            || request.scope.since.is_some()
            || request.scope.until.is_some()
            || request.scope.model.is_some()
            || request.scope.reasoning_effort.is_some()
            || request.scope.model_unknown == Some(true)
            || request.scope.effort_unknown == Some(true)
            || request.scope.undated == Some(true);
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
                || request.scope.project_unknown == Some(true) && t.project.is_some()
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
                || search_ids
                    .as_ref()
                    .is_some_and(|ids| !ids.contains(t.id.as_str()))
            {
                continue;
            }
            let task = tasks.get(&t.id)?.map(Ok).unwrap_or_else(Task::new)?;
            if scoped && task.matched.value.usage.measurement_count == 0 {
                continue;
            }
            let matched_latest = task
                .latest
                .as_ref()
                .into_iter()
                .chain((!scoped).then_some(t.last_activity_at.as_ref()).flatten())
                .max()
                .cloned();
            let turn_count = task
                .matched_bits
                .as_ref()
                .map(|bits| Ok(bits.count_ones()))
                .unwrap_or_else(|| matched_turns.count(&t.id))?;
            items.push(Item::Thread {
                id: t.id.clone(),
                upstream_id: Some(t.upstream_id.clone()),
                agent_kind: t.agent_kind.clone(),
                source_instance_id: t.source_instance_id.clone(),
                title: t.title.clone(),
                project: t.project.clone(),
                started_at: t.started_at.clone(),
                last_activity_at: t.last_activity_at.clone(),
                matched_last_activity_at: matched_latest,
                models: task.models.into_iter().collect(),
                reasoning_efforts: task.efforts.into_iter().collect(),
                matched_turn_count: (turn_count > 0).then_some(turn_count),
                matched_usage: task.matched.value.usage,
                thread_usage: task.full.value.usage,
            })?;
        }
        items.values.sort_by(|a, b| {
            let Item::Thread {
                id: ai,
                matched_usage: au,
                matched_last_activity_at: at,
                ..
            } = a
            else {
                unreachable!()
            };
            let Item::Thread {
                id: bi,
                matched_usage: bu,
                matched_last_activity_at: bt,
                ..
            } = b
            else {
                unreachable!()
            };
            if request.sort == Some(Sort::Recent) {
                bt.cmp(at).then(ai.cmp(bi))
            } else {
                consumption_order(au, bu, &request.sort)
                    .then(bt.cmp(at))
                    .then(ai.cmp(bi))
            }
        });
    }
    let facets = (request.action == Action::Usage).then(|| Facets {
        discovered_thread_count: Some(snapshot.manifest.threads.len()),
        directories: projects
            .values()
            .filter_map(|p| p.map(str::to_owned))
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect(),
        has_unassigned: unassigned
            || snapshot
                .manifest
                .threads
                .iter()
                .any(|t| t.thread.project.is_none()),
        models: models.into_iter().collect(),
        reasoning_efforts: efforts.into_iter().collect(),
        agents: agents
            .into_iter()
            .chain(
                snapshot
                    .manifest
                    .threads
                    .iter()
                    .map(|t| t.thread.agent_kind.clone()),
            )
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect(),
    });
    finish_response(
        request,
        snapshot,
        summary,
        available,
        items.values,
        facets,
        count,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn turn_bitmap_unions_disjoint_segments_and_serializes_exactly() {
        let mut a = Task::new().unwrap();
        let mut b = Task::new().unwrap();
        let n = 100_000;
        a.matched_bits = Some(BitVec::repeat(false, n));
        b.matched_bits = Some(BitVec::repeat(false, n));
        let mut expected = BTreeSet::new();
        for i in (0..n).step_by(3) {
            a.matched_bits.as_mut().unwrap().set(i, true);
            expected.insert(i);
        }
        for i in (0..n).step_by(5) {
            b.matched_bits.as_mut().unwrap().set(i, true);
            expected.insert(i);
        }
        a.combine(&b).unwrap();
        assert_eq!(
            a.matched_bits.as_ref().unwrap().count_ones(),
            expected.len()
        );
        assert_eq!(a.matched_bits.as_ref().unwrap().as_raw_slice().len(), n / 8);
        let restored: Task = serde_json::from_slice(&serde_json::to_vec(&a).unwrap()).unwrap();
        assert_eq!(restored.matched_bits, a.matched_bits);
        b.matched_bits = Some(BitVec::repeat(false, n + 1));
        assert!(a.combine(&b).is_err());
    }
}
