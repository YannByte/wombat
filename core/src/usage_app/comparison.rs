//! Compare exact scopes from one immutable snapshot; never refresh while calculating.
use super::*;

pub(super) fn delta(before: &UsageSummary, after: &UsageSummary) -> UsageDelta {
    let totals = before
        .complete_token_total()
        .zip(after.complete_token_total());
    let cost = |s: &UsageSummary| {
        if s.measurement_count == 0 {
            Some(Decimal::ZERO)
        } else if s.price.status.as_ref() == "priced" {
            s.price
                .cost
                .as_ref()
                .and_then(|v| v.parse::<Decimal>().ok())
        } else {
            None
        }
    };
    UsageDelta {
        tokens: totals.map(|(a, b)| b as i64 - a as i64),
        cost: cost(before)
            .zip(cost(after))
            .map(|(a, b)| (b - a).normalize().to_string()),
        token_ratio: totals
            .filter(|(a, _)| *a > 0)
            .map(|(a, b)| (b as f64 - a as f64) / a as f64),
    }
}

pub(super) fn validate_comparison(request: &Request) -> Result<()> {
    if request.action == Action::Statistics {
        if request.comparison.is_none() {
            return Ok(());
        }
        let mut comparison = request.clone();
        comparison.action = Action::Compare;
        comparison.presentation = None;
        comparison.thread_id = None;
        return validate_comparison(&comparison);
    }
    if request.action != Action::Compare {
        if request.comparison.is_some() {
            return Err(invalid("comparison仅适用于compare"));
        }
        return Ok(());
    }
    if request.group.is_some()
        || request.presentation.is_some()
        || request.search.is_some()
        || request.thread_id.is_some()
        || request.turn_id.is_some()
        || request.sort.is_some()
        || request.locate_thread_id.is_some()
        || request.locate_turn_id.is_some()
        || request.matched_only.is_some()
    {
        return Err(invalid("比较不接受分组、排序或定位参数"));
    }
    match request
        .comparison
        .as_ref()
        .ok_or_else(|| invalid("比较需要comparison"))?
    {
        ComparisonRequest::Periods {
            baseline_since,
            baseline_until,
            ..
        } => {
            if request.scope.all_time == Some(true) || request.scope.undated == Some(true) {
                return Err(invalid("周期比较需要明确日期范围"));
            }
            let a = date(baseline_since)?;
            let b = date(baseline_until)?;
            let c = date(
                request
                    .scope
                    .since
                    .as_deref()
                    .ok_or_else(|| invalid("周期比较需要since"))?,
            )?;
            let d = date(
                request
                    .scope
                    .until
                    .as_deref()
                    .ok_or_else(|| invalid("周期比较需要until"))?,
            )?;
            if a >= b || c >= d || b > c || (b - a).num_days() != (d - c).num_days() {
                return Err(invalid("比较周期须等长且基准周期结束不晚于当前周期开始"));
            }
        }
        ComparisonRequest::Sessions {
            left_thread_id,
            right_thread_id,
            ..
        } => {
            if left_thread_id.is_empty()
                || right_thread_id.is_empty()
                || left_thread_id == right_thread_id
                || request.scope.thread_id.is_some()
            {
                return Err(invalid("会话比较需要两个不同的会话，不能同时筛选单个会话"));
            }
            if request.offset.is_some() || request.limit.is_some() {
                return Err(invalid("会话比较不接受分页参数"));
            }
        }
    }
    Ok(())
}

