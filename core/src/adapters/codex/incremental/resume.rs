//! The recovery header is a directory, never a materialized source ledger.
use super::*;

pub(super) fn scope(source: &SourceInstance) -> String {
    format!("{}:resume", fact_scope(source))
}
#[derive(Serialize, Deserialize)]
struct Header {
    source: String,
    candidates: i64,
    components: i64,
    metadata: Facts,
}
pub(super) fn validate(db: &Connection, source: &SourceInstance) -> Result<bool> {
    let scope = scope(source);
    if !crate::live_index::has_scope(db, &scope)? {
        return Ok(false);
    }
    match crate::live_index::scalar(db, &scope, "version")?.and_then(|v| v.as_i64()) {
        Some(2) => Ok(true),
        Some(_) => Err(crate::dto::operation_error(
            "UNSUPPORTED_VERSION",
            "Unsupported parser resume state",
        )),
        None => Err(crate::dto::operation_error(
            "INDEX_UNAVAILABLE",
            "Parser resume version missing",
        )),
    }
}
pub(super) fn load(db: &Connection, source: &SourceInstance) -> Result<Option<Facts>> {
    if !validate(db, source)? {
        return Ok(None);
    }
    let header: Header = crate::live_index::scalar_as(db, &scope(source), "header")?
        .ok_or_else(|| anyhow::anyhow!("Parser resume header missing"))?;
    anyhow::ensure!(header.source == source.id, "resume source mismatch");
    let mut facts = header.metadata;
    anyhow::ensure!(
        facts.measurements.is_empty()
            && facts.operations.is_empty()
            && facts.events.is_empty()
            && facts.measurement_conflicts.is_empty(),
        "resume directory contains domain bodies"
    );
    facts.streaming = true;
    facts.external = true;
    Ok(Some(facts))
}
pub(super) fn check_counts(db: &Connection, source: &SourceInstance) -> Result<()> {
    #[derive(Deserialize)]
    struct Counts {
        candidates: i64,
        components: i64,
    }
    let header: Counts = crate::live_index::scalar_as(db, &scope(source), "header")?
        .ok_or_else(|| anyhow::anyhow!("Parser resume header missing"))?;
    let actual: (i64, i64) = db.query_row(
        "SELECT (SELECT count(*) FROM codex_candidates),(SELECT count(*) FROM codex_components)",
        [],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    anyhow::ensure!(
        actual == (header.candidates, header.components),
        "resume state incomplete"
    );
    Ok(())
}
pub(super) fn save(db: &Connection, source: &SourceInstance, facts: &Facts) -> Result<()> {
    #[derive(Serialize)]
    struct BorrowedHeader<'a> {
        source: &'a str,
        candidates: i64,
        components: i64,
        metadata: &'a Facts,
    }
    anyhow::ensure!(
        facts.measurements.is_empty() && facts.operations.is_empty() && facts.events.is_empty(),
        "parser directory contains domain bodies"
    );
    let (candidates, components) = db.query_row(
        "SELECT (SELECT count(*) FROM codex_candidates),(SELECT count(*) FROM codex_components)",
        [],
        |r| Ok((r.get::<_, i64>(0)?, r.get::<_, i64>(1)?)),
    )?;
    let scope = scope(source);
    crate::live_index::put(db, &scope, "version", "", &2_u32)?;
    crate::live_index::put(
        db,
        &scope,
        "header",
        "",
        &BorrowedHeader {
            source: &source.id,
            candidates,
            components,
            metadata: facts,
        },
    )?;
    Ok(())
}
