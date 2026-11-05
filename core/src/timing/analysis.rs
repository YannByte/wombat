//! Safe-event mapping for a single explicit turn; no source bodies or time-based identity guesses.
use super::{context, intervals};
use crate::adapters::contract::Measurement;
use crate::operation_association::{self, ObservationKind, lifecycle::TerminalState};
use crate::session_events::{
    Event, Gap, ItemKind, LifecycleKind, MessageOrigin, Payload, Phase, Precision,
};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};
use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

/// Canonical measurements are the reconciled accounting projection, never raw event copies.
pub struct AnalyzeInput<'a> {
    pub source: &'a str,
    pub thread: &'a str,
    pub turn: &'a str,
    pub measurements: &'a [Arc<Measurement>],
    pub events: &'a [Arc<Event>],
    pub budget: Budget,
}

#[derive(Clone, Copy, Debug)]
pub struct Budget {
    pub events: usize,
    pub measurements: usize,
    pub lifecycle_records: usize,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum State {
    Running,
    Completed,
    Failed,
    Cancelled,
    Unknown,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Anchor {
    pub timestamp_ms: i64,
    pub precision: Precision,
}

#[derive(Debug, Eq, PartialEq)]
pub enum Issue {
    InvalidScope,
    ResourceLimit,
    SourceGap(String),
    IdentityConflict(String),
    TargetConflict(String),
    OutcomeConflict(String),
    BoundaryConflict,
    MissingBoundaryTime(String),
    MissingItemIdentity(String),
    MissingItemTime(String),
    ReversedBoundary,
    UnmatchedClockDomain(String),
    UnknownContent(String),
    MissingContentTime(String),
    ContentConflict(String),
    UnmatchedContentDomain(String),
}

#[derive(Debug, Default, Eq, PartialEq)]
pub struct Coverage {
    pub supplied_events: usize,
    pub supplied_measurements: usize,
    pub scoped_events: usize,
    pub scoped_measurements: usize,
    pub unassigned_events: usize,
    pub outside_events: usize,
    pub outside_measurements: usize,
    pub boundary_candidates: usize,
    pub lifecycle_candidates: [usize; intervals::CATEGORY_COUNT],
    pub linked_lifecycles: [usize; intervals::CATEGORY_COUNT],
    pub conflicting_lifecycles: usize,
    pub missing_identity_lifecycles: usize,
    /// Counts describe safe message records, never messages or API requests.
    pub user_input_records: usize,
    pub unclassified_user_records: usize,
    pub injected_context_records: usize,
    pub reasoning_message_records: usize,
    pub content_candidates: usize,
    pub nonempty_content_records: usize,
    pub unknown_content_records: usize,
    pub missing_content_time_records: usize,
    pub partial: bool,
}

/// Safe message presence cannot supply tool-batch completion or response-cycle identity.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ResponseGapSupport {
    UnsupportedMissingBatchAndCycleEvidence,
}

#[derive(Debug)]
pub struct Analysis {
    pub method: &'static str,
    pub state: State,
    pub start: Option<Anchor>,
    pub end: Option<Anchor>,
    pub native_wall_clock_ms: Option<u64>,
    pub derived_wall_clock_ms: Option<u64>,
    pub native_ttft_ms: Option<u64>,
    /// Native duration minus observed boundary duration; neither is rescaled.
    pub boundary_delta_ms: Option<i128>,
    pub intervals: intervals::IntervalMetrics,
    pub operation_coverage: OperationCoverage,
    pub repeated_behavior: super::repeats::Projection,
    /// Unknown without observed candidates and at least one valid closed interval.
    /// Counts describe observations only; absence never establishes source completeness.
    pub category_union_ms: [Option<u64>; intervals::CATEGORY_COUNT],
    pub context: Option<context::Statistics>,
    pub response_gap_support: ResponseGapSupport,
    pub response_gap_union_ms: Option<u64>,
    pub first_content_record_delay_ms: Option<u64>,
    pub coverage: Coverage,
    pub issues: Vec<Issue>,
}

/// Residual relative to recorded, paired operations; independent of four lifecycle categories.
/// A zero paired count means no intervals were subtracted, never model waiting.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct OperationCoverage {
    pub diagnostic_total: usize,
    pub diagnostics: Vec<crate::timing_dto::OperationDiagnostic>,
    pub method_version: u32,
    pub endpoint_method_version: u32,
    /// Canonical groups with an eligible operation; paired is their usable endpoint subset.
    pub candidates: usize,
    pub paired: usize,
    pub computed: bool,
    pub budget_exceeded: bool,
    /// Physical observations without operation identity; independent of canonical groups.
    pub missing_identity: usize,
    pub conflicting: usize,
    pub covered_ms: Option<u64>,
    pub residual_ms: Option<u64>,
    pub residual_ranges: Vec<(u64, u64)>,
    pub residual_range_count: usize,
    pub detail_limited: bool,
    pub partial: bool,
}