pub(super) fn execute(request: Request, snapshot: &Snapshot) -> Result<Response> {
    if snapshot
        .project_loads
        .iter()
        .any(|p| p.state != crate::live::ProjectLoadState::Ready)
    {
        return Err(operation_error(
            "SYNC_PENDING",
            "比较需要等待当前视图加载完成",
        ));
    }
    let tz = timezone(&request.scope)?;
    let mut base_scope = request.scope.clone();
    base_scope.timezone = Some(tz.to_string());
    let owned = snapshot.ledger()?;
    let rows: Vec<_> = owned.iter().collect();
    let projects: BTreeMap<_, _> = snapshot
        .manifest
        .threads
        .iter()
        .map(|t| (t.thread.id.as_str(), t.thread.project.as_deref()))
        .collect();
    let quality = quality(snapshot, rows.len());
    let partial = quality.status == "partial";
    let mut page = Page {
        offset: 0,
        limit: request.limit.unwrap_or(20),
        total: 0,
        next_offset: None,
    };
    let (comparison, summary) = match request.comparison.as_ref().expect("validated comparison") {
        ComparisonRequest::Periods {
            baseline_since,
            baseline_until,
            dimension,
        } => {
            let mut baseline_scope = base_scope.clone();
            baseline_scope.since = Some(baseline_since.clone());
            baseline_scope.until = Some(baseline_until.clone());
            let collect = |scope: &Scope| {
                rows.iter()
                    .copied()
                    .filter(|r| matches(r, scope, tz, &projects))
                    .collect::<Vec<_>>()
            };
            let before = collect(&baseline_scope);
            let after = collect(&base_scope);
            let period_partial = |scope: &Scope| {
                partial
                    || scope.until.as_ref().is_some_and(|end| {
                        *end > DateTime::parse_from_rfc3339(
                            &snapshot.manifest.snapshot_ref.created_at,
                        )
                        .map(|t| t.with_timezone(&tz).date_naive().to_string())
                        .unwrap_or_default()
                    })
            };
            let compared = |scope: Scope, rows: &[&PricedMeasurement]| -> Result<ComparedUsage> {
                Ok(ComparedUsage {
                    partial: period_partial(&scope),
                    usage: summarize(rows)?,
                    scope,
                })
            };
            let baseline = compared(baseline_scope.clone(), &before)?;
            let current = compared(base_scope.clone(), &after)?;
            let key = |row: &PricedMeasurement| -> Option<String> {
                match dimension {
                    DriverDimension::Project => row
                        .fact
                        .thread_id
                        .as_deref()
                        .and_then(|id| projects.get(id))
                        .copied()
                        .flatten()
                        .map(str::to_owned),
                    DriverDimension::Model => row.fact.model.raw.as_deref().map(str::to_owned),
                    DriverDimension::Thread => row.fact.thread_id.as_deref().map(str::to_owned),
                }
            };
            let mut groups: BTreeMap<
                Option<String>,
                (Vec<&PricedMeasurement>, Vec<&PricedMeasurement>),
            > = BTreeMap::new();
            for row in &before {
                groups.entry(key(row)).or_default().0.push(row);
            }
            for row in &after {
                groups.entry(key(row)).or_default().1.push(row);
            }
            let scoped = |mut scope: Scope, key: &Option<String>| {
                match dimension {
                    DriverDimension::Project => {
                        scope.project = key.clone();
                        scope.project_unknown = key.is_none().then_some(true);
                    }
                    DriverDimension::Model => {
                        scope.model = key.clone();
                        scope.model_unknown = key.is_none().then_some(true);
                    }
                    DriverDimension::Thread => {
                        scope.thread_id = key.clone();
                    }
                }
                scope
            };
            let mut drivers = Vec::with_capacity(groups.len());
            for (key, (a, b)) in &groups {
                let baseline = compared(scoped(baseline_scope.clone(), key), a)?;
                let current = compared(scoped(base_scope.clone(), key), b)?;
                drivers.push(UsageDriver {
                    key: key.clone(),
                    delta: delta(&baseline.usage, &current.usage),
                    baseline,
                    current,
                });
            }
            drivers.sort_by(|a, b| {
                b.delta
                    .tokens
                    .map(i64::unsigned_abs)
                    .cmp(&a.delta.tokens.map(i64::unsigned_abs))
                    .then(a.key.cmp(&b.key))
            });
            page.offset = request.offset.unwrap_or(0);
            page.total = drivers.len();
            let end = page.offset.saturating_add(page.limit).min(drivers.len());
            page.next_offset = (end < drivers.len()).then_some(end);
            let selected_keys: BTreeSet<_> = drivers
                .iter()
                .skip(page.offset)
                .take(page.limit)
                .map(|d| d.key.clone())
                .collect();
            let rest_before = before
                .iter()
                .copied()
                .filter(|r| !selected_keys.contains(&key(r)))
                .collect::<Vec<_>>();
            let rest_after = after
                .iter()
                .copied()
                .filter(|r| !selected_keys.contains(&key(r)))
                .collect::<Vec<_>>();
            let remaining = delta(&summarize(&rest_before)?, &summarize(&rest_after)?);
            let drivers = drivers
                .into_iter()
                .skip(page.offset)
                .take(page.limit)
                .collect();
            let mut timeless = base_scope.clone();
            timeless.all_time = None;
            timeless.since = None;
            timeless.until = None;
            timeless.undated = Some(true);
            let undated_records = collect(&timeless).len();
            let change = delta(&baseline.usage, &current.usage);
            let summary = current.usage.clone();
            (
                Comparison::Periods {
                    dimension: dimension.clone(),
                    baseline: Box::new(baseline),
                    current: Box::new(current),
                    delta: change,
                    drivers,
                    remaining,
                    undated_records,
                },
                summary,
            )
        }
        ComparisonRequest::Sessions {
            left_thread_id,
            right_thread_id,
            include_descendants,
        } => {
            let events = if *include_descendants {
                snapshot.events()?
            } else {
                vec![]
            };
            let mut parents = std::collections::HashMap::new();
            let mut conflicts = BTreeSet::new();
            for event in &events {
                if let crate::session_events::Payload::Ancestry { parent_id, .. } = event.payload()
                    && let Some(child) = event.thread_id()
                {
                    if parents.get(child).is_some_and(|prior| prior != parent_id) {
                        conflicts.insert(child.to_owned());
                    } else {
                        parents.insert(child.to_owned(), parent_id.clone());
                    }
                }
            }
            // A self-cycle invalidates both the conflicting child and its descendants.
            for child in &conflicts {
                parents.insert(child.clone(), child.clone());
            }
            let relations = crate::session_relations::SessionRelations::new_cancellable(
                &parents,
                &std::sync::atomic::AtomicBool::new(false),
            )?;
            let session = |id: &str| -> Result<ComparedSession> {
                let thread = snapshot
                    .manifest
                    .threads
                    .iter()
                    .find(|t| t.thread.id == id)
                    .ok_or_else(|| operation_error("NOT_FOUND", "未找到比较会话"))?;
                let members: BTreeSet<_> = snapshot
                    .manifest
                    .threads
                    .iter()
                    .filter(|t| {
                        t.thread.source_instance_id == thread.thread.source_instance_id
                            && (t.thread.id == id
                                || *include_descendants
                                    && relations.is_descendant(&t.thread.id, id))
                    })
                    .map(|t| t.thread.id.as_str())
                    .collect();
                let selected = rows
                    .iter()
                    .copied()
                    .filter(|r| {
                        matches(r, &base_scope, tz, &projects)
                            && r.fact
                                .thread_id
                                .as_deref()
                                .is_some_and(|t| members.contains(t))
                    })
                    .collect::<Vec<_>>();
                let own = selected
                    .iter()
                    .copied()
                    .filter(|r| r.fact.thread_id.as_deref() == Some(id))
                    .collect::<Vec<_>>();
                let descendants = selected
                    .iter()
                    .copied()
                    .filter(|r| r.fact.thread_id.as_deref() != Some(id))
                    .collect::<Vec<_>>();
                let mut scope = base_scope.clone();
                scope.thread_id = Some(id.to_owned());
                Ok(ComparedSession {
                    thread_id: id.to_owned(),
                    title: thread.thread.title.clone(),
                    own: summarize(&own)?,
                    descendants: summarize(&descendants)?,
                    selected: summarize(&selected)?,
                    member_count: members.len(),
                    partial: partial
                        || *include_descendants
                            && (relations.unresolved > 0
                                || parents.values().any(|p| !projects.contains_key(p.as_str()))),
                    scope,
                })
            };
            let left = session(left_thread_id)?;
            let right = session(right_thread_id)?;
            let change = delta(&left.selected, &right.selected);
            let summary = right.selected.clone();
            (
                Comparison::Sessions {
                    left: Box::new(left),
                    right: Box::new(right),
                    delta: change,
                    include_descendants: *include_descendants,
                },
                summary,
            )
        }
    };
    Ok(Response {
        statistics: None,
        inspection: None,
        comparison: Some(comparison),
        facets: None,
        distribution: None,
        price_update: None,
        freshness: None,
        output_version: 5,
        action: Action::Compare,
        snapshot_ref: snapshot.manifest.snapshot_ref.clone(),
        scope: base_scope,
        available_range: available(&rows, tz),
        summary,
        items: vec![],
        page,
        quality,
    })
}

