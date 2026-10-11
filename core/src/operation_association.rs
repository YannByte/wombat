//! Source-scoped operation identities and phase links shared by domain projections.
//! Physical positions deduplicate observations; explicit aliases establish identity.
//! Times, interval calculations and use classification do not establish identity.
use crate::{
    adapters::contract::Operation,
    session_events::{Event, Gap, ItemKind, LifecycleKind, Payload, Phase, Position},
};
use anyhow::{Result, ensure};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

pub(crate) const METHOD_VERSION: u32 = 1;
pub(crate) mod endpoints;
pub(crate) mod lifecycle;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub(crate) enum Alias<'a> {
    Call(&'a str),
    Item(&'a str),
}
#[derive(Clone, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub(crate) enum OwnedAlias {
    Call(String),
    Item(String),
}
impl Alias<'_> {
    pub(crate) fn owned(self) -> OwnedAlias {
        match self {
            Self::Call(value) => OwnedAlias::Call(value.to_owned()),
            Self::Item(value) => OwnedAlias::Item(value.to_owned()),
        }
    }
}
impl<'a> Alias<'a> {
    fn raw(self) -> &'a str {
        match self {
            Self::Call(value) | Self::Item(value) => value,
        }
    }
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub(crate) struct Scope<'a> {
    pub source: &'a str,
    pub thread: Option<&'a str>,
    pub turn: Option<&'a str>,
}
impl<'a> Scope<'a> {
    fn event(event: &'a Event) -> Self {
        Self {
            source: &event.position().source_instance_id,
            thread: event.thread_id(),
            turn: event.turn_id(),
        }
    }
}
#[derive(Clone, Copy)]
pub(crate) enum ObservationKind<'a> {
    Item(&'a ItemKind),
    Compaction,
    Operation(&'a Operation),
}
#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd)]
pub(crate) enum TerminalOutcome {
    Completed,
    Failed,
    Cancelled,
    Declined,
}
pub(crate) struct ResolvedPhase<'a> {
    pub event: &'a Event,
    pub identity: Option<Arc<str>>,
    pub kind: ObservationKind<'a>,
    pub phase: &'a Phase,
    pub native_start: Option<i64>,
    pub native_end: Option<i64>,
    pub identity_conflict: bool,
    pub target_conflict: bool,
    pub outcome_conflict: bool,
    pub terminal_outcome: Option<TerminalOutcome>,
}
pub(crate) struct ResolvedGroup<'a> {
    pub id: String,
    pub anchor: &'a Event,
    pub scope: Scope<'a>,
    pub aliases: Vec<Alias<'a>>,
    pub operation_events: Vec<&'a Event>,
    pub target_conflict: bool,
    pub outcome_conflict: bool,
}
pub(crate) struct Resolution<'a> {
    pub phases: Vec<ResolvedPhase<'a>>,
    pub groups: Vec<ResolvedGroup<'a>>,
}
struct Observation<'a> {
    event: &'a Event,
    aliases: Vec<Alias<'a>>,
    kind: ObservationKind<'a>,
    phase: &'a Phase,
    native_start: Option<i64>,
    native_end: Option<i64>,
    outcome: Option<TerminalOutcome>,
}
fn operation_aliases(operation: &Operation) -> Vec<Alias<'_>> {
    [
        operation
            .call_id
            .as_deref()
            .filter(|v| !v.is_empty())
            .map(Alias::Call),
        operation
            .item_id
            .as_deref()
            .filter(|v| !v.is_empty())
            .map(Alias::Item),
    ]
    .into_iter()
    .flatten()
    .collect()
}
fn outcome(phase: &Phase) -> Option<TerminalOutcome> {
    match phase {
        Phase::Completed => Some(TerminalOutcome::Completed),
        Phase::Failed => Some(TerminalOutcome::Failed),
        Phase::Cancelled => Some(TerminalOutcome::Cancelled),
        _ => None,
    }
}
fn observation(event: &Event) -> Option<Observation<'_>> {
    let (kind, aliases, phase, native_start, native_end, terminal) = match event.payload() {
        Payload::Item {
            item_kind,
            native_id,
            phase,
            started_at_ms,
            completed_at_ms,
            ..
        } => (
            ObservationKind::Item(item_kind),
            native_id.as_deref().into_iter().map(Alias::Item).collect(),
            phase,
            *started_at_ms,
            *completed_at_ms,
            if *phase == Phase::Completed {
                None
            } else {
                outcome(phase)
            },
        ),
        Payload::Lifecycle {
            lifecycle: LifecycleKind::Compaction,
            native_id,
            phase,
            ..
        } => (
            ObservationKind::Compaction,
            native_id.as_deref().into_iter().map(Alias::Item).collect(),
            phase,
            None,
            None,
            outcome(phase),
        ),
        Payload::Operation { value, phase } => (
            ObservationKind::Operation(value),
            operation_aliases(value),
            phase,
            None,
            None,
            match value.status.as_ref() {
                "completed" => Some(TerminalOutcome::Completed),
                "failed" => Some(TerminalOutcome::Failed),
                "interrupted" | "cancelled" => Some(TerminalOutcome::Cancelled),
                "declined" => Some(TerminalOutcome::Declined),
                _ => None,
            },
        ),
        _ => return None,
    };
    Some(Observation {
        event,
        aliases,
        kind,
        phase,
        native_start,
        native_end,
        outcome: terminal,
    })
}
pub(crate) fn is_input(event: &Event) -> bool {
    matches!(
        event.payload(),
        Payload::Operation { .. }
            | Payload::Item { .. }
            | Payload::Lifecycle {
                lifecycle: LifecycleKind::Compaction,
                ..
            }
    )
}
pub(crate) fn check(cancelled: &AtomicBool) -> Result<()> {
    if cancelled.load(Ordering::Relaxed) {
        return Err(crate::dto::operation_error("CANCELLED", "操作已取消"));
    }
    Ok(())
}

