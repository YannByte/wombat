//! Read-only, bounded inspection features over canonical facts in one snapshot.
use super::*;
use crate::session_events::{ItemKind, LifecycleKind, MessageOrigin, Payload};
use crate::usage_store::EventReadBudget;
use std::sync::{Arc, atomic::AtomicBool};

const MAX_FACTS: usize = 250_000;
const MAX_WORK: usize = 100_000;
const MAX_EVENT_BYTES: u64 = 64 * 1024 * 1024;
const MAX_BYTES: usize = INSPECTION_RESPONSE_BYTES;
mod activity;
mod context;
mod opportunities;
mod rules;
pub(super) fn is_action(action: &Action) -> bool {
    matches!(
        action,
        Action::Investigate
            | Action::Trajectory
            | Action::Resources
            | Action::Review
            | Action::Context
    )
}
pub(super) fn validate(r: &Request) -> Result<()> {
    if !is_action(&r.action) {
        return Ok(());
    }
    if r.group.is_some()
        || r.sort.is_some()
        || r.presentation.is_some()
        || r.search.is_some()
        || r.turn_id.is_some()
        || r.locate_thread_id.is_some()
        || r.locate_turn_id.is_some()
        || r.matched_only.is_some()
    {
        return Err(invalid("检查查询不接受分组、排序或定位参数"));
    }
    if r.thread_id
        .as_ref()
        .zip(r.scope.thread_id.as_ref())
        .is_some_and(|(a, b)| a != b)
    {
        return Err(invalid("检查任务身份冲突"));
    }
    if r.action == Action::Trajectory
        && r.thread_id
            .as_ref()
            .or(r.scope.thread_id.as_ref())
            .is_none_or(|s| s.is_empty())
    {
        return Err(invalid("输入轨迹需要完整threadId"));
    }
    if r.action == Action::Review
        && (r.scope.all_time == Some(true)
            || r.scope.undated == Some(true)
            || r.offset.is_some()
            || r.limit.is_some())
    {
        return Err(invalid("周期复盘需要日期范围且不接受分页"));
    }
    if r.action == Action::Review && r.scope.since.is_some() != r.scope.until.is_some() {
        return Err(invalid("周期复盘须同时提供since和until"));
    }
    Ok(())
}
fn limit() -> anyhow::Error {
    operation_error("RESOURCE_LIMIT", "检查查询超过计算或输出预算，请缩小范围")
}
fn evidence(
    s: &Snapshot,
    scope: &Scope,
    thread: &str,
    turn: Option<&str>,
    operation: Option<&str>,
) -> InspectionEvidence {
    InspectionEvidence {
        view: if operation.is_some() {
            InspectionEvidenceView::Operation
        } else if turn.is_some() {
            InspectionEvidenceView::Turn
        } else {
            InspectionEvidenceView::Task
        },
        method_version: 1,
        snapshot_id: s.manifest.snapshot_ref.snapshot_id.clone(),
        scope: scope.clone(),
        thread_id: thread.into(),
        turn_id: turn.map(str::to_owned),
        operation_id: operation.map(str::to_owned),
    }
}
#[derive(Clone, PartialEq, Eq, PartialOrd, Ord)]
struct Order {
    file: String,
    generation: String,
    offset: u64,
    ordinal: u32,
    epoch: u64,
    context_epoch: u64,
    gap_epoch: u64,
}
#[derive(Default)]
struct EventWork {
    facts: usize,
    bytes: u64,
}
fn thread_events(
    s: &Snapshot,
    thread: &crate::usage_store::ThreadEntry,
    work: &mut EventWork,
) -> Result<Vec<Arc<crate::session_events::Event>>> {
    let mut events = Vec::new();
    for partition in s.manifest.events.partitions.iter().filter(|p| {
        p.target.thread_id.as_deref() == Some(thread.thread.id.as_str())
            || p.target.thread_id.is_none()
    }) {
        let bytes = partition.chunks.iter().try_fold(0_u64, |sum, chunk| {
            sum.checked_add(chunk.bytes).ok_or_else(limit)
        })?;
        let next_bytes = work.bytes.checked_add(bytes).ok_or_else(limit)?;
        let next_facts = work.facts.checked_add(partition.count).ok_or_else(limit)?;
        if next_bytes > MAX_EVENT_BYTES || next_facts > MAX_WORK {
            return Err(limit());
        }
        let batch = s.events_for_target(
            &partition.target,
            EventReadBudget {
                max_facts: MAX_WORK - work.facts,
                max_bytes: MAX_EVENT_BYTES - work.bytes,
            },
            &AtomicBool::new(false),
        )?;
        work.facts = next_facts;
        work.bytes = next_bytes;
        events.extend(batch.into_iter().filter(|e| {
            e.position().source_instance_id.as_ref() == thread.thread.source_instance_id.as_str()
                && (e.thread_id().is_some() || !e.gaps().is_empty())
        }));
    }
    sort_events(&mut events);
    Ok(events)
}
fn sort_events(events: &mut [Arc<crate::session_events::Event>]) {
    events.sort_by(|a, b| {
        let a = a.position();
        let b = b.position();
        (
            &a.source_instance_id,
            &a.file_id,
            &a.generation,
            a.byte_offset,
            a.ordinal,
        )
            .cmp(&(
                &b.source_instance_id,
                &b.file_id,
                &b.generation,
                b.byte_offset,
                b.ordinal,
            ))
    });
}
fn order_map(
    events: &[Arc<crate::session_events::Event>],
) -> Result<BTreeMap<String, Option<Order>>> {
    let mut epochs: BTreeMap<(String, String), u64> = BTreeMap::new();
    let mut out = BTreeMap::new();
    let mut context_epochs = BTreeMap::<(String, String), (u64, Option<String>)>::new();
    let mut gap_epochs = BTreeMap::<(String, String), u64>::new();
    let mut line_requests = BTreeMap::<(String, String, u64), BTreeSet<String>>::new();
    for event in events {
        let p = event.position();
        let domain = (p.file_id.to_string(), p.generation.to_string());
        let context = context_epochs.entry(domain.clone()).or_default();
        let gap = gap_epochs.entry(domain.clone()).or_default();
        if !event.gaps().is_empty() {
            *gap += 1;
        }
        if let Payload::ContextWindow { model, .. } = event.payload()
            && (model.is_none() || model != &context.1)
        {
            context.0 += 1;
            context.1 = model.clone();
        }
        let epoch = epochs
            .entry((p.file_id.to_string(), p.generation.to_string()))
            .or_default();
        if matches!(
            event.payload(),
            Payload::Lifecycle {
                lifecycle: LifecycleKind::Compaction,
                ..
            } | Payload::Message {
                origin: MessageOrigin::Compaction,
                ..
            } | Payload::Item {
                item_kind: ItemKind::Compaction,
                ..
            }
        ) {
            *epoch += 1;
        }
        if let Payload::Measurement { value, .. } = event.payload() {
            if value.request_scoped {
                line_requests
                    .entry((
                        p.file_id.to_string(),
                        p.generation.to_string(),
                        p.byte_offset,
                    ))
                    .or_default()
                    .insert(value.id.clone());
            }
            if value.request_scoped && context.1 != value.model.raw.as_deref().map(str::to_owned) {
                context.0 += 1;
                context.1 = value.model.raw.as_deref().map(str::to_owned);
            }
            let order = Order {
                file: p.file_id.to_string(),
                generation: p.generation.to_string(),
                offset: p.byte_offset,
                ordinal: p.ordinal,
                epoch: *epoch,
                context_epoch: context.0,
                gap_epoch: *gap,
            };
            match out.entry(value.id.clone()) {
                std::collections::btree_map::Entry::Vacant(e) => {
                    e.insert(Some(order));
                }
                std::collections::btree_map::Entry::Occupied(mut e) => {
                    if e.get().as_ref() != Some(&order) {
                        e.insert(None);
                    }
                }
            }
        }
    }
    for ids in line_requests.values().filter(|ids| ids.len() > 1) {
        for id in ids {
            out.insert(id.clone(), None);
        }
    }
    Ok(out)
}
fn trajectory(
    s: &Snapshot,
    scope: &Scope,
    thread: &crate::usage_store::ThreadEntry,
    rows: &[&PricedMeasurement],
    all: &[&PricedMeasurement],
    events: &[Arc<crate::session_events::Event>],
) -> Result<Vec<InputPoint>> {
    let order = order_map(events)?;
    let selected: BTreeSet<_> = rows.iter().map(|r| r.fact.id.as_str()).collect();
    let mut rows: Vec<_> = all
        .iter()
        .copied()
        .filter(|r| r.fact.request_scoped && r.fact.grain.as_ref() == "response")
        .collect();
    rows.sort_by(|a, b| {
        order
            .get(&a.fact.id)
            .cmp(&order.get(&b.fact.id))
            .then(a.fact.id.cmp(&b.fact.id))
    });
    let uncertain = rows
        .iter()
        .any(|r| order.get(&r.fact.id).and_then(Option::as_ref).is_none());
    let mut previous: Option<(&Measurement, &Order)> = None;
    let mut out = Vec::new();
    for row in rows {
        let f = &row.fact;
        let position = (!uncertain)
            .then(|| order.get(&f.id).and_then(Option::as_ref))
            .flatten();
        let boundary = match (previous, position) {
            (_, None) => Some(InputBoundary::AmbiguousOrder),
            (None, Some(_)) => Some(InputBoundary::First),
            (Some((_, a)), Some(b)) if a.file != b.file || a.generation != b.generation => {
                Some(InputBoundary::SourceChange)
            }
            (Some((_, a)), Some(b)) if a.epoch != b.epoch => Some(InputBoundary::Compaction),
            (Some((_, a)), Some(b)) if a.gap_epoch != b.gap_epoch => Some(InputBoundary::SourceGap),
            (Some((_, a)), Some(b)) if a.context_epoch != b.context_epoch => {
                Some(InputBoundary::ModelChange)
            }
            (Some((a, _)), Some(_)) if a.turn_id != f.turn_id => Some(InputBoundary::TurnChange),
            (_, Some(_))
                if f.model.raw.is_none()
                    || f.reasoning_effort.is_none()
                    || previous.is_some_and(|(a, _)| {
                        a.model.raw.is_none() || a.reasoning_effort.is_none()
                    }) =>
            {
                Some(InputBoundary::MissingContext)
            }
            (Some((a, _)), Some(_))
                if a.model != f.model || a.reasoning_effort != f.reasoning_effort =>
            {
                Some(InputBoundary::ModelChange)
            }
            (Some((a, _)), Some(_))
                if a.tokens.raw_input.is_none() || f.tokens.raw_input.is_none() =>
            {
                Some(InputBoundary::MissingInput)
            }
            (Some((a, _)), Some(_)) if !selected.contains(a.id.as_str()) => {
                Some(InputBoundary::ScopeGap)
            }
            _ => None,
        };
        let change = |a: Option<u64>, b: Option<u64>| {
            if boundary.is_none() {
                a.zip(b).map(|(a, b)| b as i64 - a as i64)
            } else {
                None
            }
        };
        if selected.contains(f.id.as_str()) {
            let compaction_comparison = previous.and_then(|(before, a)| {
                let b = position?;
                if boundary != Some(InputBoundary::Compaction)
                    || !selected.contains(before.id.as_str())
                    || a.file != b.file
                    || a.generation != b.generation
                    || a.context_epoch != b.context_epoch
                    || a.gap_epoch != b.gap_epoch
                    || before.turn_id.is_none()
                    || before.turn_id != f.turn_id
                    || before.model != f.model
                    || before.model.raw.is_none()
                    || before.reasoning_effort.is_none()
                    || before.reasoning_effort != f.reasoning_effort
                {
                    return None;
                }
                let (before_input, after_input) =
                    before.tokens.raw_input.zip(f.tokens.raw_input)?;
                Some(CompactionInputComparison {
                    before_measurement_id: before.id.clone(),
                    before_input,
                    after_input,
                    input_difference: after_input as i64 - before_input as i64,
                    before_evidence: evidence(
                        s,
                        scope,
                        &thread.thread.id,
                        before.turn_id.as_deref(),
                        None,
                    ),
                })
            });
            out.push(InputPoint {
                measurement_id: f.id.clone(),
                timestamp: f.timestamp.clone(),
                input: f.tokens.raw_input,
                uncached_input: f.tokens.input,
                cache_read: f.tokens.cache_read,
                input_delta: change(
                    previous.and_then(|(p, _)| p.tokens.raw_input),
                    f.tokens.raw_input,
                ),
                uncached_delta: change(previous.and_then(|(p, _)| p.tokens.input), f.tokens.input),
                boundary,
                epoch: position.map_or(0, |p| p.epoch),
                evidence: evidence(s, scope, &thread.thread.id, f.turn_id.as_deref(), None),
                compaction_comparison,
            });
        }
        previous = position.map(|p| (f.as_ref(), p));
    }
    Ok(out)
}
fn failed(op: &Operation) -> bool {
    inspection_outcomes(op).failed > 0
}
fn inspection_outcomes(op: &Operation) -> crate::timing::work::Outcomes {
    let mut outcomes = crate::timing::work::Outcomes::default();
    outcomes.record(op, false);
    // Confirmed search/test/diff exit 1 is an expected negative result, not a failed request.
    if outcomes.failed > 0
        && op.exit_code == Some(1)
        && op.matching.as_ref().is_some_and(|m| m.expected_nonzero)
    {
        outcomes.succeeded += outcomes.failed;
        outcomes.failed = 0;
    }
    outcomes
}
fn operation_matches(op: &Operation, scope: &Scope, tz: Tz, rows: &[&PricedMeasurement]) -> bool {
    let at = op
        .timestamp
        .as_deref()
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map(|s| s.with_timezone(&tz).date_naive().to_string());
    if scope.undated == Some(true) {
        if at.is_some() {
            return false;
        }
    } else if (scope.since.is_some() || scope.until.is_some())
        && at.as_ref().is_none_or(|at| {
            scope.since.as_ref().is_some_and(|s| at < s)
                || scope.until.as_ref().is_some_and(|u| at >= u)
        })
    {
        return false;
    }
    // Historical model/effort filters need exact turn-associated measurements.
    if scope.model.is_some()
        || scope.model_unknown == Some(true)
        || scope.reasoning_effort.is_some()
        || scope.effort_unknown == Some(true)
    {
        return op
            .turn_id
            .as_ref()
            .is_some_and(|id| rows.iter().any(|r| r.fact.turn_id.as_ref() == Some(id)));
    }
    true
}
fn candidate(
    s: &Snapshot,
    scope: &Scope,
    thread: &crate::usage_store::ThreadEntry,
    rows: &[&PricedMeasurement],
    ops: &[Operation],
    points: &[InputPoint],
    p: &InspectionPolicy,
) -> Result<InvestigationCandidate> {
    let usage = summarize(rows)?;
    let paired: Vec<_> = rows
        .iter()
        .copied()
        .filter(|r| r.fact.request_scoped && r.fact.grain.as_ref() == "response")
        .collect();
    let pair_summary = summarize(&paired)?;
    let input = pair_summary.input_total;
    let cache_share = pair_summary.cache_hit_rate;
    let mut outcomes = crate::timing::work::Outcomes::default();
    for op in ops {
        let observed = inspection_outcomes(op);
        outcomes.succeeded += observed.succeeded;
        outcomes.failed += observed.failed;
        outcomes.nonterminal += observed.nonterminal;
        outcomes.indeterminate += observed.indeterminate;
        outcomes.conflicting += observed.conflicting;
        outcomes.identity_gaps += observed.identity_gaps;
        outcomes.unclassified += observed.unclassified;
    }
    let determinate_operations = outcomes.determinate();
    let failed_operations = outcomes.failed;
    let mut fingerprints = BTreeMap::new();
    for op in ops {
        if let Some(m) = &op.matching
            && let Some(turn) = &op.turn_id
            && let Some(receiver) = &m.receiver_owner
            && let Some(fingerprint) = &m.function_request_fingerprint
        {
            *fingerprints
                .entry((turn, receiver, fingerprint))
                .or_insert(0u64) += 1;
        }
    }
    let repeated_requests = fingerprints.values().map(|n| n.saturating_sub(1)).sum();
    let jump = points
        .iter()
        .filter_map(|r| r.uncached_delta.filter(|d| *d > 0).map(|v| v as u64))
        .max();
    let mut proof = vec![evidence(
        s,
        scope,
        &thread.thread.id,
        scope.turn_id.as_deref(),
        None,
    )];
    if let Some(point) = points
        .iter()
        .filter(|p| p.uncached_delta.is_some_and(|v| v > 0))
        .max_by_key(|p| p.uncached_delta)
    {
        proof.push(point.evidence.clone());
    }
    if let Some(op) = ops.iter().find(|o| failed(o)) {
        proof.push(evidence(
            s,
            scope,
            &thread.thread.id,
            op.turn_id.as_deref(),
            Some(&op.id),
        ));
    }
    let mut candidate = InvestigationCandidate {
        thread_id: thread.thread.id.clone(),
        title: thread.thread.title.clone(),
        signals: vec![],
        usage,
        input,
        cache_share,
        largest_uncached_jump: jump,
        determinate_operations,
        failed_operations,
        outcome_gaps: outcomes.nonterminal
            + outcomes.indeterminate
            + outcomes.conflicting
            + outcomes.identity_gaps
            + outcomes.unclassified,
        repeated_requests,
        evidence: proof,
    };
    candidate.signals = rules::signals(&candidate, p);
    Ok(candidate)
}
fn resources(
    s: &Snapshot,
    scope: &Scope,
    thread: &crate::usage_store::ThreadEntry,
    ops: &[Operation],
    out: &mut BTreeMap<String, ResourceHotspot>,
) -> Result<usize> {
    let mut missing = 0;
    for op in ops {
        let mut targets: BTreeMap<(String, String), (bool, bool, bool)> = BTreeMap::new();
        if let Some(m) = &op.matching {
            for target in &m.read_targets {
                targets
                    .entry((
                        match target.platform {
                            SourcePathPlatform::Posix => "lexical_posix",
                            SourcePathPlatform::Windows => "lexical_windows",
                        }
                        .into(),
                        target.path.clone(),
                    ))
                    .or_default()
                    .0 = true;
            }
        }
        if let Some(w) = &op.work
            && let WorkData::FileChange {
                changes: Some(changes),
            } = &w.data
        {
            for change in changes {
                for path in change.targets() {
                    let entry = targets
                        .entry(("reported_path".into(), path.into()))
                        .or_default();
                    if w.stage == WorkStage::Proposed {
                        entry.1 = true;
                    } else {
                        entry.2 = true;
                    }
                }
            }
        }
        if targets.is_empty() {
            missing += 1;
        }
        for ((basis, path), (read, proposed, reported)) in targets {
            let id = crate::hash(serde_json::to_vec(&(
                &thread.thread.source_instance_id,
                &thread.thread.project,
                &basis,
                &path,
            ))?);
            let row = out.entry(id.clone()).or_insert_with(|| ResourceHotspot {
                id,
                source_instance_id: thread.thread.source_instance_id.clone(),
                project: thread.thread.project.clone(),
                path,
                identity_basis: basis,
                operations: 0,
                reads: 0,
                proposed_changes: 0,
                reported_changes: 0,
                failed_operations: 0,
                known_duration_ms: None,
                duration_covered_operations: 0,
                actual_changes: None,
                evidence: vec![],
            });
            row.operations += 1;
            row.reads += u64::from(read);
            row.proposed_changes += u64::from(proposed);
            row.reported_changes += u64::from(reported);
            row.failed_operations += u64::from(failed(op));
            if let Some(ms) = op.duration_ms {
                row.known_duration_ms = Some(
                    row.known_duration_ms
                        .unwrap_or(0)
                        .checked_add(ms)
                        .filter(|n| *n <= MAX_SAFE_INTEGER)
                        .ok_or_else(limit)?,
                );
                row.duration_covered_operations += 1;
            }
            if row.evidence.len() < 3 {
                row.evidence.push(evidence(
                    s,
                    scope,
                    &thread.thread.id,
                    op.turn_id.as_deref(),
                    Some(&op.id),
                ));
            }
        }
        if out.len() > 10_000 {
            return Err(limit());
        }
    }
    Ok(missing)
}
pub(super) fn execute(mut request: Request, snapshot: &Snapshot) -> Result<Response> {
    ensure_ready(&request, snapshot)?;
    let tz = timezone(&request.scope)?;
    request.scope.timezone = Some(tz.to_string());
    if let Some(id) = &request.thread_id {
        request.scope.thread_id = Some(id.clone());
    }
    let automatic_week = request.action == Action::Review && request.scope.since.is_none();
    if automatic_week {
        let at = DateTime::parse_from_rfc3339(&snapshot.manifest.snapshot_ref.created_at)?
            .with_timezone(&tz)
            .date_naive();
        let start = at - Duration::days(i64::from(at.weekday().num_days_from_monday()));
        request.scope.since = Some(start.to_string());
        request.scope.until = Some((start + Duration::days(7)).to_string());
    }
    let mut selected_turn = if let Some(turn) = &request.scope.turn_id {
        let thread = request
            .scope
            .thread_id
            .as_deref()
            .ok_or_else(|| invalid("轮次检查需要任务身份"))?;
        let owner = snapshot
            .manifest
            .threads
            .iter()
            .find(|t| t.thread.id == thread)
            .ok_or_else(|| operation_error("NOT_FOUND", "未找到任务"))?;
        Some(snapshot.timing_evidence(
            crate::usage_store::timing_evidence::TurnTarget {
                source: &owner.thread.source_instance_id,
                thread,
                turn,
            },
            crate::usage_store::timing_evidence::TimingReadBudget::default(),
            &AtomicBool::new(false),
        )?)
    } else {
        None
    };
    let rows = if let Some(read) = &mut selected_turn {
        std::mem::take(&mut read.priced_measurements)
    } else {
        snapshot.ledger()?
    };
    if rows.len() > MAX_FACTS {
        return Err(limit());
    }
    let projects: BTreeMap<_, _> = snapshot
        .manifest
        .threads
        .iter()
        .map(|t| (t.thread.id.as_str(), t.thread.project.as_deref()))
        .collect();
    let selected: Vec<_> = rows
        .iter()
        .filter(|r| matches(r, &request.scope, tz, &projects))
        .collect();
    let mut all_by_thread: BTreeMap<&str, Vec<&PricedMeasurement>> = BTreeMap::new();
    for row in &rows {
        if let Some(id) = row.fact.thread_id.as_deref() {
            all_by_thread.entry(id).or_default().push(row);
        }
    }
    let mut by_thread: BTreeMap<&str, Vec<&PricedMeasurement>> = BTreeMap::new();
    for row in &selected {
        if let Some(id) = row.fact.thread_id.as_deref() {
            by_thread.entry(id).or_default().push(row);
        }
    }
    let mut all_candidates = vec![];
    let mut points = vec![];
    let mut context_records = vec![];
    let mut hotspots = BTreeMap::new();
    let mut tools: BTreeMap<String, ToolFamilyCount> = BTreeMap::new();
    let mut unlocated_operations = 0;
    let mut operation_work = 0;
    let mut event_work = EventWork::default();
    let mut unknown_input_order = false;
    let mut selected_threads = 0;
    let mut outcome_gaps = false;
    let policy = InspectionPolicy::default();
    let mut activity = matches!(request.action, Action::Investigate | Action::Review)
        .then(|| activity::Builder::new(&request.scope))
        .transpose()?;
    let baseline_scope = activity.as_ref().and_then(|a| a.baseline_scope.clone());
    let baseline_rows: Vec<_> = baseline_scope
        .as_ref()
        .map(|scope| {
            rows.iter()
                .filter(|r| matches(r, scope, tz, &projects))
                .collect()
        })
        .unwrap_or_default();
    let mut opportunities = matches!(request.action, Action::Investigate | Action::Review)
        .then(|| {
            opportunities::Builder::new(
                snapshot,
                &request.scope,
                &selected,
                baseline_scope
                    .as_ref()
                    .map(|scope| (scope, baseline_rows.as_slice())),
            )
        })
        .transpose()?;
    for thread in &snapshot.manifest.threads {
        let t = &thread.thread;
        if request
            .scope
            .thread_id
            .as_ref()
            .is_some_and(|id| id != &t.id)
            || request
                .scope
                .project
                .as_ref()
                .is_some_and(|p| Some(p) != t.project.as_ref())
            || request.scope.project_unknown == Some(true) && t.project.is_some()
            || request
                .scope
                .source_instance_id
                .as_ref()
                .is_some_and(|id| id != &t.source_instance_id)
            || request
                .scope
                .agent_kind
                .as_ref()
                .is_some_and(|a| a != &t.agent_kind)
        {
            continue;
        }
        selected_threads += 1;
        if selected_threads > 10_000 {
            return Err(limit());
        }
        let matching = by_thread.get(t.id.as_str()).cloned().unwrap_or_default();
        if request.action == Action::Context {
            context_records.extend(context::records(
                snapshot,
                &request.scope,
                thread,
                &matching,
                &mut event_work,
                tz,
            )?);
            continue;
        }
        let mut ops = vec![];
        if request.action != Action::Trajectory {
            if let Some(read) = &selected_turn {
                ops.extend(read.operations.iter().map(|op| op.as_ref().clone()));
            } else {
                for turn in thread.turns.keys() {
                    let operations = snapshot.operations_for_turn(&t.id, turn)?;
                    operation_work += operations.len();
                    if operation_work > MAX_WORK {
                        return Err(limit());
                    }
                    ops.extend(operations.iter().map(|op| op.as_ref().clone()));
                }
            }
            // Canonical operation identities are retained once, including unassigned turns.
            ops.sort_by(|a, b| a.id.cmp(&b.id));
            ops.dedup_by(|a, b| a.id == b.id);
            if let (Some(activity), Some(baseline)) = (&mut activity, &baseline_scope) {
                let baseline_rows: Vec<_> = all_by_thread
                    .get(t.id.as_str())
                    .into_iter()
                    .flatten()
                    .copied()
                    .filter(|r| matches(r, baseline, tz, &projects))
                    .collect();
                for op in &ops {
                    if operation_matches(op, baseline, tz, &baseline_rows) {
                        activity.add(snapshot, baseline, thread, op, true);
                    }
                }
            }
            ops.retain(|op| operation_matches(op, &request.scope, tz, &matching));
            if let Some(activity) = &mut activity {
                for op in &ops {
                    activity.add(snapshot, &request.scope, thread, op, false);
                }
            }
        }
        let events = if request.action != Action::Resources
            && (opportunities.is_some()
                || request.action == Action::Trajectory
                || !matching.is_empty())
        {
            if let Some(read) = selected_turn.take() {
                let mut events: Vec<_> = read.events.into_iter().chain(read.controls).collect();
                sort_events(&mut events);
                events
            } else {
                thread_events(snapshot, thread, &mut event_work)?
            }
        } else {
            vec![]
        };
        if let Some(checks) = &mut opportunities {
            checks.thread(
                snapshot,
                &request.scope,
                thread,
                &ops,
                &events,
                &matching,
                tz,
            )?;
        }
        let current_points = if request.action != Action::Resources && !matching.is_empty() {
            trajectory(
                snapshot,
                &request.scope,
                thread,
                &matching,
                all_by_thread
                    .get(t.id.as_str())
                    .map(Vec::as_slice)
                    .unwrap_or_default(),
                &events,
            )?
        } else {
            vec![]
        };
        unknown_input_order |= current_points
            .iter()
            .any(|p| p.boundary == Some(InputBoundary::AmbiguousOrder));
        if request.action == Action::Trajectory {
            points.extend(current_points);
            continue;
        }
        if request.action != Action::Resources {
            all_candidates.push(candidate(
                snapshot,
                &request.scope,
                thread,
                &matching,
                &ops,
                &current_points,
                &policy,
            )?);
        }
        if matches!(request.action, Action::Resources | Action::Review) {
            unlocated_operations +=
                resources(snapshot, &request.scope, thread, &ops, &mut hotspots)?;
        }
        for op in &ops {
            outcome_gaps |= inspection_outcomes(op).partial();
        }
        for op in &ops {
            let entry = tools
                .entry(op.kind.to_string())
                .or_insert_with(|| ToolFamilyCount {
                    kind: op.kind.to_string(),
                    operations: 0,
                    failed: 0,
                });
            entry.operations += 1;
            entry.failed += u64::from(failed(op));
        }
    }
    if request.action == Action::Trajectory
        && !snapshot
            .manifest
            .threads
            .iter()
            .any(|t| Some(&t.thread.id) == request.scope.thread_id.as_ref())
    {
        return Err(operation_error("NOT_FOUND", "未找到输入轨迹任务"));
    }
    all_candidates.sort_by(|a, b| {
        consumption_order(&a.usage, &b.usage, &None).then(a.thread_id.cmp(&b.thread_id))
    });
    let mut candidates: Vec<_> = all_candidates
        .iter()
        .filter(|c| !c.signals.is_empty())
        .cloned()
        .collect();
    let candidate_count = candidates.len();
    let mut resources: Vec<_> = hotspots.into_values().collect();
    resources.sort_by(|a, b| b.operations.cmp(&a.operations).then(a.id.cmp(&b.id)));
    let resource_count = resources.len();
    let review = if request.action == Action::Review {
        let a = date(request.scope.since.as_deref().unwrap())?;
        let b = date(request.scope.until.as_deref().unwrap())?;
        let days = (b - a).num_days();
        let mut r = request.clone();
        r.action = Action::Compare;
        r.comparison = Some(ComparisonRequest::Periods {
            baseline_since: (a - Duration::days(days)).to_string(),
            baseline_until: a.to_string(),
            dimension: DriverDimension::Project,
        });
        r.limit = Some(10);
        let comparison = comparison::execute(r, snapshot)?.comparison;
        let mut models: BTreeMap<Option<String>, Vec<&PricedMeasurement>> = BTreeMap::new();
        for row in &selected {
            models
                .entry(row.fact.model.raw.as_deref().map(str::to_owned))
                .or_default()
                .push(row);
        }
        let mut groups = Vec::new();
        for (key, rows) in &models {
            groups.push(ReviewGroup {
                key: key.clone(),
                usage: summarize(rows)?,
            });
        }
        groups.sort_by(|a, b| consumption_order(&a.usage, &b.usage, &None).then(a.key.cmp(&b.key)));
        let keys: BTreeSet<_> = groups.iter().take(10).map(|g| g.key.clone()).collect();
        let rest: Vec<_> = selected
            .iter()
            .copied()
            .filter(|r| !keys.contains(&r.fact.model.raw.as_deref().map(str::to_owned)))
            .collect();
        groups.truncate(10);
        let total_tokens = summarize(&selected)?.complete_token_total();
        let selected_ids: BTreeSet<_> = all_candidates
            .iter()
            .take(10)
            .map(|c| c.thread_id.as_str())
            .collect();
        let remaining_rows: Vec<_> = selected
            .iter()
            .copied()
            .filter(|r| {
                r.fact
                    .thread_id
                    .as_deref()
                    .is_none_or(|id| !selected_ids.contains(id))
            })
            .collect();
        let top = |n| -> Result<Option<u64>> {
            summarize(
                &selected
                    .iter()
                    .copied()
                    .filter(|r| {
                        r.fact.thread_id.as_deref().is_some_and(|id| {
                            all_candidates.iter().take(n).any(|c| c.thread_id == id)
                        })
                    })
                    .collect::<Vec<_>>(),
            )
            .map(|s| s.complete_token_total())
        };
        let top_task_tokens = top(1)?;
        let top_five_tokens = top(5)?;
        let top_ten_tokens = top(10)?;
        let share = |n: Option<u64>| {
            n.zip(total_tokens)
                .filter(|(_, d)| *d > 0)
                .map(|(n, d)| n as f64 / d as f64)
        };
        let concentration = ReviewConcentration {
            method_version: 1,
            measured_tasks: selected
                .iter()
                .filter_map(|r| r.fact.thread_id.as_deref())
                .collect::<BTreeSet<_>>()
                .len(),
            total_tokens,
            top_task_tokens,
            top_task_share: share(top_task_tokens),
            top_five_tokens,
            top_five_share: share(top_five_tokens),
            top_ten_tokens,
            top_ten_share: share(top_ten_tokens),
            remaining_task_usage: summarize(&remaining_rows)?,
        };
        Some(PeriodReview {
            comparison,
            top_tasks: all_candidates.into_iter().take(10).collect(),
            models: groups,
            tools: tools.into_values().collect(),
            remaining_model_usage: summarize(&rest)?,
            week_start: automatic_week.then(|| "monday".into()),
            concentration: Some(concentration),
        })
    } else {
        None
    };
    let total = match request.action {
        Action::Investigate => candidate_count,
        Action::Trajectory => points.len(),
        Action::Resources => resource_count,
        Action::Review => 1,
        Action::Context => context_records.len(),
        _ => unreachable!(),
    };
    let offset = request.offset.unwrap_or(0);
    let count = request.limit.unwrap_or(10);
    let end = offset.saturating_add(count).min(total);
    let paginate = |len: usize| offset.min(len)..offset.saturating_add(count).min(len);
    if request.action == Action::Review {
        candidates.truncate(10);
        resources.truncate(10);
    } else {
        candidates = candidates[paginate(candidates.len())].to_vec();
        points = points[paginate(points.len())].to_vec();
        resources = resources[paginate(resources.len())].to_vec();
    }
    let quality = quality(snapshot, selected.len());
    let context = if request.action == Action::Context {
        context_records.sort_by(|a, b| a.timestamp.cmp(&b.timestamp).then(a.id.cmp(&b.id)));
        let injected_records = context_records
            .iter()
            .filter(|r| matches!(r.kind, ContextRecordKind::InjectedContext))
            .count();
        Some(ContextInventory {
            observed_records: context_records.len(),
            injected_records,
            model_window_records: context_records.len() - injected_records,
            records: context_records[paginate(context_records.len())].to_vec(),
        })
    } else {
        None
    };
    let partial = quality.status == "partial" || outcome_gaps || unknown_input_order;
    let mut limitations = vec![];
    if request.scope.turn_id.is_some() {
        limitations.push(InspectionLimit::SelectedTurnOnly);
    }
    if quality.status == "partial" {
        limitations.push(InspectionLimit::SourcePartial);
    }
    if matches!(
        request.action,
        Action::Trajectory | Action::Review | Action::Investigate
    ) {
        limitations.push(InspectionLimit::ContextOccupancyUnavailable);
    }
    if matches!(request.action, Action::Resources | Action::Review) {
        limitations.push(InspectionLimit::ActualChangesUnavailable);
        if unlocated_operations > 0 {
            limitations.push(InspectionLimit::UnlocatedOperations);
        }
    }
    if outcome_gaps {
        limitations.push(InspectionLimit::OperationOutcomesPartial);
    }
    if request.action != Action::Trajectory
        && (request.scope.model.is_some()
            || request.scope.model_unknown == Some(true)
            || request.scope.reasoning_effort.is_some()
            || request.scope.effort_unknown == Some(true))
    {
        limitations.push(InspectionLimit::OperationModelAssociation);
    }
    if unknown_input_order {
        limitations.push(InspectionLimit::UnknownInputOrder);
    }
    if request.action == Action::Context {
        limitations.push(InspectionLimit::ContextMetadataUnavailable);
        limitations.push(InspectionLimit::ContextOccupancyUnavailable);
    }
    let kind = match request.action {
        Action::Investigate => InspectionKind::Investigate,
        Action::Trajectory => InspectionKind::Trajectory,
        Action::Resources => InspectionKind::Resources,
        Action::Review => InspectionKind::Review,
        Action::Context => InspectionKind::Context,
        _ => unreachable!(),
    };
    let compact = request.compact == Some(true);
    let mut response = Response {
        statistics: None,
        inspection: Some(Inspection {
            method_version: 3,
            kind,
            policy,
            partial,
            limitations,
            candidates,
            trajectory: points,
            resources,
            review,
            candidate_count,
            resource_count,
            unlocated_operations,
            context,
            activity: activity.map(activity::Builder::finish),
            opportunities: opportunities
                .map(|builder| builder.finish(snapshot, tz))
                .transpose()?,
        }),
        comparison: None,
        facets: None,
        distribution: None,
        price_update: None,
        freshness: None,
        output_version: 5,
        action: request.action,
        snapshot_ref: snapshot.manifest.snapshot_ref.clone(),
        scope: request.scope,
        available_range: available(&selected, tz),
        summary: summarize(&selected)?,
        items: vec![],
        page: Page {
            offset,
            limit: count,
            total,
            next_offset: (end < total).then_some(end),
        },
        quality,
    };
    if compact {
        super::compact::apply(&mut response);
    }
    if serde_json::to_vec(&response)?.len() > MAX_BYTES {
        return Err(limit());
    }
    Ok(response)
}

#[cfg(test)]
pub(super) mod tests;