#[derive(Default)]
pub struct BoundaryReducer {
    candidates: usize,
    starts: BTreeMap<i64, Precision>,
    ends: BTreeMap<i64, Precision>,
    durations: BTreeSet<u64>,
    ttfts: BTreeSet<u64>,
    native_ids: BTreeSet<String>,
    states: BTreeSet<u8>,
    clock_domain_limit: bool,
    clock_domains: BTreeMap<(std::sync::Arc<str>, std::sync::Arc<str>), (bool, bool)>,
    missing_start: bool,
    missing_end: bool,
    conflict: bool,
}

pub const BOUNDARY_METHOD_VERSION: &str = "safe_event_turn_boundary_v1";

/// Complete exact-turn boundary reduction shared with the persisted native index.
/// Unique values retain at most two witnesses; clock domains retain at most
/// 100,000 entries. Domain overflow hides anchors, never independent native values.
/// Absolute anchors and independent native values retain separate conflict flags.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct BoundarySummary {
    pub format_version: u32,
    pub state: State,
    pub start: Option<Anchor>,
    pub end: Option<Anchor>,
    pub native_wall_clock_ms: Option<u64>,
    pub native_ttft_ms: Option<u64>,
    pub candidates: usize,
    pub identity_conflict: bool,
    pub state_conflict: bool,
    pub boundary_conflict: bool,
    pub native_duration_conflict: bool,
    pub native_ttft_conflict: bool,
    pub clock_domain_limit: bool,
    pub missing_start_time: bool,
    pub missing_end_time: bool,
}
impl BoundaryReducer {
    /// Caller supplies only the complete, source-validated exact turn partition.
    /// Returns whether this record has a missing boundary timestamp.
    pub fn observe(&mut self, event: &Event) -> bool {
        let mut missing_time = false;
        if let Payload::Lifecycle {
            lifecycle: LifecycleKind::Turn,
            phase,
            native_id,
            duration_ms,
            first_token_ms,
        } = event.payload()
        {
            self.candidates += 1;
            self.conflict |= conflicting(event);

            if let Some(id) = native_id
                && self.native_ids.len() < 2
            {
                self.native_ids.insert(id.clone());
            }
            if terminal(phase)
                && let Some(value) = duration_ms
                && self.durations.len() < 2
            {
                self.durations.insert(*value);
            }
            if let Some(value) = first_token_ms
                && self.ttfts.len() < 2
            {
                self.ttfts.insert(*value);
            }
            let destination = if *phase == Phase::Started {
                self.states.insert(0);
                Some(&mut self.starts)
            } else if terminal(phase) {
                self.states.insert(match phase {
                    Phase::Completed => 1,
                    Phase::Failed => 2,
                    Phase::Cancelled => 3,
                    _ => unreachable!(),
                });
                Some(&mut self.ends)
            } else {
                None
            };
            if let Some(destination) = destination {
                if let Some(time) = timestamp(event) {
                    let key = (
                        event.position().file_id.clone(),
                        event.position().generation.clone(),
                    );
                    if self.clock_domains.len() >= 100_000 && !self.clock_domains.contains_key(&key)
                    {
                        self.clock_domain_limit = true;
                    } else {
                        let domain = self.clock_domains.entry(key).or_default();
                        if *phase == Phase::Started {
                            domain.0 = true;
                        } else {
                            domain.1 = true;
                        }
                    }
                    let precision = event.time().precision.clone();
                    // Two distinct values permanently establish conflict. Same
                    // value precision still uses the least precise observation.
                    if destination.len() < 2 || destination.contains_key(&time) {
                        destination
                            .entry(time)
                            .and_modify(|old| {
                                if precision_rank(&precision) < precision_rank(old) {
                                    *old = precision.clone();
                                }
                            })
                            .or_insert(precision);
                    }
                } else {
                    if *phase == Phase::Started {
                        self.missing_start = true;
                    } else {
                        self.missing_end = true;
                    }
                    missing_time = true;
                }
            }
        }

        missing_time
    }
    pub fn finish(self) -> BoundarySummary {
        let mut result = BoundarySummary {
            format_version: 1,
            state: State::Unknown,
            start: None,
            end: None,
            native_wall_clock_ms: None,
            native_ttft_ms: None,
            candidates: self.candidates,
            identity_conflict: false,
            state_conflict: false,
            boundary_conflict: false,
            native_duration_conflict: self.durations.len() > 1,
            native_ttft_conflict: self.ttfts.len() > 1,
            clock_domain_limit: self.clock_domain_limit,
            missing_start_time: self.missing_start,
            missing_end_time: self.missing_end,
        };
        let identity_conflict = self.conflict || self.native_ids.len() > 1;
        let state_conflict = self.states.iter().filter(|value| **value != 0).count() > 1;
        let boundary_conflict = self.clock_domain_limit
            || identity_conflict
            || state_conflict
            || self.starts.len() > 1
            || self.ends.len() > 1
            || (!self.starts.is_empty()
                && !self.ends.is_empty()
                && !self
                    .clock_domains
                    .values()
                    .any(|(start, end)| *start && *end));
        if !boundary_conflict {
            result.start = (!self.missing_start)
                .then(|| anchor(&self.starts))
                .flatten();
            result.end = (!self.missing_end).then(|| anchor(&self.ends)).flatten();
        }
        if !identity_conflict && !state_conflict {
            result.state = match self
                .states
                .iter()
                .find(|value| **value != 0)
                .copied()
                .or_else(|| self.states.first().copied())
            {
                Some(0) => State::Running,
                Some(1) => State::Completed,
                Some(2) => State::Failed,
                Some(3) => State::Cancelled,
                _ => State::Unknown,
            };
        }
        // Invalid or conflicting absolute anchors cannot invalidate an independent
        // native duration, but conflicting turn identities invalidate both methods.
        if !identity_conflict {
            result.native_wall_clock_ms = singleton(&self.durations);
            result.native_ttft_ms = singleton(&self.ttfts);
        }

        result.identity_conflict = identity_conflict;
        result.state_conflict = state_conflict;
        result.boundary_conflict = boundary_conflict;
        result
    }
}