/// Fixed source traversal, independent of occurrence time or arrival batch.
fn anchor(event: &Event) -> (&str, &str, &str, u64, u32) {
    let position = event.position();
    let file = match event.payload() {
        Payload::Operation { value, .. } => value.evidence.first().map(|e| e.file.as_ref()),
        _ => None,
    }
    .unwrap_or(&position.file_id);
    (
        &position.source_instance_id,
        file,
        &position.generation,
        position.byte_offset,
        position.ordinal,
    )
}
fn row(event: &Event) -> (Scope<'_>, &str, &str, u64) {
    let position = event.position();
    (
        Scope::event(event),
        &position.file_id,
        &position.generation,
        position.byte_offset,
    )
}
pub(crate) fn fallback_id(scope: Scope<'_>, position: &Position) -> String {
    crate::hash(
        serde_json::to_vec(&(
            "operation-association",
            scope.source,
            scope.thread,
            scope.turn,
            position,
        ))
        .expect("safe position"),
    )
}
fn is_mcp(operation: &Operation) -> bool {
    matches!(
        operation.kind.as_ref(),
        "mcpTool" | "mcpResource" | "mcpDiscovery" | "mcpUnclassified" | "mcpConflict"
    )
}
#[derive(Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
enum BridgeKind {
    Command,
    File,
    Mcp,
}
fn bridge_kind(operation: &Operation) -> Option<BridgeKind> {
    match operation.kind.as_ref() {
        "command" => Some(BridgeKind::Command),
        "file" => Some(BridgeKind::File),
        _ if is_mcp(operation) => Some(BridgeKind::Mcp),
        _ => None,
    }
}
/// Missing target fields do not contradict recorded fields. This same policy
/// applies to ordinary phase reconciliation and explicit inherited replays.
pub(crate) fn mcp_target_conflict(left: &Operation, right: &Operation) -> bool {
    if left.kind.as_ref() == "mcpConflict" || right.kind.as_ref() == "mcpConflict" {
        return true;
    }
    is_mcp(left)
        && is_mcp(right)
        && (left
            .server
            .as_deref()
            .zip(right.server.as_deref())
            .is_some_and(|(a, b)| a != b)
            || left
                .tool
                .as_deref()
                .zip(right.tool.as_deref())
                .is_some_and(|(a, b)| a != b)
            || left.kind != right.kind
                && left.kind.as_ref() != "mcpUnclassified"
                && right.kind.as_ref() != "mcpUnclassified")
}

