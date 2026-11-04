//! Per-object use projections over authoritative, reconciled source operations.
//!
//! Adapters own call/result aliases and explicit fork inheritance. This projection
//! neither pairs log records nor guesses operation identity from paths or time.
use crate::adapters::contract::{
    CommandSource, MatchGap, OPERATION_MATCH_VERSION, Operation, ParsedCommand,
    WORK_OBSERVATION_VERSION, WorkData, WorkStage,
};
use std::collections::BTreeSet;
use typed_path::Utf8TypedPath;

pub(crate) const METHOD_VERSION: u32 = 4;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum UseKind {
    SkillRead,
    McpTool,
    McpResource,
}

/// Native parsed commands are display classifications, never independent dispatch proof.
/// Codex 89c8bcf37d64be69e4c8286f4541c1a84ed312a4, protocol/src/parse_command.rs.
/// Paths stay borrowed; consumers resolve only against recorded historical bases.
pub(crate) struct ReadTargets<'a> {
    pub paths: Vec<&'a str>,
    pub candidate: bool,
    pub unbound: bool,
    /// Native cwd is distinct from the task project. None cannot fall back to current cwd.
    pub native_cwd: Option<Option<&'a str>>,
}
impl ReadTargets<'_> {
    pub(crate) fn may_be_skill(&self) -> bool {
        self.unbound || self.paths.iter().any(|path| is_skill_file(path))
    }
    pub(crate) fn resolve(&self, path: &str, historical_project: Option<&str>) -> Option<String> {
        normalized_path(path, self.native_cwd.unwrap_or(historical_project))
    }
}
pub(crate) fn normalized_path(path: &str, base: Option<&str>) -> Option<String> {
    if path.is_empty() {
        return None;
    }
    let path = Utf8TypedPath::derive(path);
    let absolute = if path.is_absolute() {
        path.to_path_buf()
    } else {
        // Drive-relative and root-relative Windows paths require unrecorded
        // drive state. Ordinary relative paths use their recorded base's grammar.
        if path.is_windows() {
            return None;
        }
        let base = Utf8TypedPath::derive(base?);
        if !base.is_absolute() {
            return None;
        }
        base.join(path.as_str())
    };
    // Historical paths never consult the host OS, cwd, or filesystem.
    let mut normalized = absolute.normalize().as_str().to_owned();
    // Match the adapter's Windows drive canonicalization without folding path
    // components or consulting the current host filesystem.
    if normalized.as_bytes().get(1) == Some(&b':') && normalized.as_bytes()[0].is_ascii_alphabetic()
    {
        normalized.as_mut_str()[..1].make_ascii_lowercase();
    }
    Some(normalized)
}
pub(crate) fn read_targets(operation: &Operation) -> Option<ReadTargets<'_>> {
    if let Some(work) = &operation.work
        && let WorkData::Command {
            cwd,
            source,
            parsed_commands,
            ..
        } = &work.data
    {
        if *source == Some(CommandSource::UserShell) {
            return None;
        }
        if work.format_version != WORK_OBSERVATION_VERSION {
            return None;
        }
        let mut paths = vec![];
        let mut unbound = false;
        for command in parsed_commands.as_deref()? {
            if let ParsedCommand::Read { path } = command {
                if let Some(path) = path.as_deref().filter(|path| !path.is_empty()) {
                    paths.push(path);
                } else {
                    unbound = true;
                }
            }
        }
        if paths.is_empty() && !unbound {
            return None;
        }
        // Native execution and determinate argv/target agreement are stronger
        // than a display label. Reuse the adapter verifier, never wrapper text.
        if operation.kind.as_ref() == "command"
            && work.stage == WorkStage::Terminal
            && operation.status.as_ref() != "declined"
            && matches!(
                source,
                Some(CommandSource::Agent | CommandSource::UnifiedExecStartup)
            )
            && !unbound
            && let Some(matching) = operation.matching.as_ref().filter(|matching| {
                matching.format_version == OPERATION_MATCH_VERSION
                    && matching.request_fingerprint.is_some()
                    && !matching.read_targets.is_empty()
                    && matching
                        .gaps
                        .iter()
                        .all(|gap| *gap == MatchGap::MissingReceiver)
            })
        {
            return Some(ReadTargets {
                paths: matching
                    .read_targets
                    .iter()
                    .map(|target| target.path.as_str())
                    .collect(),
                candidate: false,
                unbound: false,
                native_cwd: Some(cwd.as_deref()),
            });
        }
        return Some(ReadTargets {
            paths,
            candidate: true,
            unbound,
            native_cwd: Some(cwd.as_deref()),
        });
    }
    if operation.kind.as_ref() == "skillRead"
        || operation.kind.as_ref() == "tool" && operation.name.as_ref() == "read_file"
    {
        let paths = operation
            .path
            .as_deref()
            .filter(|path| !path.is_empty())
            .into_iter()
            .collect();
        return Some(ReadTargets {
            paths,
            candidate: operation.name.as_ref() == "read_skill_file",
            unbound: operation.path.as_deref().is_none_or(str::is_empty),
            native_cwd: None,
        });
    }
    None
}

