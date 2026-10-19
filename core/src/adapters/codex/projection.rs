//! Canonical output borrows parser facts and owns only selections and changed operations.
//! Recovery candidates, fingerprints and their lookup trees are never copied for publication.
use super::*;

pub(super) struct Projection {
    pub metadata: Facts,
    pub measurements: Vec<Arc<Measurement>>,
    pub operations: Vec<Arc<Operation>>,
    pub events: Vec<Arc<crate::session_events::Event>>,
}

pub(super) fn build(
    facts: &mut Facts,
    report: &mut SourceReport,
    cancelled: &std::sync::atomic::AtomicBool,
) -> anyhow::Result<Projection> {
    let check = crate::operation_association::check;
    check(cancelled)?;
    facts.resolve_operations(report, cancelled)?;
    let forest = session_relations::SessionRelations::new_cancellable(&facts.parents, cancelled)?;
    if forest.unresolved > 0 {
        issue(
            report,
            "forkAncestryCycle",
            "分叉关系存在循环，保留相关事实而不推断重放",
            None,
        );
    }
    let inherited = facts.inherited_measurements(&forest, cancelled)?;
    let mut metadata = Facts {
        watermarks: facts.watermarks.clone(),
        title_observations: facts.title_observations.clone(),
        threads: facts.threads.clone(),
        turns: facts.turns.clone(),
        ..Facts::default()
    };
    let operations = facts.project_operations(&mut metadata, &forest, report, cancelled)?;
    let covered = facts.direct_exclusions(&inherited, report, cancelled)?;
    let mut measurements = Vec::new();
    for (id, candidate) in &facts.measurements {
        check(cancelled)?;
        if !inherited.contains(id.as_str()) && !covered.contains(id.as_str()) {
            measurements.push(Arc::clone(&candidate.measurement));
        }
    }
    for (thread, projects) in &facts.projects {
        check(cancelled)?;
        if projects.len() > 1 {
            if let Some(value) = metadata.threads.get_mut(thread) {
                value.project = None;
            }
            issue(
                report,
                "projectConflict",
                "对话记录涉及多个工作目录，未推断单一项目",
                None,
            );
        }
    }
    check(cancelled)?;
    Ok(Projection {
        metadata,
        measurements,
        operations,
        events: vec![],
    })
}