fn terminal(phase: &Phase) -> bool {
    matches!(phase, Phase::Completed | Phase::Failed | Phase::Cancelled)
}
fn category(item_kind: &ItemKind) -> Option<usize> {
    match item_kind {
        ItemKind::Command => Some(0),
        ItemKind::Compaction => Some(1),
        ItemKind::Reasoning => Some(2),
        ItemKind::Mcp => Some(3),
        _ => None,
    }
}
fn phase_category(kind: ObservationKind<'_>) -> Option<usize> {
    match kind {
        ObservationKind::Item(item_kind) => category(item_kind),
        ObservationKind::Compaction => Some(1),
        ObservationKind::Operation(operation)
            if matches!(
                operation.kind.as_ref(),
                "mcpTool" | "mcpResource" | "mcpDiscovery" | "mcpUnclassified" | "mcpConflict"
            ) =>
        {
            Some(3)
        }
        ObservationKind::Operation(_) => None,
    }
}
fn phase_operation(kind: ObservationKind<'_>) -> bool {
    match kind {
        ObservationKind::Item(kind) => matches!(
            kind,
            ItemKind::Command | ItemKind::File | ItemKind::Mcp | ItemKind::Tool
        ),
        ObservationKind::Operation(operation) => matches!(
            operation.kind.as_ref(),
            "tool"
                | "command"
                | "file"
                | "mcp"
                | "mcpTool"
                | "mcpResource"
                | "mcpDiscovery"
                | "mcpUnclassified"
                | "mcpConflict"
                | "skillRead"
                | "search"
                | "image"
                | "agent"
        ),
        ObservationKind::Compaction => false,
    }
}
fn event_category(event: &Event) -> Option<usize> {
    match event.payload() {
        Payload::Item { item_kind, .. } => category(item_kind),
        Payload::Lifecycle {
            lifecycle: LifecycleKind::Compaction,
            ..
        } => Some(1),
        Payload::Operation { value, .. }
            if matches!(
                value.kind.as_ref(),
                "mcpTool" | "mcpResource" | "mcpDiscovery" | "mcpUnclassified" | "mcpConflict"
            ) =>
        {
            Some(3)
        }
        _ => None,
    }
}
fn category_at(index: usize) -> intervals::Category {
    [
        intervals::Category::Command,
        intervals::Category::Compaction,
        intervals::Category::Reasoning,
        intervals::Category::Mcp,
    ][index]
}
fn precision_rank(precision: &Precision) -> u8 {
    match precision {
        Precision::Second => 0,
        Precision::Millisecond => 1,
        Precision::Microsecond => 2,
        Precision::Nanosecond => 3,
        Precision::Unknown => 0,
    }
}
fn timestamp(event: &Event) -> Option<i64> {
    event.time().timestamp.as_deref().and_then(|time| {
        chrono::DateTime::parse_from_rfc3339(time)
            .ok()
            .map(|time| time.timestamp_millis())
    })
}
fn conflicting(event: &Event) -> bool {
    event
        .gaps()
        .iter()
        .any(|gap| matches!(gap, Gap::ConflictingIdentity | Gap::MissingIdentity))
}
fn singleton<T: Copy + Ord>(values: &BTreeSet<T>) -> Option<T> {
    (values.len() == 1).then(|| *values.first().unwrap())
}
fn anchor(values: &BTreeMap<i64, Precision>) -> Option<Anchor> {
    if values.len() != 1 {
        return None;
    }
    values
        .first_key_value()
        .map(|(timestamp_ms, precision)| Anchor {
            timestamp_ms: *timestamp_ms,
            precision: precision.clone(),
        })
}