/// Catalogs, declarations, discovery and unresolved MCP identities are not uses.
pub(crate) fn use_kind(operation: &Operation) -> Option<UseKind> {
    use_kind_with_targets(operation, read_targets(operation).as_ref())
}
pub(crate) fn use_kind_with_targets(
    operation: &Operation,
    reads: Option<&ReadTargets<'_>>,
) -> Option<UseKind> {
    if matches!(
        operation.work.as_ref().map(|work| &work.data),
        Some(WorkData::Command {
            source: Some(CommandSource::UserShell),
            ..
        })
    ) {
        return None;
    }
    if reads.is_some_and(|targets| targets.candidate) {
        return None;
    }
    if reads.is_some_and(|targets| targets.paths.iter().any(|path| is_skill_file(path))) {
        return Some(UseKind::SkillRead);
    }
    match operation.kind.as_ref() {
        "skillRead" => Some(UseKind::SkillRead),
        "tool"
            if operation.name.as_ref() == "read_file"
                && operation.path.as_deref().is_some_and(is_skill_file) =>
        {
            Some(UseKind::SkillRead)
        }
        "mcpTool" => Some(UseKind::McpTool),
        "mcpResource" => Some(UseKind::McpResource),
        _ => None,
    }
}

pub(crate) fn is_skill_file(path: &str) -> bool {
    let parsed = Utf8TypedPath::derive(path);
    parsed.file_name() == Some("SKILL.md")
        || !parsed.is_absolute()
            && typed_path::Utf8WindowsPath::new(path).file_name() == Some("SKILL.md")
}

/// Completion of a legacy exec wrapper does not establish dispatch of a read
/// whose command was only extracted from a literal in the wrapper's input.
pub(crate) fn is_skill_read_candidate(operation: &Operation) -> bool {
    read_targets(operation).is_some_and(|targets| targets.candidate && targets.may_be_skill())
}

