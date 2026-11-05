//! Source matching fields reduced inside canonical identities; missing fields are not matches.
use super::*;
use crate::{
    adapters::contract::{CommandSource, MatchGap, WorkData, WorkGap},
    operation_association::{ObservationKind, TerminalOutcome, endpoints::EndpointGroup},
    session_events::{Gap, ItemKind, LifecycleKind, MessageOrigin, Payload, Phase},
};
type Domain<'a> = (&'a str, &'a str);
type Position = (u64, u32);
#[derive(Clone, Copy)]
struct Boundary {
    position: Position,
    time: Option<i64>,
}
pub(super) struct Candidate<'a, 'p> {
    pub endpoint: &'p EndpointGroup<'a>,
    pub receiver: &'a str,
    pub fingerprint: Option<&'a str>,
    pub targets: &'a [ReadMatchTarget],
    pub domain: Domain<'a>,
    pub phase: usize,
    pub physical_order: Position,
    pub failed: bool,
    pub succeeded: bool,
    pub duration: Option<(u64, bool)>,
    pub evidence: Vec<String>,
}
impl Candidate<'_, '_> {
    pub fn completes_before(&self, later: &Self) -> bool {
        self.endpoint
            .completion_ms
            .zip(later.endpoint.start_ms)
            .is_some_and(|(end, start)| {
                end < start || (end == start && self.physical_order < later.physical_order)
            })
    }
}
pub(super) fn elapsed(start: i64, end: i64) -> Option<u64> {
    u64::try_from(i128::from(end) - i128::from(start)).ok()
}
struct Meter {
    units: usize,
    bytes: usize,
}
impl Meter {
    fn use_field(&mut self, bytes: usize) -> bool {
        let Some(units) = self.units.checked_sub(1) else {
            return false;
        };
        let Some(remaining) = self.bytes.checked_sub(bytes) else {
            return false;
        };
        self.units = units;
        self.bytes = remaining;
        true
    }
}
fn position(event: &Event) -> Position {
    (event.position().byte_offset, event.position().ordinal)
}
fn domain(event: &Event) -> Domain<'_> {
    (&event.position().file_id, &event.position().generation)
}
fn timestamp(event: &Event) -> Option<i64> {
    event
        .time()
        .timestamp
        .as_deref()
        .and_then(|s| chrono::DateTime::parse_from_rfc3339(s).ok())
        .map(|t| t.timestamp_millis())
}
fn breaks(event: &Event) -> bool {
    event.gaps().iter().any(|gap| *gap != Gap::InvalidTimestamp)
        || matches!(
            event.payload(),
            Payload::Ancestry { .. }
                | Payload::Message {
                    origin: MessageOrigin::UserInput
                        | MessageOrigin::UserUnclassified
                        | MessageOrigin::Compaction
                        | MessageOrigin::InterAgent,
                    ..
                }
                | Payload::Lifecycle {
                    lifecycle: LifecycleKind::Compaction,
                    ..
                }
                | Payload::Item {
                    item_kind: ItemKind::Compaction | ItemKind::User,
                    ..
                }
        )
}
pub(super) fn eligible_kind(kind: ObservationKind<'_>) -> bool {
    matches!(
        kind,
        ObservationKind::Item(ItemKind::Command | ItemKind::Tool | ItemKind::Mcp)
    ) || matches!(kind, ObservationKind::Operation(op) if matches!(op.kind.as_ref(), "command" | "tool" | "mcp" | "mcpTool" | "mcpResource" | "mcpUnclassified" | "mcpDiscovery"))
}
struct Fields<'a> {
    receiver: Option<&'a str>,
    fingerprint: Option<&'a str>,
    targets: Option<&'a [ReadMatchTarget]>,
    expected_nonzero: bool,
    invalid: bool,
    unsupported: bool,
    rejected_receiver: bool,
    gap: bool,
    durations: BTreeSet<u64>,
    witnesses: BTreeSet<String>,
}
fn known<'a, T: ?Sized + PartialEq>(old: &mut Option<&'a T>, next: Option<&'a T>) -> bool {
    match (*old, next) {
        (Some(a), Some(b)) => a != b,
        (None, Some(value)) => {
            *old = Some(value);
            false
        }
        _ => false,
    }
}
fn fields<'a>(
    endpoint: &EndpointGroup<'a>,
    phases: &[ResolvedPhase<'a>],
    meter: &mut Meter,
    cancelled: &AtomicBool,
) -> Result<Option<Fields<'a>>> {
    for value in [
        Some(endpoint.identity.as_str()),
        Some(endpoint.scope.source),
        endpoint.scope.thread,
        endpoint.scope.turn,
        endpoint.clock_domain.map(|domain| domain.0),
        endpoint.clock_domain.map(|domain| domain.1),
    ]
    .into_iter()
    .flatten()
    {
        if !meter.use_field(value.len()) {
            return Ok(None);
        }
    }
    let mut out = Fields {
        receiver: None,
        fingerprint: None,
        targets: None,
        expected_nonzero: false,
        invalid: endpoint.identity_conflict
            || endpoint.time_conflict
            || endpoint.kind_conflict
            || endpoint
                .start_ms
                .zip(endpoint.completion_ms)
                .is_some_and(|(start, end)| end < start),
        unsupported: false,
        rejected_receiver: false,
        gap: false,
        durations: BTreeSet::new(),
        witnesses: endpoint
            .terminal_evidence_ids
            .iter()
            .map(|id| (*id).to_owned())
            .collect(),
    };
    let mut function_fingerprint = None;
    let mut cwd = None;
    let mut source = None;
    let mut parsed = None;
    for &index in &endpoint.indices {
        check(cancelled)?;
        let phase = &phases[index];
        if !meter.use_field(0) {
            return Ok(None);
        }
        match phase.kind {
            ObservationKind::Operation(op) => {
                if matches!(
                    phase.phase,
                    Phase::Completed | Phase::Failed | Phase::Cancelled
                ) && let Some(duration) = op.duration_ms
                    && out.durations.insert(duration)
                    && out.durations.len() <= 2
                {
                    out.witnesses.insert(phase.event.id().into());
                }
                if let Some(work) = &op.work {
                    out.gap |= !work.gaps.is_empty();
                    out.invalid |= work.gaps.contains(&WorkGap::ConflictingObservation);
                    if let WorkData::Command {
                        cwd: new_cwd,
                        source: new_source,
                        parsed_commands: new_parsed,
                    } = &work.data
                    {
                        if let Some(value) = new_cwd
                            && !meter.use_field(value.len())
                        {
                            return Ok(None);
                        }
                        for command in new_parsed.iter().flatten() {
                            check(cancelled)?;
                            if !meter.use_field(command.path().map_or(0, str::len)) {
                                return Ok(None);
                            }
                        }
                        out.invalid |= known(&mut cwd, new_cwd.as_deref());
                        out.invalid |= known(&mut source, new_source.as_ref());
                        out.invalid |= known(&mut parsed, new_parsed.as_deref());
                        out.rejected_receiver |= new_source.as_ref().is_some_and(|s| {
                            !matches!(s, CommandSource::Agent | CommandSource::UnifiedExecStartup)
                        });
                    }
                }
                let Some(matching) = &op.matching else {
                    continue;
                };
                out.gap |= !matching.gaps.is_empty();
                out.invalid |= matching.gaps.contains(&MatchGap::ConflictingObservation);
                out.unsupported |= matching.gaps.contains(&MatchGap::UnsupportedParameters)
                    || (op.kind.as_ref() != "command"
                        && matching.gaps.contains(&MatchGap::ResourceLimit));
                out.rejected_receiver |= matching.receiver_owner.is_none();
                for value in [
                    matching.receiver_owner.as_deref(),
                    matching.request_fingerprint.as_deref(),
                    matching.function_request_fingerprint.as_deref(),
                ]
                .into_iter()
                .flatten()
                {
                    if !meter.use_field(value.len()) {
                        return Ok(None);
                    }
                }
                if (out.receiver.is_none() && matching.receiver_owner.is_some())
                    || (out.fingerprint.is_none() && matching.request_fingerprint.is_some())
                    || (function_fingerprint.is_none()
                        && matching.function_request_fingerprint.is_some())
                    || (out.targets.is_none() && !matching.read_targets.is_empty())
                    || (!out.expected_nonzero && matching.expected_nonzero)
                {
                    out.witnesses.insert(phase.event.id().into());
                }
                out.invalid |= known(&mut out.receiver, matching.receiver_owner.as_deref());
                out.invalid |= known(
                    &mut out.fingerprint,
                    matching.request_fingerprint.as_deref(),
                );
                out.invalid |= known(
                    &mut function_fingerprint,
                    matching.function_request_fingerprint.as_deref(),
                );
                for target in &matching.read_targets {
                    check(cancelled)?;
                    if !meter.use_field(target.path.len()) {
                        return Ok(None);
                    }
                }
                if !matching.read_targets.is_empty() {
                    out.invalid |= known(&mut out.targets, Some(matching.read_targets.as_slice()));
                }
                out.expected_nonzero |= matching.expected_nonzero;
            }
            ObservationKind::Item(_) => {
                if let Payload::Item {
                    duration: Some(duration),
                    ..
                } = phase.event.payload()
                    && matches!(
                        phase.phase,
                        Phase::Completed | Phase::Failed | Phase::Cancelled
                    )
                    && let Some(value) = duration
                        .secs
                        .checked_mul(1000)
                        .and_then(|v| v.checked_add(u64::from(duration.nanos / 1_000_000)))
                    && out.durations.insert(value)
                    && out.durations.len() <= 2
                {
                    out.witnesses.insert(phase.event.id().into());
                }
            }
            _ => {}
        }
    }
    out.fingerprint = out.fingerprint.or(function_fingerprint);
    Ok(Some(out))
}
/// Scoped events and discontinuities are already bounded by the turn analysis.
pub(super) fn collect<'a, 'p>(
    events: &[Arc<Event>],
    controls: &[Arc<Event>],
    phases: &[ResolvedPhase<'a>],
    endpoints: &'p Endpoints<'a>,
    budget: Budget,
    coverage: &mut Coverage,
    cancelled: &AtomicBool,
) -> Result<Option<Vec<Candidate<'a, 'p>>>> {
    if endpoints.groups.len() > budget.operations.min(100_000) {
        return Ok(None);
    }
    let mut meter = Meter {
        units: budget.metadata.min(100_000),
        bytes: budget.string_bytes.min(64 * 1024 * 1024),
    };
    let mut boundaries: BTreeMap<Domain<'_>, Vec<Boundary>> = BTreeMap::new();
    let mut seen_breaks = BTreeSet::new();
    for event in events.iter().chain(controls) {
        check(cancelled)?;
        if breaks(event) && seen_breaks.insert(event.id()) {
            if !meter.use_field(0) {
                return Ok(None);
            }
            boundaries.entry(domain(event)).or_default().push(Boundary {
                position: position(event),
                time: timestamp(event),
            });
        }
    }
    // Unidentified requests may interrupt a matching chain. They cannot become a predecessor.
    for &index in &endpoints.missing_identity {
        check(cancelled)?;
        let phase = &phases[index];
        if matches!(
            phase.kind,
            ObservationKind::Operation(_)
                | ObservationKind::Item(ItemKind::Command | ItemKind::Tool | ItemKind::Mcp)
        ) {
            if !meter.use_field(0) {
                return Ok(None);
            }
            boundaries
                .entry(domain(phase.event))
                .or_default()
                .push(Boundary {
                    position: position(phase.event),
                    time: None,
                });
        }
    }
    let mut preliminary = Vec::new();
    for endpoint in &endpoints.groups {
        check(cancelled)?;
        if !endpoint
            .indices
            .iter()
            .any(|&i| eligible_kind(phases[i].kind))
        {
            continue;
        }
        coverage.candidates += 1;
        let Some(fields) = fields(endpoint, phases, &mut meter, cancelled)? else {
            return Ok(None);
        };
        if fields.gap {
            coverage.source_metadata_gaps += 1;
        }
        let receiver = fields
            .receiver
            .filter(|r| Some(*r) == endpoint.scope.thread && endpoint.scope.turn.is_some());
        let valid = !fields.invalid
            && !fields.unsupported
            && !fields.rejected_receiver
            && receiver.is_some()
            && (fields.fingerprint.is_some() || fields.targets.is_some())
            && endpoint.clock_domain.is_some();
        if !valid {
            if fields.invalid {
                coverage.conflicting += 1;
            } else if fields.rejected_receiver || receiver.is_none() {
                coverage.excluded_receivers += 1;
            } else if endpoint.clock_domain.is_none() {
                coverage.missing_clock_domain += 1;
            } else {
                coverage.missing_matching += 1;
            }
            for &index in &endpoint.indices {
                check(cancelled)?;
                if !meter.use_field(0) {
                    return Ok(None);
                }
                boundaries
                    .entry(domain(phases[index].event))
                    .or_default()
                    .push(Boundary {
                        position: position(phases[index].event),
                        time: None,
                    });
            }
            continue;
        }
        preliminary.push((endpoint, fields, receiver.unwrap()));
    }
    for list in boundaries.values_mut() {
        check(cancelled)?;
        list.sort_unstable_by_key(|b| b.position);
        list.dedup_by_key(|b| b.position);
        coverage.context_resets += list.len();
    }
    let mut out = Vec::new();
    for (endpoint, fields, receiver) in preliminary {
        check(cancelled)?;
        let domain = endpoint.clock_domain.unwrap();
        let list = boundaries.get(&domain).map_or(&[][..], Vec::as_slice);
        let first = endpoint
            .indices
            .iter()
            .map(|&i| position(phases[i].event))
            .min()
            .unwrap();
        let last = endpoint
            .indices
            .iter()
            .map(|&i| position(phases[i].event))
            .max()
            .unwrap();
        let phase = list.partition_point(|b| b.position <= first);
        let crossed = phase != list.partition_point(|b| b.position <= last)
            || phase
                .checked_sub(1)
                .and_then(|i| list[i].time)
                .zip(endpoint.start_ms)
                .is_some_and(|(boundary, start)| start < boundary)
            || list
                .get(phase)
                .and_then(|b| b.time)
                .zip(endpoint.completion_ms)
                .is_some_and(|(boundary, end)| boundary < end);
        if crossed {
            coverage.crossed_context += 1;
            continue;
        }
        if endpoint.indices.iter().any(|&i| matches!(phases[i].kind, ObservationKind::Operation(op) if op.kind.as_ref() == "command") || matches!(phases[i].kind, ObservationKind::Item(ItemKind::Command))) {
            coverage.eligible_commands += 1;
        }
        if endpoint.start_ms.is_none() {
            coverage.missing_start += 1;
        }
        let outcome = endpoint.terminal.outcome();
        if outcome.is_none() {
            coverage.indeterminate_outcomes += 1;
        }
        let code = endpoint.terminal.exit_code;
        let failed = matches!(outcome, Some(TerminalOutcome::Failed))
            || (outcome == Some(TerminalOutcome::Completed) && code.is_some_and(|c| c != 0));
        let failed = failed && !(fields.expected_nonzero && (code.is_none() || code == Some(1)));
        let succeeded = outcome == Some(TerminalOutcome::Completed) && code.is_none_or(|c| c == 0);
        let duration = if fields.durations.len() > 1 {
            coverage.duration_conflicts += 1;
            None
        } else if let Some(value) = fields.durations.first() {
            Some((*value, true))
        } else {
            endpoint
                .start_ms
                .zip(endpoint.end_ms)
                .and_then(|(start, end)| elapsed(start, end))
                .map(|v| (v, false))
        };
        let mut proof = fields.witnesses.clone();
        proof.extend(endpoint.evidence_ids.iter().cloned());
        anyhow::ensure!(proof.len() <= PROOF_REF_LIMIT, "repeat proof witness bound");
        out.push(Candidate {
            evidence: proof.into_iter().collect(),
            endpoint,
            receiver,
            fingerprint: fields.fingerprint,
            targets: fields.targets.unwrap_or(&[]),
            domain,
            phase,
            physical_order: first,
            failed,
            succeeded,
            duration,
        });
    }
    Ok(Some(out))
}
