//! Source-wide accounting orders scalar keys on disk, then emits one fact.
//! Counter intervals use big-endian u64 keys, never floating-point SQL numbers.
use super::*;

pub(super) fn project(
    db: &Connection,
    forest: &session_relations::SessionRelations<'_>,
    report: &mut SourceReport,
    cancelled: &std::sync::atomic::AtomicBool,
) -> Result<()> {
    db.execute_batch("CREATE TEMP TABLE IF NOT EXISTS codex_ranges(thread TEXT NOT NULL,lo BLOB NOT NULL,hi BLOB NOT NULL,PRIMARY KEY(thread,lo)) WITHOUT ROWID;
        CREATE INDEX IF NOT EXISTS codex_range_ends ON codex_ranges(thread,hi,lo);
        CREATE TEMP TABLE IF NOT EXISTS codex_unbounded(thread TEXT NOT NULL,missing INTEGER NOT NULL,turn TEXT NOT NULL,PRIMARY KEY(thread,missing,turn)) WITHOUT ROWID;
        CREATE TEMP TABLE IF NOT EXISTS codex_excluded(id TEXT PRIMARY KEY) WITHOUT ROWID;
        CREATE TEMP TABLE IF NOT EXISTS codex_ancestry(thread TEXT PRIMARY KEY,preorder INTEGER NOT NULL) WITHOUT ROWID;
        DELETE FROM codex_ranges; DELETE FROM codex_excluded; DELETE FROM codex_ancestry; DELETE FROM codex_unbounded;")?;
    let mut query =
        db.prepare_cached("SELECT DISTINCT thread FROM codex_candidates WHERE thread!=''")?;
    let mut rows = query.query([])?;
    while let Some(row) = rows.next()? {
        crate::operation_association::check(cancelled)?;
        let thread: String = row.get(0)?;
        if let Some(position) = forest.owner_order(&thread) {
            db.execute(
                "INSERT INTO codex_ancestry VALUES(?1,?2)",
                params![thread, position as i64],
            )?;
        }
    }
    // Same exact fingerprint and oldest trusted ancestor; direct observations
    // are still ownership anchors but can never be removed as inherited copies.
    let mut query=db.prepare_cached("SELECT c.id,c.thread,c.fingerprint,c.direct FROM codex_candidates c JOIN codex_ancestry a ON a.thread=c.thread ORDER BY c.fingerprint,a.preorder,c.id")?;
    let mut rows = query.query([])?;
    let mut owner: Option<(String, String)> = None;
    while let Some(row) = rows.next()? {
        crate::operation_association::check(cancelled)?;
        let thread: String = row.get(1)?;
        let fingerprint: String = row.get(2)?;
        if owner.as_ref().is_some_and(|(key, ancestor)| {
            key == &fingerprint && forest.is_descendant(&thread, ancestor)
        }) {
            if !row.get::<_, bool>(3)? {
                exclude(db, row.get_ref(0)?.as_str()?)?;
            }
        } else {
            owner = Some((fingerprint, thread));
        }
    }
    // Merge half-open direct ranges once in owner/start/end order, including
    // adjacent intervals. Only the current union interval stays in memory.
    let mut query=db.prepare_cached("SELECT thread,lo,hi FROM codex_candidates c WHERE direct=1 AND thread!='' AND lo IS NOT NULL AND hi IS NOT NULL AND NOT EXISTS(SELECT 1 FROM codex_excluded x WHERE x.id=c.id) ORDER BY thread,lo,hi")?;
    let mut rows = query.query([])?;
    let mut current: Option<(String, Vec<u8>, Vec<u8>)> = None;
    while let Some(row) = rows.next()? {
        crate::operation_association::check(cancelled)?;
        let thread: String = row.get(0)?;
        let lo: Vec<u8> = row.get(1)?;
        let hi: Vec<u8> = row.get(2)?;
        if let Some((owner, _, to)) = &mut current
            && owner == &thread
            && lo <= *to
        {
            if hi > *to {
                *to = hi;
            }
        } else {
            if let Some(range) = current.take() {
                range_row(db, range)?;
            }
            current = Some((thread, lo, hi));
        }
    }
    if let Some(range) = current {
        range_row(db, range)?;
    }
    db.execute("INSERT OR IGNORE INTO codex_unbounded SELECT thread,turn IS NULL,coalesce(turn,'') FROM codex_candidates WHERE direct=1 AND (lo IS NULL OR hi IS NULL)",[])?;
    let mut query=db.prepare_cached("SELECT id,thread,turn,lo,hi,direct FROM codex_candidates c WHERE NOT EXISTS(SELECT 1 FROM codex_excluded x WHERE x.id=c.id) ORDER BY id")?;
    let mut rows = query.query([])?;
    while let Some(row) = rows.next()? {
        crate::operation_association::check(cancelled)?;
        let id: String = row.get(0)?;
        let thread: String = row.get(1)?;
        let turn: Option<String> = row.get(2)?;
        let lo: Option<Vec<u8>> = row.get(3)?;
        let hi: Option<Vec<u8>> = row.get(4)?;
        let mut excluded = false;
        if !row.get::<_, bool>(5)? {
            let unknown:bool=db.prepare_cached("SELECT CASE WHEN ?2 IS NULL THEN EXISTS(SELECT 1 FROM codex_unbounded WHERE thread=?1) ELSE EXISTS(SELECT 1 FROM codex_unbounded WHERE thread=?1 AND missing=1) OR EXISTS(SELECT 1 FROM codex_unbounded WHERE thread=?1 AND missing=0 AND turn=?2) END")?.query_row(params![thread,turn],|row|row.get(0))?;
            if unknown {
                issue(
                    report,
                    "usageCoverageUnknown",
                    "同轮存在逐响应用量，未叠加缺少覆盖依据的旧累计记录",
                    evidence(db, &id)?,
                );
                excluded = true;
            } else if let (Some(lo), Some(hi)) = (lo.as_ref(), hi.as_ref())
                && !thread.is_empty()
            {
                let next:Option<(Vec<u8>,Vec<u8>)>=db.prepare_cached("SELECT lo,hi FROM codex_ranges WHERE thread=?1 AND hi>?2 ORDER BY hi,lo LIMIT 1")?.query_row(params![thread,lo],|row|Ok((row.get(0)?,row.get(1)?))).optional()?;
                if next
                    .as_ref()
                    .is_some_and(|(from, to)| from <= lo && to >= hi)
                    && hi > lo
                {
                    excluded = true;
                } else if next.as_ref().is_some_and(|(from, _)| from < hi) {
                    issue(
                        report,
                        "usageOverlap",
                        "逐响应记录与累计区间部分重叠；该累计记录未重复计入",
                        evidence(db, &id)?,
                    );
                    excluded = true;
                }
            }
        }
        if !excluded {
            output_key(db, "measurements", &id, &thread)?;
        }
    }
    let mut writer = FactWriter::new(db, &report.source.id, "measurements");
    let mut query=db.prepare_cached("SELECT c.id,c.thread,c.payload FROM codex_candidates c INDEXED BY codex_candidate_order WHERE EXISTS(SELECT 1 FROM codex_output o WHERE o.field='measurements' AND o.id=c.id) ORDER BY c.thread,c.model,c.effort,c.hour,c.id")?;
    let mut rows = query.query([])?;
    while let Some(row) = rows.next()? {
        crate::operation_association::check(cancelled)?;
        let id: String = row.get(0)?;
        let state: MeasurementState = serde_json::from_str(row.get_ref(2)?.as_str()?)?;
        // Selection already owns membership; the writer stores no second key.
        writer.push_selected(&id, &state.candidate.measurement)?;
    }
    writer.flush()?;
    db.execute_batch(
        "DELETE FROM codex_ranges; DELETE FROM codex_excluded; DELETE FROM codex_ancestry; DELETE FROM codex_unbounded;",
    )?;
    Ok(())
}
fn exclude(db: &Connection, id: &str) -> Result<()> {
    db.prepare_cached("INSERT INTO codex_excluded VALUES(?1)")?
        .execute([id])?;
    Ok(())
}
fn range_row(db: &Connection, range: (String, Vec<u8>, Vec<u8>)) -> Result<()> {
    db.prepare_cached("INSERT INTO codex_ranges VALUES(?1,?2,?3)")?
        .execute(params![range.0, range.1, range.2])?;
    Ok(())
}

fn evidence(db: &Connection, id: &str) -> Result<Option<EvidenceRef>> {
    let payload: String = db
        .prepare_cached("SELECT payload FROM codex_candidates WHERE id=?1")?
        .query_row([id], |row| row.get(0))?;
    let state: MeasurementState = serde_json::from_str(&payload)?;
    Ok(state.candidate.measurement.evidence.first().cloned())
}
