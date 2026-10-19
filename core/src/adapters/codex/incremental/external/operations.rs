//! Native replay matching sorts typed alias keys on disk. Only proven links
//! enter the compact graph; each canonical body is loaded and emitted once.
use super::*;
use crate::adapters::codex::operations as domain;

pub(super) fn project(
    db: &Connection,
    facts: &mut Facts,
    forest: &session_relations::SessionRelations<'_>,
    report: &mut SourceReport,
    cancelled: &std::sync::atomic::AtomicBool,
) -> Result<()> {
    db.execute_batch("CREATE TEMP TABLE IF NOT EXISTS codex_aliases(upstream TEXT NOT NULL,kind INTEGER NOT NULL,alias TEXT NOT NULL,preorder INTEGER NOT NULL,thread TEXT NOT NULL,id TEXT NOT NULL);
        CREATE TEMP TABLE IF NOT EXISTS codex_replay_links(child TEXT NOT NULL,parent TEXT NOT NULL,PRIMARY KEY(child,parent)) WITHOUT ROWID;
        CREATE TEMP TABLE IF NOT EXISTS codex_operation_replays(id TEXT PRIMARY KEY,owner TEXT NOT NULL) WITHOUT ROWID;
        CREATE INDEX IF NOT EXISTS codex_operation_owners ON codex_operation_replays(owner,id);
        CREATE TEMP TABLE IF NOT EXISTS codex_merge_issues(id TEXT PRIMARY KEY,payload TEXT NOT NULL);
        CREATE TEMP TABLE IF NOT EXISTS codex_operation_threads(thread TEXT PRIMARY KEY,preorder INTEGER NOT NULL) WITHOUT ROWID;
        DELETE FROM codex_aliases; DELETE FROM codex_replay_links; DELETE FROM codex_operation_replays; DELETE FROM codex_operation_threads; DELETE FROM codex_merge_issues;")?;
    let missing:bool=db.query_row("SELECT EXISTS(SELECT 1 FROM codex_output o LEFT JOIN codex_components c ON c.id=o.payload WHERE o.field='raw_operations' AND c.id IS NULL)",[],|row|row.get(0))?;
    anyhow::ensure!(!missing, "raw operation reference missing");
    for thread in facts.parents.keys().chain(facts.parents.values()) {
        if let Some(preorder) = forest.owner_order(thread) {
            db.prepare_cached("INSERT OR IGNORE INTO codex_operation_threads VALUES(?1,?2)")?
                .execute(params![thread, preorder as i64])?;
        }
    }
    let mut query=db.prepare_cached("SELECT o.id,c.payload FROM codex_operation_threads t CROSS JOIN codex_output o INDEXED BY codex_output_thread ON o.thread=t.thread AND o.field='raw_operations' JOIN codex_components c ON c.id=o.payload ORDER BY o.id")?;
    let mut rows = query.query([])?;
    while let Some(row) = rows.next()? {
        crate::operation_association::check(cancelled)?;
        let id: String = row.get(0)?;
        let state: Component = serde_json::from_str(row.get_ref(1)?.as_str()?)?;
        let operation = &state.operation;
        if let Some(preorder) = forest.owner_order(&operation.thread_id)
            && let Some(turn) = operation
                .turn_id
                .as_ref()
                .and_then(|id| facts.turns.get(id.as_ref()))
        {
            for alias in state.identity.aliases.iter() {
                let (kind, alias) = match alias {
                    crate::operation_association::OwnedAlias::Call(value) => (0, value),
                    crate::operation_association::OwnedAlias::Item(value) => (1, value),
                };
                db.prepare_cached("INSERT INTO codex_aliases VALUES(?1,?2,?3,?4,?5,?6)")?
                    .execute(params![
                        turn.upstream_id,
                        kind,
                        alias,
                        preorder as i64,
                        operation.thread_id.as_ref(),
                        id
                    ])?;
            }
        }
    }
    let mut query=db.prepare_cached("SELECT upstream,kind,alias,thread,id FROM codex_aliases ORDER BY upstream,kind,alias,preorder,id")?;
    let mut rows = query.query([])?;
    type Key = (String, i64, String);
    let mut owner: Option<(Key, String, String)> = None;
    while let Some(row) = rows.next()? {
        crate::operation_association::check(cancelled)?;
        let key = (
            row.get::<_, String>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, String>(2)?,
        );
        let thread: String = row.get(3)?;
        let id: String = row.get(4)?;
        if let Some((prior, ancestor, parent)) = &owner
            && &key == prior
            && forest.is_descendant(&thread, ancestor)
        {
            db.prepare_cached("INSERT OR IGNORE INTO codex_replay_links VALUES(?1,?2)")?
                .execute(params![id, parent])?;
        } else {
            owner = Some((key, thread, id));
        }
    }
    let mut query =
        db.prepare_cached("SELECT child,parent FROM codex_replay_links ORDER BY child,parent")?;
    let mut rows = query.query([])?;
    let plan = domain::replay_plan(
        std::iter::from_fn(|| match rows.next() {
            Ok(Some(row)) => Some((|| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })()),
            Ok(None) => None,
            Err(error) => Some(Err(error.into())),
        }),
        |id| Ok(domain::owner_key(&load(db, id)?.operation, forest)),
        cancelled,
    )?;
    for (owner, id) in plan {
        db.prepare_cached("INSERT INTO codex_operation_replays VALUES(?1,?2)")?
            .execute(params![id, owner])?;
    }
    let mut query=db.prepare_cached("SELECT o.id,c.payload FROM codex_output o INDEXED BY codex_output_thread JOIN codex_components c ON c.id=o.payload WHERE o.field='raw_operations' AND NOT EXISTS(SELECT 1 FROM codex_operation_replays r WHERE r.id=o.id) ORDER BY o.thread,o.id")?;
    let mut rows = query.query([])?;
    let mut writer = FactWriter::new(db, &report.source.id, "operations");
    let mut diagnostics = report.clone();
    diagnostics.issues.clear();
    while let Some(row) = rows.next()? {
        crate::operation_association::check(cancelled)?;
        let id: String = row.get(0)?;
        let state: Component = serde_json::from_str(row.get_ref(1)?.as_str()?)?;
        let mut operation = Arc::unwrap_or_clone(state.operation);
        operation.id.clone_from(&id);
        let mut children =
            db.prepare_cached("SELECT id FROM codex_operation_replays WHERE owner=?1 ORDER BY id")?;
        let mut child_rows = children.query([&id])?;
        diagnostics.issues.clear();
        let mut merged = false;
        while let Some(child) = child_rows.next()? {
            crate::operation_association::check(cancelled)?;
            domain::merge_replay(
                &mut operation,
                &load(db, child.get_ref(0)?.as_str()?)?.operation,
                facts,
                &mut diagnostics,
                cancelled,
            )?;
            merged = true;
        }
        if merged {
            domain::finish_replay(&mut operation, cancelled)?;
        }
        if !diagnostics.issues.is_empty() {
            db.prepare_cached("INSERT INTO codex_merge_issues VALUES(?1,?2)")?
                .execute(params![id, serde_json::to_string(&diagnostics.issues)?])?;
        }
        writer.push(&id, &operation.thread_id, &operation)?;
    }
    writer.flush()?;
    // Physical write order must not change issue admission or public ordering.
    let mut issues = db.prepare_cached("SELECT payload FROM codex_merge_issues ORDER BY id")?;
    let mut rows = issues.query([])?;
    while let Some(row) = rows.next()? {
        for diagnostic in serde_json::from_str::<Vec<Issue>>(row.get_ref(0)?.as_str()?)? {
            issue(
                report,
                &diagnostic.code,
                &diagnostic.message,
                diagnostic.evidence,
            );
        }
    }
    drop(rows);
    db.execute("DELETE FROM codex_merge_issues", [])?;
    db.execute_batch("DELETE FROM codex_aliases; DELETE FROM codex_replay_links; DELETE FROM codex_operation_replays;")?;
    Ok(())
}
fn load(db: &Connection, id: &str) -> Result<Component> {
    let payload: String = db
        .prepare_cached("SELECT c.payload FROM codex_output o JOIN codex_components c ON c.id=o.payload WHERE o.field='raw_operations' AND o.id=?1")?
        .query_row([id], |row| row.get(0))?;
    let mut state: Component = serde_json::from_str(&payload)?;
    Arc::make_mut(&mut state.operation).id = id.to_owned();
    Ok(state)
}