/// O(E log E + M log M) time and O(E + M) space, bounded before retained collections.
/// An input limit returns partial unknown results, never a selected prefix. Native
/// durations are independent observations and cannot supply absolute endpoints.
pub fn analyze(input: AnalyzeInput<'_>) -> Analysis {
    analyze_impl(input, &AtomicBool::new(false)).expect("uncancelled analysis")
}

/// Cancellation is checked during bounded record and identity reduction, not
/// merely before and after the query. It never returns a successful prefix.
pub fn analyze_cancellable(
    input: AnalyzeInput<'_>,
    cancelled: &AtomicBool,
) -> anyhow::Result<Analysis> {
    analyze_impl(input, cancelled)
}
pub(super) fn check(cancelled: &AtomicBool) -> anyhow::Result<()> {
    if cancelled.load(Ordering::Relaxed) {
        return Err(crate::dto::operation_error(
            "CANCELLED",
            "Timing query cancelled",
        ));
    }
    Ok(())
}
fn analyze_impl(input: AnalyzeInput<'_>, cancelled: &AtomicBool) -> anyhow::Result<Analysis> {
    check(cancelled)?;
    let mut result = Analysis {
        method: crate::timing_dto::METHOD_VERSION,
        state: State::Unknown,
        start: None,
        end: None,
        native_wall_clock_ms: None,
        derived_wall_clock_ms: None,
        native_ttft_ms: None,
        boundary_delta_ms: None,
        intervals: intervals::analyze(None, &[], &[], input.budget.lifecycle_records),
        operation_coverage: OperationCoverage {
            method_version: 1,
            endpoint_method_version: operation_association::endpoints::METHOD_VERSION,
            ..OperationCoverage::default()
        },
        repeated_behavior: super::repeats::Projection::default(),
        category_union_ms: [None; intervals::CATEGORY_COUNT],
        context: None,
        response_gap_support: ResponseGapSupport::UnsupportedMissingBatchAndCycleEvidence,
        response_gap_union_ms: None,
        first_content_record_delay_ms: None,
        coverage: Coverage {
            supplied_events: input.events.len(),
            supplied_measurements: input.measurements.len(),
            ..Coverage::default()
        },
        issues: Vec::new(),
    };
    if [input.source, input.thread, input.turn]
        .iter()
        .any(|id| id.is_empty())
    {
        result.issues.push(Issue::InvalidScope);
        return Ok(result);
    }
    if input.events.len() > input.budget.events
        || input.measurements.len() > input.budget.measurements
    {
        result.coverage.partial = true;
        result.operation_coverage.partial = true;
        result.operation_coverage.budget_exceeded = true;
        result.repeated_behavior.coverage.budget_exceeded = true;
        result.repeated_behavior.coverage.partial = true;
        result.issues.push(Issue::ResourceLimit);
        return Ok(result);
    }
    let events: Vec<_> = input
        .events
        .iter()
        .filter(|event| {
            let source = event.position().source_instance_id.as_ref() == input.source;
            let exact = source
                && event.thread_id() == Some(input.thread)
                && event.turn_id() == Some(input.turn);
            if exact {
                result.coverage.scoped_events += 1;
            } else if source
                && (event.thread_id().is_none()
                    || (event.thread_id() == Some(input.thread) && event.turn_id().is_none()))
            {
                result.coverage.unassigned_events += 1;
            } else {
                result.coverage.outside_events += 1;
            }
            exact
        })
        .cloned()
        .collect();
    let measurements: Vec<_> = input
        .measurements
        .iter()
        .filter(|value| {
            let exact = value.source_instance_id.as_ref() == input.source
                && value.thread_id.as_deref() == Some(input.thread)
                && value.turn_id.as_deref() == Some(input.turn);
            if exact {
                result.coverage.scoped_measurements += 1;
            } else {
                result.coverage.outside_measurements += 1;
            }
            exact
        })
        .cloned()
        .collect();
    let mut domains = BTreeMap::new();
    for event in &events {
        check(cancelled)?;
        let position = event.position();
        let span = domains
            .entry((position.file_id.as_ref(), position.generation.as_ref()))
            .or_insert((position.byte_offset, position.byte_offset));
        span.0 = span.0.min(position.byte_offset);
        span.1 = span.1.max(position.byte_offset);
    }
    // Unassigned source gaps are control facts, never target-turn observations.
    // Retain only breaks inside this turn's observed physical source spans.
    let discontinuities: Vec<_> = input
        .events
        .iter()
        .filter(|event| {
            let position = event.position();
            position.source_instance_id.as_ref() == input.source
                && event.turn_id().is_none()
                && event
                    .thread_id()
                    .is_none_or(|thread| thread == input.thread)
                && !event.gaps().is_empty()
                && domains
                    .get(&(position.file_id.as_ref(), position.generation.as_ref()))
                    .is_some_and(|(start, end)| {
                        *start <= position.byte_offset && position.byte_offset <= *end
                    })
        })
        .cloned()
        .collect();
    for event in &discontinuities {
        check(cancelled)?;
        result.issues.push(Issue::SourceGap(event.id().to_owned()));
        if event.gaps().contains(&Gap::SourcePartial) {
            result.coverage.partial = true;
        }
    }
    result.context = Some(context::summarize_events_cancellable(
        &measurements,
        &events,
        &discontinuities,
        cancelled,
    )?);
    let mut boundaries = BoundaryReducer::default();
    let lifecycle_records = events
        .iter()
        .filter(|event| event_category(event).is_some())
        .count();
    for event in &events {
        check(cancelled)?;
        if let Payload::Message { origin, .. } = event.payload() {
            // One physical safe record, independent of content presence/native message identity.
            match origin {
                MessageOrigin::UserInput => result.coverage.user_input_records += 1,
                MessageOrigin::UserUnclassified => result.coverage.unclassified_user_records += 1,
                MessageOrigin::InjectedContext => result.coverage.injected_context_records += 1,
                MessageOrigin::Reasoning => result.coverage.reasoning_message_records += 1,
                _ => {}
            }
        }
        if !event.gaps().is_empty() {
            result.issues.push(Issue::SourceGap(event.id().to_owned()));
            if event.gaps().contains(&Gap::SourcePartial) {
                result.coverage.partial = true;
            }
        }
        if boundaries.observe(event) {
            result
                .issues
                .push(Issue::MissingBoundaryTime(event.id().to_owned()));
        }
    }
    for event in &events {
        check(cancelled)?;
        if let Some(index) = event_category(event) {
            result.coverage.lifecycle_candidates[index] += 1;
        }
    }
    let observations = if lifecycle_records <= input.budget.lifecycle_records {
        operation_association::resolve(events.iter().map(Arc::as_ref), cancelled)?
            .phases
            .into_iter()
            .filter(|phase| phase_category(phase.kind).is_some() || phase_operation(phase.kind))
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    let endpoints = operation_association::endpoints::reduce(&observations, cancelled)?;
    result.operation_coverage.endpoint_method_version = endpoints.method_version;
    for &index in &endpoints.missing_identity {
        if phase_operation(observations[index].kind) {
            result.operation_coverage.missing_identity += 1;
        }
        let observation = &observations[index];
        if phase_category(observation.kind).is_some() {
            result.coverage.missing_identity_lifecycles += 1;
            result.issues.push(Issue::MissingItemIdentity(
                observation.event.id().to_owned(),
            ));
        }
    }
    for &index in &endpoints.missing_time {
        let observation = &observations[index];
        if phase_category(observation.kind).is_some() {
            result
                .issues
                .push(Issue::MissingItemTime(observation.event.id().to_owned()));
        }
    }
    let boundary = boundaries.finish();
    if boundary.clock_domain_limit {
        result.coverage.partial = true;
        result.issues.push(Issue::ResourceLimit);
    }
    result.coverage.boundary_candidates = boundary.candidates;
    result.state = boundary.state;
    result.start = boundary.start;
    result.end = boundary.end;
    result.native_wall_clock_ms = boundary.native_wall_clock_ms;
    result.native_ttft_ms = boundary.native_ttft_ms;
    if boundary.boundary_conflict
        || boundary.native_duration_conflict
        || boundary.native_ttft_conflict
    {
        result.issues.push(Issue::BoundaryConflict);
    }
    let window = result
        .start
        .as_ref()
        .zip(result.end.as_ref())
        .and_then(|(start, end)| {
            if end.timestamp_ms < start.timestamp_ms {
                result.issues.push(Issue::ReversedBoundary);
                None
            } else {
                Some(intervals::Window {
                    start_ms: start.timestamp_ms,
                    end_ms: end.timestamp_ms,
                })
            }
        });
    result.derived_wall_clock_ms =
        window.map(|window| (i128::from(window.end_ms) - i128::from(window.start_ms)) as u64);
    result.boundary_delta_ms = result
        .native_wall_clock_ms
        .zip(result.derived_wall_clock_ms)
        .map(|(native, derived)| i128::from(native) - i128::from(derived));
    content::first_record(&events, &discontinuities, &mut result, cancelled)?;
    if lifecycle_records > input.budget.lifecycle_records {
        result.coverage.partial = true;
        result.issues.push(Issue::ResourceLimit);
        result.intervals = intervals::analyze(window, &[], &[], input.budget.lifecycle_records);
        result.intervals.partial = true;
        result.intervals.covered_ms = None;
        result.intervals.unclassified_ms = None;
        result.intervals.coverage_ratio = None;
        result.intervals.mask_ms = [0; intervals::MASK_COUNT];
        result
            .intervals
            .issues
            .push(intervals::Issue::ResourceLimit);
        result.operation_coverage.partial = true;
        result.operation_coverage.budget_exceeded = true;
        result.repeated_behavior.coverage.budget_exceeded = true;
        result.repeated_behavior.coverage.partial = true;
        return Ok(result);
    }
    result.repeated_behavior = super::repeats::project(
        &events,
        &discontinuities,
        &observations,
        &endpoints,
        window,
        super::repeats::Budget {
            operations: input.budget.lifecycle_records,
            ..Default::default()
        },
        cancelled,
    )?;
    let mut mapped = Vec::new();
    let mut operation_intervals = Vec::new();
    for endpoint in endpoints.groups {
        check(cancelled)?;
        let mut categories = BTreeSet::new();
        let mut target_conflict = false;
        let mut is_operation = false;
        for &index in &endpoint.indices {
            is_operation |= phase_operation(observations[index].kind);
            let observation = &observations[index];
            if let Some(category) = phase_category(observation.kind) {
                categories.insert(category);
            }
            target_conflict |= observation.target_conflict;
        }
        let kind_conflict = endpoint.kind_conflict
            || (is_operation && (categories.contains(&1) || categories.contains(&2)));
        if is_operation {
            use crate::timing_dto::OperationDiagnosticReason as Reason;
            let mut reasons = Vec::new();
            if endpoint.identity_conflict {
                reasons.push(Reason::IdentityConflict);
            }
            if endpoint.time_conflict {
                reasons.push(Reason::TimeConflict);
            }
            if kind_conflict || categories.len() > 1 {
                reasons.push(Reason::KindConflict);
            }
            if target_conflict {
                reasons.push(Reason::TargetConflict);
            }
            if endpoint.terminal.state == TerminalState::Conflicting {
                reasons.push(Reason::OutcomeConflict);
            }
            if !reasons.is_empty() {
                result.operation_coverage.diagnostic_total += 1;
                if result.operation_coverage.diagnostics.len() < 8 {
                    let indices = endpoint
                        .indices
                        .iter()
                        .take(2)
                        .chain(endpoint.indices.iter().rev().take(2))
                        .copied()
                        .collect::<BTreeSet<_>>();
                    let mut event_refs = Vec::new();
                    let mut evidence = Vec::new();
                    for index in indices {
                        let e = observations[index].event;
                        event_refs.push(format!("event:{}", e.id()));
                        if let Payload::Operation { value, .. } = e.payload()
                            && let Some(position) = value.evidence.first()
                            && !evidence.contains(position)
                        {
                            evidence.push(position.clone());
                        }
                    }
                    result.operation_coverage.diagnostics.push(
                        crate::timing_dto::OperationDiagnostic {
                            operation_id: endpoint.identity.clone(),
                            reasons,
                            observation_count: endpoint.indices.len(),
                            event_refs,
                            evidence,
                        },
                    );
                }
            }
        }
        let identity = intervals::Identity {
            source: endpoint.scope.source.to_owned(),
            task: input.thread.to_owned(),
            turn: input.turn.to_owned(),
            item: endpoint.identity,
        };
        if is_operation {
            result.operation_coverage.candidates += 1;
            if endpoint.identity_conflict
                || kind_conflict
                || endpoint.time_conflict
                || categories.len() > 1
            {
                result.operation_coverage.conflicting += 1;
            } else {
                if endpoint
                    .start_ms
                    .zip(endpoint.end_ms)
                    .is_some_and(|(start, end)| end >= start)
                {
                    result.operation_coverage.paired += 1;
                }
                operation_intervals.push(intervals::LifecycleInterval {
                    identity: identity.clone(),
                    category: intervals::Category::Command,
                    start_ms: endpoint.start_ms,
                    end_ms: endpoint.end_ms,
                    evidence_ids: endpoint.evidence_ids.clone(),
                });
            }
        }
        if categories.is_empty() {
            continue;
        }
        if target_conflict {
            result
                .issues
                .push(Issue::TargetConflict(identity.item.clone()));
        }
        if endpoint.terminal.state == TerminalState::Conflicting {
            result
                .issues
                .push(Issue::OutcomeConflict(identity.item.clone()));
        }
        if endpoint.identity_conflict
            || kind_conflict
            || endpoint.time_conflict
            || categories.len() != 1
        {
            result.coverage.conflicting_lifecycles += 1;
            result.issues.push(Issue::IdentityConflict(identity.item));
            continue;
        }
        let index = *categories.first().unwrap();
        if endpoint.unmatched_clock_domain {
            result
                .issues
                .push(Issue::UnmatchedClockDomain(identity.item.clone()));
        }
        if endpoint
            .start_ms
            .zip(endpoint.end_ms)
            .is_some_and(|(start, end)| end >= start)
        {
            result.coverage.linked_lifecycles[index] += 1;
        }
        mapped.push(intervals::LifecycleInterval {
            identity,
            category: category_at(index),
            start_ms: endpoint.start_ms,
            end_ms: endpoint.end_ms,
            evidence_ids: endpoint.evidence_ids,
        });
    }
    let operation_records = observations
        .iter()
        .filter(|phase| phase_operation(phase.kind))
        .count();
    let operation_metrics = intervals::analyze_cancellable(
        window,
        &operation_intervals,
        &[],
        input.budget.lifecycle_records,
        cancelled,
    )?;
    result.operation_coverage.computed = true;
    result.operation_coverage.budget_exceeded =
        operation_records > input.budget.lifecycle_records || operation_metrics.partial;
    result.operation_coverage.partial = result.coverage.partial
        || result.operation_coverage.candidates != result.operation_coverage.paired
        || result.operation_coverage.missing_identity > 0
        || operation_records > input.budget.lifecycle_records
        || operation_metrics.partial;
    if operation_records <= input.budget.lifecycle_records {
        result.operation_coverage.covered_ms = operation_metrics.covered_ms;
        result.operation_coverage.residual_ms = operation_metrics.unclassified_ms;
    }
    if result.operation_coverage.residual_ms.is_some() {
        result.operation_coverage.residual_range_count = operation_metrics.timeline.gap_count;
        result.operation_coverage.residual_ranges = operation_metrics.timeline.gaps;
    }
    result.operation_coverage.detail_limited = operation_metrics.timeline.limited;
    result.intervals = intervals::analyze_cancellable(
        window,
        &mapped,
        &[],
        input.budget.lifecycle_records,
        cancelled,
    )?;
    result.category_union_ms = std::array::from_fn(|index| {
        (result.intervals.observed_window_ms.is_some()
            && !result.intervals.partial
            && result.intervals.complete_intervals[index] > 0)
            .then_some(result.intervals.category_union_ms[index])
    });
    check(cancelled)?;
    Ok(result)
}

mod content;

#[cfg(test)]
#[path = "analysis/tests.rs"]
mod tests;
