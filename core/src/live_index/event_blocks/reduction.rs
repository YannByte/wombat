//! Sort locators on disk and release raw observations after one identity scope.
use super::*;

// json_each has no source cardinality statistics. Keep targets outside the
// frame loop so each target uses the complete existing expression index.
pub(super) const TARGET_ROWS: &str = "SELECT e.rowid,e.target FROM json_each(?2) t CROSS JOIN event_blocks e ON coalesce(json_extract(e.target,'$[0]'),'')=coalesce(json_extract(t.value,'$[0]'),'') AND coalesce(json_extract(e.target,'$[1]'),'')=coalesce(json_extract(t.value,'$[1]'),'') WHERE e.bucket=?1 ORDER BY e.target,e.origin,e.window,e.part";

pub(crate) fn each_group(
    db: &Connection,
    scope: &str,
    targets: Option<&[String]>,
    association_only: bool,
    mut observe: impl FnMut(&mut Arc<Event>) -> Result<bool>,
    mut consume: impl FnMut(String, Rows) -> Result<()>,
) -> Result<()> {
    validate_cold_source(db, scope)?;
    let origin: Option<i64> = db.prepare_cached("SELECT coalesce(l.origin,b.id) FROM buckets b LEFT JOIN event_links l ON l.bucket=b.id WHERE b.scope=?1 AND b.field='events'")?.query_row([scope], |r| r.get(0)).optional()?;
    let Some(origin) = origin else {
        return Ok(());
    };
    let sql = if targets.is_some() {
        TARGET_ROWS
    } else {
        "SELECT rowid,target FROM event_blocks WHERE bucket=?1 ORDER BY target,origin,window,part"
    };
    let mut query = db.prepare_cached(sql)?;
    let mut cursor = if let Some(targets) = targets {
        query.query(params![origin, serde_json::to_string(targets)?])?
    } else {
        query.query([origin])?
    };
    let mut block = db.prepare_cached(
        "SELECT bytes,digest,codec,count,origin,window FROM event_blocks WHERE rowid=?1",
    )?;
    let mut current = None;
    let mut retained = BTreeMap::new();
    while let Some(row) = cursor.next()? {
        let target: String = row.get(1)?;
        if current.as_ref().is_some_and(|previous| previous != &target) {
            consume(
                current.take().expect("current target"),
                std::mem::take(&mut retained).into_values().collect(),
            )?;
        }
        current = Some(target.clone());
        let owner: (Option<String>, Option<String>) = serde_json::from_str(&target)?;
        let id: i64 = row.get(0)?;
        let (bytes, digest, version, count, origin, window) = block.query_row([id], |r| {
            Ok((
                r.get::<_, i64>(0)?,
                r.get::<_, String>(1)?,
                r.get::<_, i64>(2)?,
                r.get::<_, i64>(3)?,
                r.get::<_, String>(4)?,
                r.get::<_, i64>(5)?,
            ))
        })?;
        let written = if association_only {
            metadata::load(db, id, &digest, bytes, count)?
        } else {
            None
        };
        let events: Rows = if let Some(written) = written {
            require_index(
                written.target.thread_id == owner.0 && written.target.turn_id == owner.1,
                "written association target mismatch",
            )?;
            metadata::associations(db, id, &written)?
        } else {
            let data: Vec<u8> = db
                .prepare_cached("SELECT data FROM event_blocks WHERE rowid=?1")?
                .query_row([id], |r| r.get(0))?;
            let events: Rows = codec::decode(&data, bytes, &digest, version)?;
            require_index(
                !events.is_empty() && events.len() <= 200 && events.len() as i64 == count,
                "event reduction count mismatch",
            )?;
            events
        };
        validate_window(&events, &(origin, window))?;
        require_index(
            events
                .iter()
                .all(|e| (e.thread_id(), e.turn_id()) == (owner.0.as_deref(), owner.1.as_deref())),
            "event reduction target mismatch",
        )?;
        #[cfg(test)]
        DECODED.set(DECODED.get() + events.len());
        for mut event in events {
            if observe(&mut event)? {
                retained.insert(event.shared_id(), event);
            }
        }
    }
    if let Some(target) = current {
        consume(target, retained.into_values().collect())?;
    }
    Ok(())
}