/// A temporary source association pass, also used on append and replay.
/// Query callers enforce their record/string budgets; source passes retain the
/// existing full-source residency limits. Cancellation is checked in each loop.
/// O(E log E) time and O(E) auxiliary space; no additional durable event copy.
pub(crate) struct Resolver<'a> {
    observations: Vec<Observation<'a>>,
}
impl<'a> Resolver<'a> {
    pub(crate) fn new() -> Self {
        Self {
            observations: Vec::new(),
        }
    }
    pub(crate) fn observe(&mut self, event: &'a Event) {
        if let Some(observation) = observation(event) {
            self.observations.push(observation);
        }
    }
    pub(crate) fn finish(self, cancelled: &AtomicBool) -> Result<Resolution<'a>> {
        self.finish_inner(cancelled, true)
    }
    fn scope_groups(
        observations: Vec<Observation<'a>>,
        cancelled: &AtomicBool,
    ) -> Result<Vec<(&'a Event, Vec<Observation<'a>>)>> {
        let mut aliases = BTreeMap::<(Scope<'a>, Alias<'a>), Vec<(Scope<'a>, Alias<'a>)>>::new();
        let mut operation_rows = BTreeMap::new();
        for observed in &observations {
            check(cancelled)?;
            let scope = Scope::event(observed.event);
            let keys: Vec<_> = observed
                .aliases
                .iter()
                .map(|alias| (scope, *alias))
                .collect();
            for key in &keys {
                aliases
                    .entry(*key)
                    .or_default()
                    .extend(keys.iter().copied());
            }
            if let ObservationKind::Operation(operation) = observed.kind
                && let Some(kind) = bridge_kind(operation)
            {
                operation_rows
                    .entry((row(observed.event), kind))
                    .or_insert_with(Vec::new)
                    .extend(keys);
            }
        }
        // The native Item format does not distinguish item-ID from call-only fallback.
        // Only the same physical row with a matching native kind can bridge it.
        for observed in &observations {
            check(cancelled)?;
            let kind = match observed.kind {
                ObservationKind::Item(ItemKind::Mcp) => BridgeKind::Mcp,
                ObservationKind::Item(ItemKind::Command) => BridgeKind::Command,
                ObservationKind::Item(ItemKind::File) => BridgeKind::File,
                _ => continue,
            };
            for alias in &observed.aliases {
                let native = (Scope::event(observed.event), *alias);
                for operation in operation_rows
                    .get(&(row(observed.event), kind))
                    .into_iter()
                    .flatten()
                {
                    check(cancelled)?;
                    if operation.1.raw() != alias.raw() {
                        continue;
                    }
                    aliases.entry(native).or_default().push(*operation);
                    aliases.entry(*operation).or_default().push(native);
                }
            }
        }
        // Alias degree is normally one or two. One tree allocation per small
        // neighbor set otherwise dominates large source passes. Collect edges in
        // compact vectors and sort/deduplicate once, retaining O(E log E) work.
        drop(operation_rows);
        for neighbors in aliases.values_mut() {
            check(cancelled)?;
            neighbors.sort_unstable();
            neighbors.dedup();
            neighbors.shrink_to_fit();
        }
        let mut identities = BTreeMap::new();
        for first in aliases.keys().copied() {
            check(cancelled)?;
            if identities.contains_key(&first) {
                continue;
            }
            let mut pending = vec![first];
            while let Some(alias) = pending.pop() {
                check(cancelled)?;
                if identities.insert(alias, first).is_some() {
                    continue;
                }
                for next in &aliases[&alias] {
                    check(cancelled)?;
                    if !identities.contains_key(next) {
                        pending.push(*next);
                    }
                }
            }
        }
        drop(aliases);
        // Anonymous source records retain physical groups without claiming a canonical use.
        let mut groups = BTreeMap::<GroupKey<'a>, Vec<Observation<'a>>>::new();
        for observed in observations {
            check(cancelled)?;
            let scope = Scope::event(observed.event);
            let key = observed
                .aliases
                .first()
                .map_or(GroupKey::Physical(observed.event.id()), |alias| {
                    GroupKey::Alias(identities[&(scope, *alias)])
                });
            groups.entry(key).or_default().push(observed);
        }
        drop(identities);
        // Compute the preferred anchor once in a checked pass. Operation anchors
        // take priority over earlier native-only observations.
        let mut ordered = Vec::new();
        for group in groups.into_values() {
            check(cancelled)?;
            let mut first = group[0].event;
            for observed in &group {
                check(cancelled)?;
                if matches!(observed.kind, ObservationKind::Operation(_)) {
                    first = observed.event;
                    break;
                }
            }
            ordered.push((first, group));
        }
        Ok(ordered)
    }
    fn finish_inner(mut self, cancelled: &AtomicBool, phases: bool) -> Result<Resolution<'a>> {
        check(cancelled)?;
        // Sorting/deduplication are cooperative: check on either side, not in comparisons.
        // Multiple references to one safe event do not become additional observations.
        self.observations.sort_unstable_by_key(|observed| {
            (Scope::event(observed.event), anchor(observed.event))
        });
        check(cancelled)?;
        self.observations
            .dedup_by(|a, b| a.event.id() == b.event.id());
        check(cancelled)?;
        // Aliases can never connect different source/thread/turn scopes. Release
        // each scope's graph before building the next, while retaining the final
        // global anchor order for ID collision handling and phase output.
        let mut pending = self.observations.into_iter().peekable();
        let mut ordered = Vec::new();
        while let Some(first) = pending.next() {
            check(cancelled)?;
            let scope = Scope::event(first.event);
            let mut observations = vec![first];
            while pending
                .peek()
                .is_some_and(|next| Scope::event(next.event) == scope)
            {
                check(cancelled)?;
                observations.push(pending.next().expect("peeked observation"));
            }
            ordered.extend(Self::scope_groups(observations, cancelled)?);
        }
        check(cancelled)?;
        ordered.sort_unstable_by(|a, b| anchor(a.0).cmp(&anchor(b.0)));
        check(cancelled)?;
        let mut used_ids = BTreeSet::new();
        let mut resolution = Resolution {
            phases: Vec::new(),
            groups: Vec::new(),
        };
        for (first, observed) in ordered {
            check(cancelled)?;
            let scope = Scope::event(first);
            let mut id = match first.payload() {
                Payload::Operation { value, .. } if !value.id.is_empty() => value.id.clone(),
                _ => fallback_id(scope, first.position()),
            };
            // Legacy source IDs do not distinguish Call from Item. Preserve the oldest
            // component's existing ID and deterministically disambiguate other components.
            if !used_ids.insert(id.clone()) {
                id = fallback_id(scope, first.position());
                ensure!(used_ids.insert(id.clone()), "operation identity collision");
            }
            let mut servers = BTreeSet::new();
            let mut tools = BTreeSet::new();
            let mut kinds = BTreeSet::new();
            let mut terminal = lifecycle::TerminalEvidence::default();
            let mut target_conflict = false;
            let mut outcome_conflict = false;
            let mut group_aliases = BTreeSet::new();
            let mut operation_events = Vec::new();
            for phase in &observed {
                check(cancelled)?;
                group_aliases.extend(phase.aliases.iter().copied());
                if let ObservationKind::Operation(operation) = phase.kind {
                    operation_events.push(phase.event);
                    outcome_conflict |= operation.outcome_conflict;
                    if let Some(outcome) = phase.outcome {
                        terminal.observe_outcome(outcome);
                        if let Some(code) = operation.exit_code {
                            terminal.observe_code(code);
                        }
                    }
                    if is_mcp(operation) {
                        target_conflict |= operation.kind.as_ref() == "mcpConflict";
                        if let Some(server) = operation.server.as_deref() {
                            servers.insert(server);
                        }
                        if let Some(tool) = operation.tool.as_deref() {
                            tools.insert(tool);
                        }
                        if !matches!(operation.kind.as_ref(), "mcpUnclassified" | "mcpConflict") {
                            kinds.insert(operation.kind.as_ref());
                        }
                    }
                }
            }
            target_conflict |= servers.len() > 1 || tools.len() > 1 || kinds.len() > 1;
            outcome_conflict |= terminal.state() == lifecycle::TerminalState::Conflicting;
            if phases {
                let shared_identity: Arc<str> = Arc::from(id.as_str());
                for phase in observed {
                    check(cancelled)?;
                    resolution.phases.push(ResolvedPhase {
                        event: phase.event,
                        identity: (!phase.aliases.is_empty()).then(|| Arc::clone(&shared_identity)),
                        kind: phase.kind,
                        phase: phase.phase,
                        native_start: phase.native_start,
                        native_end: phase.native_end,
                        identity_conflict: phase.event.gaps().contains(&Gap::ConflictingIdentity),
                        target_conflict,
                        outcome_conflict,
                        terminal_outcome: phase.outcome,
                    });
                }
            }
            let mut retained_aliases = Vec::new();
            for alias in group_aliases {
                check(cancelled)?;
                retained_aliases.push(alias);
            }
            resolution.groups.push(ResolvedGroup {
                id,
                anchor: first,
                scope,
                aliases: retained_aliases,
                operation_events,
                target_conflict,
                outcome_conflict,
            });
        }
        check(cancelled)?;
        Ok(resolution)
    }
}
#[derive(Clone, Copy, Eq, PartialEq, Ord, PartialOrd)]
enum GroupKey<'a> {
    Alias((Scope<'a>, Alias<'a>)),
    Physical(&'a str),
}
pub(crate) fn resolve<'a>(
    events: impl IntoIterator<Item = &'a Event>,
    cancelled: &AtomicBool,
) -> Result<Resolution<'a>> {
    let mut resolver = Resolver::new();
    for event in events {
        check(cancelled)?;
        resolver.observe(event);
    }
    resolver.finish(cancelled)
}

/// Accounting consumes groups only; avoid retaining a second phase projection.
pub(crate) fn resolve_groups<'a>(
    events: impl IntoIterator<Item = &'a Event>,
    cancelled: &AtomicBool,
) -> Result<Vec<ResolvedGroup<'a>>> {
    let mut resolver = Resolver::new();
    for event in events {
        check(cancelled)?;
        resolver.observe(event);
    }
    Ok(resolver.finish_inner(cancelled, false)?.groups)
}

#[cfg(test)]
mod tests;
