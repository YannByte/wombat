//! Disk-backed parser working set. Recovery and canonical facts stream through
//! file-backed work relations; only one association target retains raw phases.
use super::*;
use rusqlite::{OptionalExtension, params};
use serde_json::value::RawValue;
mod accounting;
mod operations;
mod writer;
use writer::{FactWriter, output_key};
#[cfg(test)]
mod tests;

#[derive(Serialize, Deserialize)]
struct MeasurementState {
    candidate: Candidate,
    issues: Vec<Issue>,
}
#[derive(Serialize, Deserialize)]
struct Component {
    identity: facts::Prepared,
    operation: Arc<Operation>,
}

pub(super) fn prepare(db: &Connection) -> Result<()> {
    crate::live_index::prepare_fact_write(db)?;
    // Wide payloads belong in append-oriented rowid leaves. WITHOUT ROWID is
    // reserved for skinny directories; large keys amplify overflow-page I/O.
    db.execute_batch("CREATE TEMP TABLE IF NOT EXISTS changed_event_targets(target TEXT PRIMARY KEY);
        CREATE TEMP TABLE IF NOT EXISTS codex_candidates(id TEXT NOT NULL PRIMARY KEY,thread TEXT NOT NULL,payload TEXT NOT NULL,fingerprint TEXT NOT NULL,direct INTEGER NOT NULL,lo BLOB,hi BLOB,turn TEXT,issues INTEGER NOT NULL,model TEXT,effort TEXT,hour TEXT);
        CREATE INDEX IF NOT EXISTS codex_candidates_thread ON codex_candidates(thread,id);
        CREATE INDEX IF NOT EXISTS codex_candidate_order ON codex_candidates(thread,model,effort,hour,id);
        CREATE INDEX IF NOT EXISTS codex_candidate_issues ON codex_candidates(issues,id);
        CREATE INDEX IF NOT EXISTS codex_candidates_coverage ON codex_candidates(thread,direct,turn,lo,hi);
        CREATE TEMP TABLE IF NOT EXISTS codex_components(id TEXT NOT NULL PRIMARY KEY,target TEXT NOT NULL,thread TEXT NOT NULL,source TEXT NOT NULL,file TEXT NOT NULL,generation TEXT NOT NULL,offset BLOB NOT NULL,ordinal INTEGER NOT NULL,scope_thread TEXT,scope_turn TEXT,preferred TEXT NOT NULL,fallback TEXT NOT NULL,issues TEXT NOT NULL,payload TEXT NOT NULL);
        CREATE INDEX IF NOT EXISTS codex_components_thread ON codex_components(thread,id);
        CREATE TEMP TABLE IF NOT EXISTS codex_replay(id TEXT NOT NULL PRIMARY KEY,measurement TEXT NOT NULL,file TEXT NOT NULL,offset BLOB NOT NULL,ordinal INTEGER NOT NULL,thread TEXT NOT NULL,payload TEXT NOT NULL);
        CREATE TEMP TABLE IF NOT EXISTS codex_output(field TEXT NOT NULL,id TEXT NOT NULL,thread TEXT NOT NULL,payload TEXT NOT NULL,PRIMARY KEY(field,id));
        CREATE INDEX IF NOT EXISTS codex_output_thread ON codex_output(thread,field,id);
        CREATE TEMP TABLE IF NOT EXISTS codex_ids(id TEXT PRIMARY KEY) WITHOUT ROWID;
        CREATE TEMP TABLE IF NOT EXISTS codex_times(id TEXT PRIMARY KEY,at TEXT NOT NULL) WITHOUT ROWID;
        DELETE FROM codex_candidates; DELETE FROM codex_components; DELETE FROM codex_replay; DELETE FROM codex_output; DELETE FROM codex_ids; DELETE FROM codex_times;")?;
    Ok(())
}
fn stage_candidate(db: &Connection, id: &str, state: &MeasurementState) -> Result<()> {
    anyhow::ensure!(
        id == state.candidate.measurement.id,
        "raw measurement identity mismatch"
    );
    let lo = state.candidate.interval_start.map(u64::to_be_bytes);
    let hi = state.candidate.cumulative.map(u64::to_be_bytes);
    db.prepare_cached(
        "INSERT OR REPLACE INTO codex_candidates VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
    )?
    .execute(params![
        id,
        state
            .candidate
            .measurement
            .thread_id
            .as_deref()
            .unwrap_or(""),
        serde_json::to_string(state)?,
        state.candidate.fingerprint,
        state.candidate.direct,
        lo.as_ref().map(|v| v.as_slice()),
        hi.as_ref().map(|v| v.as_slice()),
        state.candidate.measurement.turn_id.as_deref(),
        state.issues.len() as i64,
        state.candidate.measurement.model.raw.as_deref(),
        state.candidate.measurement.reasoning_effort.as_deref(),
        state
            .candidate
            .measurement
            .timestamp
            .as_deref()
            .map(|at| at.chars().take(13).collect::<String>())
    ])?;
    Ok(())
}
fn stage_component(db: &Connection, id: &str, target: &str, state: &Component) -> Result<()> {
    let g = &state.identity;
    db.prepare_cached(
        "INSERT OR REPLACE INTO codex_components VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13,?14)",
    )?
    .execute(params![
        id,
        target,
        state.operation.thread_id.as_ref(),
        g.anchor.0.as_ref(),
        g.anchor.1.as_ref(),
        g.anchor.2.as_ref(),
        g.anchor.3.to_be_bytes().as_slice(),
        g.anchor.4,
        g.scope.1.as_deref(),
        g.scope.2.as_deref(),
        g.preferred,
        g.fallback,
        serde_json::to_string(&g.issues)?,
        serde_json::to_string(&StoredComponent {
            target,
            component: state
        })?
    ])?;
    Ok(())
}
#[derive(Serialize, Deserialize)]
struct StoredComponent<C = Component, T = String> {
    target: T,
    #[serde(flatten)]
    component: C,
}
pub(super) fn restore(
    db: &Connection,
    source: &SourceInstance,
    cancelled: &std::sync::atomic::AtomicBool,
) -> Result<()> {
    let scope = resume::scope(source);
    for field in ["candidates", "components"] {
        crate::live_index::each_located(db, &scope, Some(field), |_, id, payload, _| {
            crate::operation_association::check(cancelled)?;
            if field == "candidates" {
                let state: MeasurementState = serde_json::from_str(payload)?;
                anyhow::ensure!(
                    state.candidate.measurement.source_instance_id.as_ref() == source.id,
                    "raw measurement source mismatch"
                );
                stage_candidate(db, id, &state)?;
            } else {
                let stored: StoredComponent = serde_json::from_str(payload)?;
                anyhow::ensure!(
                    stored.component.identity.scope.0.as_ref() == source.id,
                    "raw component source mismatch"
                );
                stage_component(db, id, &stored.target, &stored.component)?;
            }
            Ok(())
        })?;
    }
    resume::check_counts(db, source)?;
    Ok(())
}
/// Keep original collection observations on disk during verification/replacement.
/// Only unavailable files remain in the archive before captured files are reparsed.
pub(super) fn rebuild_archive(
    db: &Connection,
    scope: &str,
    source: &SourceInstance,
    replaced: &BTreeSet<String>,
    cancelled: &std::sync::atomic::AtomicBool,
) -> Result<()> {
    let replaced: BTreeSet<_> = replaced.iter().map(|p| crate::hash(p.as_bytes())).collect();
    let mut retired = BTreeSet::new();
    crate::live_index::each_event(db, scope, |event| {
        crate::operation_association::check(cancelled)?;
        anyhow::ensure!(
            event.position().source_instance_id.as_ref() == source.id,
            "stored event source identity mismatch"
        );
        let position = event.position();
        if replaced.contains(position.file_id.as_ref()) {
            retired.insert(crate::hash(serde_json::to_vec(&(
                &position.source_instance_id,
                &position.file_id,
                &position.generation,
            ))?));
        }
        db.prepare_cached("INSERT INTO codex_times VALUES(?1,?2)")?
            .execute(params![event.id(), event.collected_at()])?;
        Ok(())
    })?;
    crate::live_index::discard_event_origins(db, scope, &retired)?;
    Ok(())
}
pub(super) fn flush(
    db: &Connection,
    scope: &str,
    facts: &mut Facts,
    cancelled: &std::sync::atomic::AtomicBool,
) -> Result<()> {
    for event in facts
        .pending_events
        .iter_mut()
        .filter(|_| facts.full_event_write)
    {
        let prior: Option<String> = db
            .prepare_cached("SELECT at FROM codex_times WHERE id=?1")?
            .query_row([event.id()], |row| row.get(0))
            .optional()?;
        if let Some(at) = prior {
            Arc::make_mut(event).retain_collection_time(at)?;
        }
    }
    crate::live_index::save_events(db, scope, &facts.pending_events, false, cancelled)?;
    facts.pending_events.clear();
    Ok(())
}

/// Reopen only changed conversations. Measurements have source+owner-scoped
/// identities, so all observations of a corrected response are reduced together.
pub(super) fn refresh(
    db: &Connection,
    scope: &str,
    facts: &mut Facts,
    report: &mut SourceReport,
    full: bool,
    replay_metadata: bool,
    cancelled: &std::sync::atomic::AtomicBool,
) -> Result<()> {
    let changed = crate::live_index::changed_event_targets(db)?;
    let threads: BTreeSet<String> = changed
        .iter()
        .map(|t| {
            serde_json::from_str::<(Option<String>, Option<String>)>(t)
                .map(|t| t.0.unwrap_or_default())
        })
        .collect::<std::result::Result<_, _>>()?;
    let origin: Option<i64> = db
        .prepare_cached("SELECT id FROM buckets WHERE scope=?1 AND field='events'")?
        .query_row([scope], |row| row.get(0))
        .optional()?;
    let targets = if full {
        None
    } else {
        let mut query=db.prepare_cached("SELECT DISTINCT target FROM event_blocks WHERE bucket=?1 AND coalesce(json_extract(target,'$[0]'),'') IN (SELECT value FROM json_each(?2)) ORDER BY target")?;
        Some(
            query
                .query_map(params![origin, serde_json::to_string(&threads)?], |row| {
                    row.get::<_, String>(0)
                })?
                .collect::<rusqlite::Result<Vec<_>>>()?,
        )
    };
    if !full {
        let encoded = serde_json::to_string(&threads)?;
        db.execute(
            "DELETE FROM codex_candidates WHERE thread IN (SELECT value FROM json_each(?1))",
            [&encoded],
        )?;
        db.execute(
            "DELETE FROM codex_components WHERE target IN (SELECT value FROM json_each(?1))",
            [serde_json::to_string(&changed)?],
        )?;
    }
    let changed_targets: std::collections::HashSet<_> = changed.iter().cloned().collect();
    let changed_owners: std::collections::HashSet<_> = changed
        .iter()
        .map(|value| {
            serde_json::from_str::<(Option<String>, Option<String>)>(value)
                .map(|(a, b)| (a.map(Arc::<str>::from), b.map(Arc::<str>::from)))
        })
        .collect::<std::result::Result<_, _>>()?;
    let source_id = report.source.id.clone();
    let mut strings = crate::session_events::EventStrings::default();
    let mut fact_strings = crate::adapters::shared_strings::FactStrings::default();
    let mut ignored = report.clone();
    ignored.issues.clear();
    // Metadata from a full replay must follow physical source traversal too.
    crate::live_index::each_event_group(
        db,
        scope,
        targets.as_deref(),
        true,
        |event| {
            crate::operation_association::check(cancelled)?;
            anyhow::ensure!(
                event.position().source_instance_id.as_ref() == source_id,
                "stored event source identity mismatch"
            );
            strings.share(Arc::make_mut(event));
            if let Some(input) = event_projection::Replay::from_event(event) {
                let measurement = match &input {
                    event_projection::Replay::Measurement { value, .. } => Some(value.id.as_str()),
                    _ => None,
                };
                if measurement.is_some() || replay_metadata {
                    let (file, offset, ordinal) = event_projection::order(event);
                    db.prepare_cached(
                        "INSERT OR REPLACE INTO codex_replay VALUES(?1,?2,?3,?4,?5,?6,?7)",
                    )?
                    .execute(params![
                        event.id(),
                        measurement.unwrap_or(""),
                        file,
                        offset.to_be_bytes().as_slice(),
                        ordinal,
                        event.thread_id().unwrap_or(""),
                        serde_json::to_string(&input)?
                    ])?;
                }
            }
            Ok(event_projection::association_input(event)
                && (full || changed_owners.contains(&event.shared_target())))
        },
        |target, events| {
            if !full && !changed_targets.contains(&target) {
                return Ok(());
            }
            let groups = Facts::prepare_associations(
                events.iter().map(AsRef::as_ref),
                &mut fact_strings,
                report,
                cancelled,
            )?;
            for (n, mut identity) in groups.into_iter().enumerate() {
                let operation = identity.operation.take().expect("operation component");
                stage_component(
                    db,
                    &format!("{}:{n}", crate::hash(&target)),
                    &target,
                    &Component {
                        identity,
                        operation,
                    },
                )?;
            }
            Ok(())
        },
    )?;
    if replay_metadata {
        let titles = std::mem::take(&mut facts.title_observations);
        *facts = Facts {
            title_observations: titles,
            streaming: true,
            external: true,
            ..Facts::default()
        };
        let mut query = db.prepare_cached(
            "SELECT payload FROM codex_replay WHERE measurement='' ORDER BY file,offset,ordinal,id",
        )?;
        let mut rows = query.query([])?;
        while let Some(row) = rows.next()? {
            crate::operation_association::check(cancelled)?;
            serde_json::from_str::<event_projection::Replay<'_>>(row.get_ref(0)?.as_str()?)?
                .apply(facts, &mut ignored);
        }
    }
    // Exact-ID reduction retains only one merged measurement and its sticky
    // conflicts. ORDER BY spills via SQLite's file-backed temporary store.
    let mut query=db.prepare_cached("SELECT measurement,payload FROM codex_replay WHERE measurement!='' ORDER BY thread,measurement,file,offset,ordinal,id")?;
    let mut rows = query.query([])?;
    let mut current = None;
    let mut local = Facts::default();
    let mut diagnostics = report.clone();
    diagnostics.issues.clear();
    while let Some(row) = rows.next()? {
        crate::operation_association::check(cancelled)?;
        let id: String = row.get(0)?;
        if current.as_ref().is_some_and(|prior| prior != &id) {
            finish_measurement(db, &mut local, &mut diagnostics)?;
        }
        current = Some(id);
        serde_json::from_str::<event_projection::Replay<'_>>(row.get_ref(1)?.as_str()?)?
            .apply(&mut local, &mut diagnostics);
    }
    finish_measurement(db, &mut local, &mut diagnostics)?;
    db.execute("DELETE FROM codex_replay", [])?;
    Ok(())
}
fn finish_measurement(db: &Connection, facts: &mut Facts, report: &mut SourceReport) -> Result<()> {
    if let Some((id, candidate)) = facts.measurements.pop_first() {
        stage_candidate(
            db,
            &id,
            &MeasurementState {
                candidate,
                issues: std::mem::take(&mut report.issues),
            },
        )?;
        facts.dirty_measurements.clear();
        facts.measurement_conflicts.clear();
    }
    Ok(())
}

pub(super) fn measurement_issues(db: &Connection) -> Result<BTreeMap<String, Vec<Issue>>> {
    #[derive(Deserialize)]
    struct Issues {
        issues: Vec<Issue>,
    }
    let mut result = BTreeMap::<String, Vec<Issue>>::new();
    let mut query =
        db.prepare_cached("SELECT payload FROM codex_candidates WHERE issues>0 ORDER BY id")?;
    let mut rows = query.query([])?;
    while let Some(row) = rows.next()? {
        for issue in serde_json::from_str::<Issues>(row.get_ref(0)?.as_str()?)?.issues {
            let file = issue.evidence.as_ref().map_or("", |e| e.file.as_ref());
            result.entry(file.to_owned()).or_default().push(issue);
        }
    }
    Ok(result)
}

/// Global identity arbitration is scalar and source ordered. Domain operation
/// bodies never accumulate while unrelated targets compete for the same ID.
pub(super) fn resolve_operations(
    db: &Connection,
    report: &mut SourceReport,
    cancelled: &std::sync::atomic::AtomicBool,
) -> Result<()> {
    let mut query=db.prepare_cached("SELECT id,thread,preferred,fallback,issues FROM codex_components ORDER BY source,file,generation,offset,ordinal,scope_thread,scope_turn,id")?;
    let mut rows = query.query([])?;
    while let Some(row) = rows.next()? {
        crate::operation_association::check(cancelled)?;
        let key: String = row.get(0)?;
        let thread: String = row.get(1)?;
        let mut id: String = row.get(2)?;
        if db
            .prepare_cached("INSERT OR IGNORE INTO codex_ids VALUES(?1)")?
            .execute([&id])?
            == 0
        {
            id = row.get(3)?;
            anyhow::ensure!(
                db.prepare_cached("INSERT OR IGNORE INTO codex_ids VALUES(?1)")?
                    .execute([&id])?
                    == 1,
                "operation identity collision"
            );
        }
        report.issues.extend(serde_json::from_str::<Vec<Issue>>(
            row.get_ref(4)?.as_str()?,
        )?);
        // The canonical-ID directory references the existing component body.
        db.prepare_cached("INSERT INTO codex_output VALUES('raw_operations',?1,?2,?3)")?
            .execute(params![id, thread, key])?;
    }
    Ok(())
}

pub(super) fn project(
    db: &Connection,
    facts: &mut Facts,
    report: &mut SourceReport,
    cancelled: &std::sync::atomic::AtomicBool,
) -> Result<()> {
    let parents = facts.parents.clone();
    let forest = session_relations::SessionRelations::new_cancellable(&parents, cancelled)?;
    if forest.unresolved > 0 {
        issue(
            report,
            "forkAncestryCycle",
            "分叉关系存在循环，保留相关事实而不推断重放",
            None,
        );
    }
    operations::project(db, facts, &forest, report, cancelled)?;
    accounting::project(db, &forest, report, cancelled)?;
    for (thread, projects) in &facts.projects {
        if projects.len() > 1 {
            if let Some(value) = facts.threads.get_mut(thread) {
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
    Ok(())
}
fn persist(db: &Connection, scope: &str, field: &str, sql: &str) -> Result<()> {
    let mut query = db.prepare_cached(sql)?;
    let mut rows = query.query([])?;
    let mut batch = Vec::new();
    let mut bytes = 0;
    while let Some(row) = rows.next()? {
        let id: String = row.get(0)?;
        let payload = RawValue::from_string(row.get(1)?)?;
        let size = id.len() + payload.get().len();
        if !batch.is_empty() && (batch.len() >= 128 || bytes + size > 256 * 1024) {
            crate::live_index::put_packed(db, scope, field, batch.drain(..))?;
            bytes = 0;
        }
        batch.push((id, payload));
        bytes += size;
    }
    crate::live_index::put_packed(db, scope, field, batch)?;
    drop(rows);
    // Membership needs keys only; selecting payloads again fetches overflow
    // pages even when the Rust caller does not inspect that result column.
    let mut ids = db.prepare_cached(&format!("SELECT id FROM ({sql})"))?;
    let mut rows = ids.query([])?;
    crate::live_index::retain_packed_streamed(
        db,
        scope,
        field,
        std::iter::from_fn(|| match rows.next() {
            Ok(Some(row)) => Some(row.get::<_, String>(0).map_err(Into::into)),
            Ok(None) => None,
            Err(error) => Some(Err(error.into())),
        }),
    )
}
pub(super) fn save(db: &Connection, source: &SourceInstance) -> Result<()> {
    let raw = resume::scope(source);
    persist(
        db,
        &raw,
        "candidates",
        "SELECT id,payload FROM codex_candidates ORDER BY thread,id",
    )?;
    persist(
        db,
        &raw,
        "components",
        "SELECT id,payload FROM codex_components ORDER BY thread,id",
    )?;
    let projection = format!("projection:{}", source.id);
    for field in ["measurements", "operations"] {
        let mut query =
            db.prepare_cached("SELECT id FROM codex_output WHERE field=?1 ORDER BY id")?;
        let mut rows = query.query([field])?;
        crate::live_index::retain_packed_streamed(
            db,
            &projection,
            field,
            std::iter::from_fn(|| match rows.next() {
                Ok(Some(row)) => Some(row.get::<_, String>(0).map_err(Into::into)),
                Ok(None) => None,
                Err(error) => Some(Err(error.into())),
            }),
        )?;
    }
    Ok(())
}
pub(super) fn clear(db: &Connection) -> Result<()> {
    db.execute_batch("DELETE FROM codex_candidates; DELETE FROM codex_components; DELETE FROM codex_replay; DELETE FROM codex_output; DELETE FROM codex_ids; DELETE FROM codex_times;")?;
    Ok(())
}
