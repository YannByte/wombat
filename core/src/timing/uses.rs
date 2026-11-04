//! Whole-turn object uses over canonical operations. No event pairing or configuration reads.
//! Counts describe collected evidence; even known zero never proves an object unused.
use crate::{
    adapters::contract::{Operation, SourceReport, Thread},
    dto::operation_error,
    usage_observations::{self, Coverage, Projection, TimeBasis, UseKind},
};
use anyhow::Result;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

/// None means the committed metadata cannot establish unassigned operation coverage.
/// Some(0) applies only to the collected source scope, never unlogged activity.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct MembershipCoverage {
    pub skill: Option<usize>,
    pub mcp: Option<usize>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SourceCoverage {
    Complete,
    Partial,
    Unknown,
}
#[derive(Clone, Copy, Debug)]
pub(crate) struct Budget {
    pub operations: usize,
    /// Cumulative target, canonical identity and timestamp strings inspected,
    /// including replay candidates and strings copied by the shared projection.
    pub string_bytes: usize,
}
impl Default for Budget {
    fn default() -> Self {
        Self {
            operations: 100_000,
            string_bytes: 64 * 1024 * 1024,
        }
    }
}
pub(crate) struct Input<'a> {
    pub thread: &'a Thread,
    pub turn: &'a str,
    pub source: Option<&'a SourceReport>,
    pub operations: &'a [Arc<Operation>],
    pub membership: MembershipCoverage,
    pub budget: Budget,
    pub cancelled: &'a AtomicBool,
}
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum ObjectKey {
    Skill {
        source: String,
        path: String,
    },
    Mcp {
        source: String,
        project: Option<String>,
        server: String,
    },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum UseState {
    Used,
    Candidate,
    Unclassified,
}
#[derive(Debug)]
pub(crate) struct Record<'a> {
    pub operation: &'a Arc<Operation>,
    pub kind: Option<UseKind>,
    pub state: UseState,
    pub object: Option<usize>,
    pub objects: Vec<usize>,
    pub unbound_target: bool,
    pub identity_known: bool,
    pub time_basis: TimeBasis,
    pub replay_of: Option<usize>,
    pub target_conflict: bool,
}
#[derive(Debug)]
pub(crate) struct ObjectUses {
    pub key: Arc<ObjectKey>,
    /// Identified canonical uses observed for this object, even when other records have gaps.
    pub associated_use_count: Option<u64>,
    /// Whole-turn count needs observed unassigned-membership coverage as well.
    pub use_count: Option<u64>,
    /// Gaps in associated records only; unassigned-turn coverage is separate.
    pub coverage: Coverage,
    pub unassigned_turn_records: Option<usize>,
    pub records: Vec<usize>,
}
#[derive(Debug)]
pub(crate) struct TurnUses<'a> {
    pub method_version: u32,
    pub source_coverage: SourceCoverage,
    pub membership: MembershipCoverage,
    pub objects: Vec<ObjectUses>,
    pub records: Vec<Record<'a>>,
    pub unassigned_records: Vec<usize>,
    /// Associated-record gaps; membership None must never become a zero turn-gap metric.
    pub coverage: Coverage,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Family {
    Skill,
    Mcp,
}
fn family(key: &ObjectKey) -> Family {
    match key {
        ObjectKey::Skill { .. } => Family::Skill,
        ObjectKey::Mcp { .. } => Family::Mcp,
    }
}
struct ObjectBuilder {
    key: Arc<ObjectKey>,
    projection: Projection,
    records: Vec<usize>,
    target_conflicts: BTreeSet<usize>,
    dispatch_gaps: BTreeSet<usize>,
}
fn check(cancelled: &AtomicBool) -> Result<()> {
    if cancelled.load(Ordering::Relaxed) {
        return Err(operation_error("CANCELLED", "使用投影已取消"));
    }
    Ok(())
}
fn limit() -> anyhow::Error {
    operation_error("RESOURCE_LIMIT", "使用投影超过资源预算")
}
fn charge(remaining: &mut usize, bytes: usize) -> Result<()> {
    *remaining = remaining.checked_sub(bytes).ok_or_else(limit)?;
    Ok(())
}
fn target(
    input: &Input<'_>,
    operation: &Operation,
    family: Family,
    raw: Option<&str>,
    base: Option<&str>,
    remaining: &mut usize,
) -> Result<Option<ObjectKey>> {
    charge(remaining, input.thread.source_instance_id.len())?;
    match family {
        Family::Skill => {
            let Some(raw) = raw.filter(|p| !p.is_empty()) else {
                return Ok(None);
            };
            charge(remaining, raw.len())?;
            if let Some(base) = base {
                charge(remaining, base.len())?;
            }
            let Some(path) = usage_observations::normalized_path(raw, base) else {
                return Ok(None);
            };
            Ok(Some(ObjectKey::Skill {
                source: input.thread.source_instance_id.clone(),
                path,
            }))
        }
        Family::Mcp => {
            let Some(server) = operation.server.as_deref().filter(|s| !s.is_empty()) else {
                return Ok(None);
            };
            charge(remaining, server.len())?;
            if let Some(project) = &input.thread.project {
                charge(remaining, project.len())?;
            }
            Ok(Some(ObjectKey::Mcp {
                source: input.thread.source_instance_id.clone(),
                project: input.thread.project.clone(),
                server: server.into(),
            }))
        }
    }
}