/// Exact canonical replay conflicts, not time proximity or anonymous record hashes.
/// Only fixed-size digests survive each record; target resolution uses the same historical bases.
pub(crate) fn target_conflicts<'a, I: Iterator<Item = &'a Operation>>(
    operations: impl Fn() -> I,
    project: impl Fn(&'a Operation) -> Option<&'a str>,
    relevant: impl Fn(&'a Operation) -> bool,
) -> BTreeSet<(&'a str, &'a str)> {
    use sha2::{Digest, Sha256};
    let related = operations()
        .filter(|operation| relevant(operation))
        .filter_map(operation_identity)
        .collect::<BTreeSet<_>>();
    let mut identities = std::collections::BTreeMap::new();
    let mut conflicts = BTreeSet::new();
    for operation in operations() {
        let Some(identity) = operation_identity(operation) else {
            continue;
        };
        if !related.contains(&identity) {
            continue;
        }
        let reads = read_targets(operation);
        let mcp = matches!(
            operation.kind.as_ref(),
            "mcpTool" | "mcpResource" | "mcpConflict" | "mcpUnclassified"
        );
        if reads.is_none() && !mcp {
            continue;
        }
        let mut hash = Sha256::new();
        let mut field = |value: &str| {
            hash.update((value.len() as u64).to_le_bytes());
            hash.update(value.as_bytes());
        };
        field(operation.kind.as_ref());
        if let Some(reads) = reads {
            field(if reads.candidate {
                "candidate"
            } else {
                "dispatched"
            });
            field(if reads.unbound { "unbound" } else { "bound" });
            let targets = reads
                .paths
                .iter()
                .map(|raw| {
                    reads
                        .resolve(raw, project(operation))
                        .unwrap_or_else(|| format!("unresolved:{raw}"))
                })
                .collect::<BTreeSet<_>>();
            for target in targets {
                field(&target);
            }
        } else {
            field(operation.server.as_deref().unwrap_or(""));
            field(project(operation).unwrap_or(""));
        }
        let fingerprint: [u8; 32] = hash.finalize().into();
        match identities.get(&identity) {
            Some(previous) if *previous != fingerprint => {
                conflicts.insert(identity);
            }
            None => {
                identities.insert(identity, fingerprint);
            }
            _ => {}
        }
    }
    conflicts
}

/// Operation currently stores the earliest observed source time. It does not
/// distinguish a dispatch from a result-only timestamp.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TimeBasis {
    SourceOperationTime,
    Unknown,
}

pub(crate) fn time_basis(operation: &Operation) -> TimeBasis {
    if operation
        .timestamp
        .as_deref()
        .is_some_and(|timestamp| chrono::DateTime::parse_from_rfc3339(timestamp).is_ok())
    {
        TimeBasis::SourceOperationTime
    } else {
        TimeBasis::Unknown
    }
}

#[derive(Debug, Default, Eq, PartialEq)]
pub(crate) struct Coverage {
    pub dispatch_gaps: usize,
    pub identity_gaps: usize,
    pub target_gaps: usize,
    pub time_gaps: usize,
    pub turn_gaps: usize,
}

/// Business identity has already been reconciled by the adapter. A physical
/// record hash alone cannot establish a distinct dispatch, read or retry.
pub(crate) fn operation_identity(operation: &Operation) -> Option<(&str, &str)> {
    (!operation.thread_id.is_empty()
        && !operation.id.is_empty()
        && [operation.call_id.as_deref(), operation.item_id.as_deref()]
            .into_iter()
            .flatten()
            .any(|identity| !identity.is_empty()))
    .then_some((operation.thread_id.as_ref(), operation.id.as_str()))
}

/// A projection belongs to one resolved object; the object is implicitly part
/// of every key. Space is O(observed operations + related turns/tasks).
#[derive(Debug, Default)]
pub(crate) struct Projection {
    operations: BTreeSet<(String, String)>,
    related_turns: BTreeSet<(String, String)>,
    related_tasks: BTreeSet<String>,
    missing_time_operations: BTreeSet<(String, String)>,
    pub coverage: Coverage,
}

impl Projection {
    /// Outcomes never subtract dispatched uses. Reliable source identities are
    /// already canonicalized; independent retries retain independent IDs.
    #[cfg(test)]
    pub(crate) fn observe(&mut self, operation: &Operation) {
        let _ = self.associate(operation);
        if time_basis(operation) == TimeBasis::Unknown {
            self.note_time_gap(operation);
        }
    }

    /// Consumers retaining every replay's time coverage call this once per canonical use.
    pub(crate) fn observe_with_time_accounted(&mut self, operation: &Operation) -> Option<u64> {
        self.associate(operation)
    }
    fn associate(&mut self, operation: &Operation) -> Option<u64> {
        let delta = if let Some((thread, id)) = operation_identity(operation) {
            Some(u64::from(
                self.operations.insert((thread.to_owned(), id.to_owned())),
            ))
        } else {
            self.coverage.identity_gaps += 1;
            None
        };
        self.related_tasks.insert(operation.thread_id.to_string());
        if let Some(turn) = operation.turn_id.as_deref().filter(|turn| !turn.is_empty()) {
            self.related_turns
                .insert((operation.thread_id.to_string(), turn.to_owned()));
        } else {
            self.coverage.turn_gaps += 1;
        }
        delta
    }

    pub(crate) fn note_time_gap(&mut self, operation: &Operation) {
        let new = operation_identity(operation).is_none_or(|(thread, id)| {
            self.missing_time_operations
                .insert((thread.into(), id.into()))
        });
        if new {
            self.coverage.time_gaps += 1;
        }
    }

    /// Missing timestamps affect date-window membership, not a known all-time
    /// operation identity. Time coverage stays visible in either case.
    pub(crate) fn count(&self, time_filtered: bool) -> Option<u64> {
        (self.coverage.dispatch_gaps == 0
            && self.coverage.identity_gaps == 0
            && self.coverage.target_gaps == 0
            && (!time_filtered || self.coverage.time_gaps == 0))
            .then_some(self.observed_count())
    }

    /// Canonical uses already located by the consumer in its selected scope.
    /// Gaps in other records do not erase these positive observations.
    pub(crate) fn observed_count(&self) -> u64 {
        self.operations.len() as u64
    }

    pub(crate) fn related_turns(&self) -> usize {
        self.related_turns.len()
    }

    pub(crate) fn related_tasks(&self) -> usize {
        self.related_tasks.len()
    }
}

#[cfg(test)]
mod tests;

/// Publish the same projection used for the scalar count; unavailable coverage is not zero.
pub(crate) fn basis(
    projection: Option<&Projection>,
    unit: crate::config_dto::UseUnit,
    scope: crate::config_dto::UseScope,
    captured_at: &str,
    snapshot: Option<&crate::usage_store::Snapshot>,
    time_filtered: bool,
) -> crate::config_dto::UseBasis {
    use crate::config_dto::{
        UseBasis, UseBasisStatus, UseCoverage, UseSourceCompleteness, UseTimeBasis,
    };
    let source_completeness = snapshot.map_or(UseSourceCompleteness::Unknown, |snapshot| {
        if scope.source_instance_ids.is_empty() {
            return UseSourceCompleteness::Unknown;
        }
        let mut partial = false;
        for id in &scope.source_instance_ids {
            let Some(report) = snapshot
                .manifest
                .sources
                .iter()
                .find(|report| &report.source.id == id)
            else {
                return UseSourceCompleteness::Unknown;
            };
            partial |= report.status != "complete" || !report.issues.is_empty();
        }
        if partial {
            UseSourceCompleteness::Partial
        } else {
            UseSourceCompleteness::Complete
        }
    });
    let gap =
        |get: fn(&Coverage) -> usize| projection.map(|projection| get(&projection.coverage) as u64);
    UseBasis {
        method_version: METHOD_VERSION,
        status: match projection {
            None => UseBasisStatus::Unavailable,
            Some(projection) if projection.count(time_filtered).is_some() => {
                UseBasisStatus::Observed
            }
            Some(_) => UseBasisStatus::Partial,
        },
        unit,
        captured_at: captured_at.into(),
        snapshot_id: snapshot.map(|snapshot| snapshot.manifest.snapshot_ref.snapshot_id.clone()),
        scope,
        time_basis: UseTimeBasis::SourceOperationTime,
        coverage: UseCoverage {
            dispatch_gaps: gap(|coverage| coverage.dispatch_gaps),
            identity_gaps: gap(|coverage| coverage.identity_gaps),
            target_gaps: gap(|coverage| coverage.target_gaps),
            time_gaps: gap(|coverage| coverage.time_gaps),
            turn_gaps: gap(|coverage| coverage.turn_gaps),
        },
        source_completeness,
    }
}