pub(crate) fn publication_change(
    before: &Snapshot,
    after: &Snapshot,
    db: Option<(&rusqlite::Connection, &UsageSummary)>,
) -> Result<PublicationChange> {
    let counts = db
        .map(|(db, _)| before.measurement_changes(db, &after.manifest.sources))
        .transpose()?
        .flatten();
    let collect =
        |snapshot: &Snapshot| -> Result<(BTreeMap<String, String>, Result<UsageSummary>)> {
            let mut identities = BTreeMap::new();
            let mut total = Aggregate::new(&[])?;
            let mut failed = None;
            if snapshot.has_stored_ledger() {
                snapshot.visit_ledger_groups(chrono_tz::UTC, None, None, |group, read| {
                    if failed.is_none()
                        && let Err(error) = total.combine(&group.aggregate)
                    {
                        failed = Some(error);
                    }
                    if counts.is_none() {
                        for row in read()? {
                            identities.insert(
                                row.fact.id.clone(),
                                crate::hash(serde_json::to_vec(&row.fact)?),
                            );
                        }
                    }
                    Ok(())
                })?;
            } else {
                let rows = snapshot.ledger()?;
                for row in &rows {
                    if counts.is_none() {
                        identities.insert(
                            row.fact.id.clone(),
                            crate::hash(serde_json::to_vec(&row.fact)?),
                        );
                    }
                }
                failed = summarize(&rows.iter().collect::<Vec<_>>())
                    .map(|usage| total.usage = usage)
                    .err();
            }
            Ok((identities, failed.map_or_else(|| Ok(total.usage), Err)))
        };
    let (old, left) = collect(before)?;
    let (new, right) = match db {
        Some((db, summary)) => (
            if counts.is_none() {
                crate::usage_store::ledger_projection::identity_digests(
                    db,
                    &after.manifest.sources,
                )?
            } else {
                BTreeMap::new()
            },
            Ok(summary.clone()),
        ),
        None => collect(after)?,
    };
    let threads: BTreeSet<_> = before
        .manifest
        .threads
        .iter()
        .map(|t| t.thread.id.as_str())
        .collect();
    let turns = |s: &Snapshot| {
        s.manifest
            .threads
            .iter()
            .flat_map(|t| {
                t.turns
                    .iter()
                    .map(move |(id, turn)| ((t.thread.id.clone(), id.clone()), turn.turn.clone()))
            })
            .collect::<BTreeMap<_, _>>()
    };
    let old_turns = turns(before);
    let new_turns = turns(after);
    let coverage = |s: &Snapshot| serde_json::json!({"sources":s.manifest.sources.iter().map(|r|(&r.source.id,&r.status,r.issues.iter().map(|i|&i.code).collect::<BTreeSet<_>>())).collect::<Vec<_>>(),"watermarks":s.manifest.watermarks.iter().map(|w|(&w.file_id,&w.state,&w.issue_codes)).collect::<Vec<_>>()});
    let aggregate_delta = match left.and_then(|left| right.map(|right| delta(&left, &right))) {
        Ok(value) => value,
        Err(error)
            if error
                .downcast_ref::<crate::dto::OperationError>()
                .is_some_and(|e| e.code == "RESOURCE_LIMIT") =>
        {
            UsageDelta {
                tokens: None,
                cost: None,
                token_ratio: None,
            }
        }
        Err(error) => return Err(error),
    };
    Ok(PublicationChange {
        method_version: 1,
        baseline: before.manifest.snapshot_ref.clone(),
        current: after.manifest.snapshot_ref.clone(),
        measurements_added: counts.map_or_else(
            || new.keys().filter(|id| !old.contains_key(*id)).count(),
            |c| c.0,
        ),
        measurements_removed: counts.map_or_else(
            || old.keys().filter(|id| !new.contains_key(*id)).count(),
            |c| c.1,
        ),
        measurements_changed: counts.map_or_else(
            || {
                new.iter()
                    .filter(|(id, row)| old.get(*id).is_some_and(|prior| prior != *row))
                    .count()
            },
            |c| c.2,
        ),
        threads_added: after
            .manifest
            .threads
            .iter()
            .filter(|t| !threads.contains(t.thread.id.as_str()))
            .count(),
        turns_changed: new_turns
            .iter()
            .filter(|(id, turn)| old_turns.get(*id) != Some(*turn))
            .count(),
        prices_changed: before.manifest.price_catalog_hash != after.manifest.price_catalog_hash,
        coverage_changed: coverage(before) != coverage(after),
        delta: aggregate_delta,
    })
}

#[cfg(test)]
mod tests;