pub(crate) fn project(input: Input<'_>) -> Result<TurnUses<'_>> {
    check(input.cancelled)?;
    if input.thread.id.is_empty()
        || input.thread.source_instance_id.is_empty()
        || input.turn.is_empty()
    {
        return Err(operation_error("INVALID_ARGUMENT", "使用投影身份不能为空"));
    }
    if input
        .source
        .is_some_and(|s| s.source.id != input.thread.source_instance_id)
    {
        return Err(operation_error(
            "INVALID_ARGUMENT",
            "使用投影来源身份不匹配",
        ));
    }
    if input.operations.len() > input.budget.operations.min(100_000) {
        return Err(limit());
    }
    let mut remaining = input.budget.string_bytes.min(64 * 1024 * 1024);
    let mut objects: Vec<ObjectBuilder> = vec![];
    let mut object_keys = BTreeMap::new();
    let mut records: Vec<Record<'_>> = vec![];
    let mut identities = BTreeMap::new();
    let mut unassigned_records = vec![];
    let mut coverage = Coverage::default();
    let mut unknown_skill_targets = BTreeSet::new();
    let mut unknown_mcp_targets = BTreeSet::new();
    let mut target_conflicts = BTreeSet::new();
    let mut dispatch_gaps = BTreeSet::new();
    let mut time_gaps = BTreeSet::new();
    for operation in input.operations {
        check(input.cancelled)?;
        if operation.thread_id.as_ref() != input.thread.id
            || operation.turn_id.as_deref() != Some(input.turn)
        {
            return Err(operation_error(
                "SNAPSHOT_CORRUPT",
                "使用投影轮次归属不匹配",
            ));
        }
        let kind = usage_observations::use_kind(operation);
        let candidate = usage_observations::is_skill_read_candidate(operation);
        let unclassified = matches!(operation.kind.as_ref(), "mcpConflict" | "mcpUnclassified");
        let (family, state) = match (kind, candidate, unclassified) {
            (Some(UseKind::SkillRead), _, _) => (Family::Skill, UseState::Used),
            (Some(UseKind::McpTool | UseKind::McpResource), _, _) => (Family::Mcp, UseState::Used),
            (_, true, _) => (Family::Skill, UseState::Candidate),
            (_, _, true) => (Family::Mcp, UseState::Unclassified),
            _ => continue,
        };
        charge(&mut remaining, operation.id.len())?;
        charge(&mut remaining, operation.thread_id.len())?;
        if let Some(timestamp) = &operation.timestamp {
            charge(&mut remaining, timestamp.len())?;
        }
        let reads = usage_observations::read_targets(operation);
        let mut keys = BTreeSet::new();
        let mut unbound = false;
        if family == Family::Skill {
            if let Some(reads) = &reads {
                unbound = reads.unbound;
                for raw in &reads.paths {
                    check(input.cancelled)?;
                    if !usage_observations::is_skill_file(raw) {
                        charge(&mut remaining, raw.len())?;
                        continue;
                    }
                    let base = reads.native_cwd.unwrap_or(input.thread.project.as_deref());
                    match target(&input, operation, family, Some(raw), base, &mut remaining)? {
                        Some(key) => {
                            keys.insert(key);
                        }
                        None => unbound = true,
                    }
                }
            } else {
                unbound = true;
            }
        } else if let Some(key) = target(&input, operation, family, None, None, &mut remaining)? {
            keys.insert(key);
        } else {
            unbound = true;
        }
        let associated = keys
            .into_iter()
            .map(|key| {
                let key = Arc::new(key);
                *object_keys.entry(key.clone()).or_insert_with(|| {
                    let index = objects.len();
                    objects.push(ObjectBuilder {
                        key,
                        projection: Projection::default(),
                        records: vec![],
                        target_conflicts: BTreeSet::new(),
                        dispatch_gaps: BTreeSet::new(),
                    });
                    index
                })
            })
            .collect::<Vec<_>>();
        let object = (associated.len() == 1).then(|| associated[0]);
        let identity = usage_observations::operation_identity(operation);
        let previous = identity.and_then(|identity| identities.get(&identity).copied());
        let index = records.len();
        records.push(Record {
            operation,
            kind,
            state,
            object,
            objects: associated.clone(),
            unbound_target: unbound,
            identity_known: identity.is_some(),
            time_basis: usage_observations::time_basis(operation),
            replay_of: previous,
            target_conflict: false,
        });
        for object in &associated {
            objects[*object].records.push(index);
        }
        if unbound {
            unassigned_records.push(index);
        }
        // Gaps belong to the canonical identity, including replay evidence. Record them
        // before skipping a replay, so unresolved targets cannot depend on input order.
        let group = previous.unwrap_or(index);
        if usage_observations::time_basis(operation) == TimeBasis::Unknown {
            time_gaps.insert(group);
        }
        if state != UseState::Used {
            dispatch_gaps.insert(group);
            for object in &associated {
                objects[*object].dispatch_gaps.insert(group);
            }
        }
        if unbound || unclassified {
            match family {
                Family::Skill => unknown_skill_targets.insert(group),
                Family::Mcp => unknown_mcp_targets.insert(group),
            };
        }
        if let Some(previous) = previous {
            let old = &records[previous];
            if old.objects != associated
                || old.unbound_target != unbound
                || old.kind != kind
                || old.state != state
            {
                records[index].target_conflict = true;
                records[previous].target_conflict = true;
                target_conflicts.insert(group);
                for object in records[previous].objects.iter().chain(&associated) {
                    objects[*object].target_conflicts.insert(group);
                }
            }
            continue;
        }
        if let Some(identity) = identity {
            identities.insert(identity, index);
        }
        if identity.is_none() {
            coverage.identity_gaps += 1;
        }
        if usage_observations::time_basis(operation) == TimeBasis::Unknown {
            coverage.time_gaps += 1;
        }
    }
    for index in 0..records.len() {
        check(input.cancelled)?;
        if records[index]
            .replay_of
            .is_some_and(|prior| records[prior].target_conflict)
        {
            records[index].target_conflict = true;
        }
    }
    // Rebuild counts only after replay target conflicts are known. A conflicting
    // identity must not remain counted against whichever target happened to occur first.
    for object in &mut objects {
        check(input.cancelled)?;
        object.projection = Projection::default();
        for record_index in &object.records {
            check(input.cancelled)?;
            let record = &records[*record_index];
            if record.time_basis == TimeBasis::Unknown {
                object.projection.note_time_gap(record.operation);
            }
            if record.replay_of.is_none()
                && !record.target_conflict
                && record.state == UseState::Used
            {
                let _ = object
                    .projection
                    .observe_with_time_accounted(record.operation);
            }
        }
    }
    coverage.time_gaps = time_gaps.len();
    coverage.dispatch_gaps = dispatch_gaps.len();
    coverage.target_gaps = unknown_skill_targets.len()
        + unknown_mcp_targets
            .difference(&unknown_skill_targets)
            .count()
        + target_conflicts
            .iter()
            .filter(|group| {
                !unknown_skill_targets.contains(group) && !unknown_mcp_targets.contains(group)
            })
            .count();
    let mut out = vec![];
    for mut object in objects {
        check(input.cancelled)?;
        let unassigned = match family(&object.key) {
            Family::Skill => input.membership.skill,
            Family::Mcp => input.membership.mcp,
        };
        let unknown_targets = match family(&object.key) {
            Family::Skill => &unknown_skill_targets,
            Family::Mcp => &unknown_mcp_targets,
        };
        object.projection.coverage.target_gaps =
            unknown_targets.len() + object.target_conflicts.difference(unknown_targets).count();
        object.projection.coverage.dispatch_gaps = object.dispatch_gaps.len();
        let associated_use_count = Some(object.projection.observed_count());
        out.push(ObjectUses {
            key: object.key,
            associated_use_count,
            use_count: (unassigned == Some(0))
                .then(|| object.projection.count(false))
                .flatten(),
            coverage: object.projection.coverage,
            unassigned_turn_records: unassigned,
            records: object.records,
        });
    }
    check(input.cancelled)?;
    Ok(TurnUses {
        method_version: usage_observations::METHOD_VERSION,
        source_coverage: match input.source.map(|s| s.status.as_str()) {
            Some("complete") => SourceCoverage::Complete,
            Some("partial" | "failed" | "cancelled" | "notFound") => SourceCoverage::Partial,
            _ => SourceCoverage::Unknown,
        },
        membership: input.membership,
        objects: out,
        records,
        unassigned_records,
        coverage,
    })
}
#[cfg(test)]
mod tests;
