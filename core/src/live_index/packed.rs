//! Canonical query facts are stored once. Updates rewrite only their bounded frame.
//! SHA-256 row keys use 32-byte blobs; other keys keep their exact text identity.
use super::*;
use serde_json::value::RawValue;
use std::collections::BTreeMap;

type Rows = Vec<Option<(String, Box<RawValue>)>>;
const ROWS: usize = 128;

pub(super) fn schema(db: &Connection) -> Result<()> {
    db.execute_batch("CREATE TABLE fact_blocks(id INTEGER PRIMARY KEY,bucket INTEGER NOT NULL,codec INTEGER NOT NULL,bytes INTEGER NOT NULL,digest TEXT NOT NULL,data BLOB NOT NULL);
      CREATE INDEX fact_blocks_bucket ON fact_blocks(bucket,id);
      CREATE TABLE fact_rows(bucket INTEGER NOT NULL,id TEXT NOT NULL,block INTEGER NOT NULL,slot INTEGER NOT NULL,PRIMARY KEY(bucket,id)) WITHOUT ROWID;
      CREATE INDEX fact_rows_block ON fact_rows(block,slot);")?;
    Ok(())
}
fn load(db: &Connection, block: i64) -> Result<Rows> {
    let mut s = db.prepare_cached("SELECT data,bytes,digest,codec FROM fact_blocks WHERE id=?1")?;
    let mut rows = s.query([block])?;
    let row = rows
        .next()?
        .ok_or_else(|| crate::dto::operation_error("INDEX_UNAVAILABLE", "missing fact block"))?;
    codec::decode(
        row.get_ref(0)?.as_blob()?,
        row.get(1)?,
        row.get_ref(2)?.as_str()?,
        row.get(3)?,
    )
}
fn store(
    db: &Connection,
    bucket: i64,
    block: Option<i64>,
    rows: &Rows,
    encoder: &mut codec::Encoder,
) -> Result<i64> {
    let (data, size, digest) = encoder.encode(rows)?;
    if let Some(block) = block {
        let changed = db
            .prepare_cached(
                "UPDATE fact_blocks SET data=?1,bytes=?2,digest=?3 WHERE id=?4 AND digest!=?3",
            )?
            .execute(params![data, size as i64, digest, block])?;
        if changed > 0 {
            changed_block(db, bucket, block)?;
        }
        Ok(block)
    } else {
        db.prepare_cached(
            "INSERT INTO fact_blocks(bucket,codec,bytes,digest,data) VALUES(?1,?2,?3,?4,?5)",
        )?
        .execute(params![bucket, codec::VERSION, size as i64, digest, data])?;
        let block = db.last_insert_rowid();
        changed_block(db, bucket, block)?;
        Ok(block)
    }
}
fn changed_block(db: &Connection, bucket: i64, block: i64) -> Result<()> {
    db.execute_batch("CREATE TEMP TABLE IF NOT EXISTS changed_fact_blocks(bucket INTEGER,block INTEGER,PRIMARY KEY(bucket,block)) WITHOUT ROWID;")?;
    db.execute(
        "INSERT OR IGNORE INTO changed_fact_blocks VALUES(?1,?2)",
        params![bucket, block],
    )?;
    Ok(())
}

pub(crate) fn measurement_block(
    db: &Connection,
    block: i64,
) -> Result<Vec<(usize, crate::adapters::contract::Measurement)>> {
    let rows = load(db, block)?;
    require_index(rows.len() <= ROWS, "measurement frame slot limit exceeded")?;
    let mut query=db.prepare_cached("SELECT CASE WHEN typeof(r.id)='blob' THEN lower(hex(r.id)) ELSE r.id END,r.slot FROM fact_rows r JOIN buckets b ON b.id=r.bucket WHERE r.block=?1 AND b.field='measurements' ORDER BY r.slot")?;
    let mut cursor = query.query([block])?;
    let mut found = 0;
    while let Some(row) = cursor.next()? {
        let id: String = row.get(0)?;
        let slot: i64 = row.get(1)?;
        require_index(slot >= 0, "negative measurement slot")?;
        ensure_slot(&rows, slot as usize, &id)?;
        found += 1;
    }
    require_index(
        found == rows.iter().flatten().count(),
        "measurement frame membership mismatch",
    )?;
    rows.into_iter()
        .enumerate()
        .filter_map(|(slot, row)| {
            row.map(|(id, payload)| {
                let value: crate::adapters::contract::Measurement =
                    serde_json::from_str(payload.get())?;
                require_index(value.id == id, "measurement block identity mismatch")?;
                Ok((slot, value))
            })
        })
        .collect()
}
fn publish(
    db: &Connection,
    bucket: i64,
    block: Option<i64>,
    rows: &Rows,
    encoder: &mut codec::Encoder,
) -> Result<()> {
    // Corrections may enlarge an existing row. Split after its staged block has
    // been fully updated, so a valid correction cannot overflow an old frame.
    let mut start = 0;
    let mut active = block;
    while start < rows.len() {
        let mut end = start;
        let mut bytes = 2;
        while end < rows.len() {
            let size = serde_json::to_vec(&rows[end])?.len() + 1;
            if end > start && bytes + size > codec::TARGET {
                break;
            }
            bytes += size;
            end += 1;
        }
        publish_frame(db, bucket, active, &rows[start..end].to_vec(), encoder)?;
        active = None;
        start = end;
    }
    Ok(())
}
fn publish_frame(
    db: &Connection,
    bucket: i64,
    block: Option<i64>,
    rows: &Rows,
    encoder: &mut codec::Encoder,
) -> Result<i64> {
    let block = store(db, bucket, block, rows, encoder)?;
    let mut insert=db.prepare_cached("INSERT INTO fact_rows(bucket,id,block,slot) VALUES(?1,CASE WHEN length(?2)=64 AND ?2 NOT GLOB '*[^0-9a-f]*' THEN unhex(?2) ELSE ?2 END,?3,?4) ON CONFLICT(bucket,id) DO UPDATE SET block=excluded.block,slot=excluded.slot")?;
    for (slot, row) in rows.iter().enumerate() {
        if let Some((id, _)) = row {
            insert.execute(params![bucket, id, block, slot as i64])?;
        }
    }
    Ok(block)
}
/// Prepare writer relations before opening long-lived source cursors. Creating
/// TEMP schema while a cursor is active can invalidate that cursor in SQLite.
pub(crate) fn prepare_write(db: &Connection) -> Result<()> {
    db.execute_batch("CREATE TEMP TABLE IF NOT EXISTS pending_facts(bucket INTEGER,id TEXT,payload TEXT,block INTEGER,slot INTEGER,PRIMARY KEY(bucket,id)); CREATE TEMP TABLE IF NOT EXISTS changed_fact_blocks(bucket INTEGER,block INTEGER,PRIMARY KEY(bucket,block)) WITHOUT ROWID;")?;
    Ok(())
}
pub(crate) fn upsert<K: AsRef<str>, T: serde::Serialize>(
    db: &Connection,
    scope: &str,
    field: &str,
    values: impl IntoIterator<Item = (K, T)>,
) -> Result<()> {
    let bucket = bucket(db, scope, field)?;
    // Keep just one decoded frame. The staging relation groups corrections without
    // retaining another complete set of serialized facts in process memory.
    prepare_write(db)?;
    db.execute("DELETE FROM pending_facts WHERE bucket=?1", [bucket])?;
    let mut stage=db.prepare_cached("INSERT INTO pending_facts SELECT ?1,?2,?3,r.block,r.slot FROM (SELECT 1) LEFT JOIN fact_rows r ON r.bucket=?1 AND r.id=(CASE WHEN length(?2)=64 AND ?2 NOT GLOB '*[^0-9a-f]*' THEN unhex(?2) ELSE ?2 END) ON CONFLICT(bucket,id) DO UPDATE SET payload=excluded.payload")?;
    for (id, value) in values {
        let id = id.as_ref();
        stage.execute(params![bucket, id, serde_json::to_string(&value)?])?;
    }
    drop(stage);
    // Physical locality is an optimization only: exact filters still use facts.
    // Keep revisions together, then cluster new measurements by report keys.
    let measurements: bool = db.query_row(
        "SELECT field='measurements' FROM buckets WHERE id=?1",
        [bucket],
        |r| r.get(0),
    )?;
    let mut query=db.prepare_cached("SELECT id,payload,block,slot FROM pending_facts WHERE bucket=?1 ORDER BY block,json_extract(payload,'$.threadId'),CASE WHEN ?2 THEN json_extract(payload,'$.model.raw') END,CASE WHEN ?2 THEN json_extract(payload,'$.reasoningEffort') END,CASE WHEN ?2 THEN substr(json_extract(payload,'$.timestamp'),1,13) END,id")?;
    let mut cursor = query.query(params![bucket, measurements])?;
    let mut active = None;
    let mut data = Rows::new();
    let mut bytes = 0;
    let mut started = false;
    let mut encoder = codec::Encoder::default();
    while let Some(row) = cursor.next()? {
        let id: String = row.get(0)?;
        let payload: String = row.get(1)?;
        let block: Option<i64> = row.get(2)?;
        if let Some(block) = block {
            if active != Some(block) {
                if !data.is_empty() {
                    publish(db, bucket, active, &data, &mut encoder)?;
                }
                active = Some(block);
                data = load(db, block)?;
            }
            let slot = row.get::<_, u32>(3)? as usize;
            ensure_slot(&data, slot, &id)?;
            data[slot] = Some((id, serde_json::from_str(&payload)?));
        } else {
            if !started {
                started = true;
                let tail: Option<i64> = db
                    .query_row(
                        "SELECT id FROM fact_blocks WHERE bucket=?1 ORDER BY id DESC LIMIT 1",
                        [bucket],
                        |r| r.get(0),
                    )
                    .optional()?;
                if let Some(tail) = tail {
                    let old = load(db, tail)?;
                    let size = serde_json::to_vec(&old)?.len();
                    if old.len() < ROWS && size < codec::TARGET {
                        active = Some(tail);
                        data = old;
                        bytes = size;
                    }
                }
            }
            if data.len() >= ROWS || bytes + payload.len() + id.len() + 16 > codec::TARGET {
                if !data.is_empty() {
                    publish(db, bucket, active, &data, &mut encoder)?;
                }
                data.clear();
                active = None;
                bytes = 0;
            }
            bytes += payload.len() + id.len() + 16;
            data.push(Some((id, serde_json::from_str(&payload)?)));
        }
    }
    if !data.is_empty() {
        publish(db, bucket, active, &data, &mut encoder)?;
    }
    drop(cursor);
    drop(query);
    db.execute("DELETE FROM pending_facts WHERE bucket=?1", [bucket])?;
    Ok(())
}
fn ensure_slot(rows: &Rows, slot: usize, id: &str) -> Result<()> {
    require_index(
        rows.get(slot)
            .and_then(Option::as_ref)
            .is_some_and(|r| r.0 == id),
        "fact block identity mismatch",
    )
}
pub(super) fn each_filtered(
    db: &Connection,
    scope: &str,
    skip_operations: bool,
    mut consume: impl FnMut(&str, &str, &str) -> Result<()>,
) -> Result<bool> {
    each_located_inner(db, scope, None, skip_operations, |field, id, payload, _| {
        consume(field, id, payload)
    })
}

#[derive(Clone, Debug)]
pub(crate) struct FactLocation {
    pub block: i64,
    pub slot: usize,
    pub id: String,
}
pub(crate) fn each_located(
    db: &Connection,
    scope: &str,
    selected: Option<&str>,
    consume: impl FnMut(&str, &str, &str, FactSlot) -> Result<()>,
) -> Result<bool> {
    each_located_inner(db, scope, selected, false, consume)
}
#[derive(Clone, Copy)]
pub(crate) struct FactSlot {
    pub block: i64,
    pub slot: usize,
}
fn each_located_inner(
    db: &Connection,
    scope: &str,
    selected: Option<&str>,
    skip_operations: bool,
    mut consume: impl FnMut(&str, &str, &str, FactSlot) -> Result<()>,
) -> Result<bool> {
    // Never join a compressed BLOB onto every row before sorting: that would
    // materialize the same frame up to 128 times in SQLite temporary storage.
    let missing: bool=db.query_row("SELECT EXISTS(SELECT 1 FROM buckets b JOIN fact_rows r ON r.bucket=b.id LEFT JOIN fact_blocks f ON f.id=r.block AND f.bucket=b.id WHERE b.scope=?1 AND f.id IS NULL)",[scope],|r|r.get(0))?;
    require_index(!missing, "missing fact block")?;
    let mut query=db.prepare_cached("SELECT b.field,f.id,f.bucket,f.data,f.bytes,f.digest,f.codec FROM buckets b JOIN fact_blocks f ON f.bucket=b.id WHERE b.scope=?1 AND (?2 IS NULL OR b.field=?2) AND (?3=0 OR b.field!='operations')")?;
    let mut cursor = query.query(params![scope, selected, skip_operations])?;
    let mut found = false;
    while let Some(row) = cursor.next()? {
        let field = row.get_ref(0)?.as_str()?;
        let block: i64 = row.get(1)?;
        let bucket: i64 = row.get(2)?;
        let data: Rows = codec::decode(
            row.get_ref(3)?.as_blob()?,
            row.get(4)?,
            row.get_ref(5)?.as_str()?,
            row.get(6)?,
        )?;
        let mut mapping = db.prepare_cached(
            "SELECT CASE WHEN typeof(id)='blob' THEN lower(hex(id)) ELSE id END,slot FROM fact_rows WHERE block=?1 AND bucket=?2 ORDER BY slot",
        )?;
        let mut members = mapping.query(params![block, bucket])?;
        let mut count = 0;
        while let Some(member) = members.next()? {
            let id = member.get_ref(0)?.as_str()?;
            let slot = member.get::<_, u32>(1)? as usize;
            ensure_slot(&data, slot, id)?;
            consume(
                field,
                id,
                data[slot].as_ref().unwrap().1.get(),
                FactSlot { block, slot },
            )?;
            found = true;
            count += 1;
        }
        require_index(
            count == data.iter().flatten().count(),
            "incomplete fact block membership",
        )?;
    }
    Ok(found)
}

#[cfg(test)]
pub(super) fn remove(db: &Connection, bucket: i64, id: &str) -> Result<()> {
    let location: Option<(i64, usize)> = db
        .prepare_cached("SELECT block,slot FROM fact_rows WHERE bucket=?1 AND id=(CASE WHEN length(?2)=64 AND ?2 NOT GLOB '*[^0-9a-f]*' THEN unhex(?2) ELSE ?2 END)")?
        .query_row(params![bucket, id], |r| {
            Ok((r.get(0)?, r.get::<_, u32>(1)? as usize))
        })
        .optional()?;
    if let Some((block, slot)) = location {
        let mut rows = load(db, block)?;
        ensure_slot(&rows, slot, id)?;
        rows[slot] = None;
        db.execute(
            "DELETE FROM fact_rows WHERE bucket=?1 AND id=(CASE WHEN length(?2)=64 AND ?2 NOT GLOB '*[^0-9a-f]*' THEN unhex(?2) ELSE ?2 END)",
            params![bucket, id],
        )?;
        if rows.iter().all(Option::is_none) {
            changed_block(db, bucket, block)?;
            db.execute("DELETE FROM fact_blocks WHERE id=?1", [block])?;
        } else {
            store(
                db,
                bucket,
                Some(block),
                &rows,
                &mut codec::Encoder::default(),
            )?;
        }
    }
    Ok(())
}
pub(crate) fn retain<'a>(
    db: &Connection,
    scope: &str,
    field: &str,
    ids: impl Iterator<Item = &'a str>,
) -> Result<()> {
    retain_prefix(db, scope, field, None, ids)
}
pub(crate) fn retain_prefix<'a>(
    db: &Connection,
    scope: &str,
    field: &str,
    prefix: Option<&str>,
    ids: impl Iterator<Item = &'a str>,
) -> Result<()> {
    let bucket = bucket(db, scope, field)?;
    let wanted: HashSet<_> = ids.collect();
    // A bound literal GLOB prefix uses the existing (bucket,id) range index.
    // substr(id,...) scanned every association for each changed target.
    let pattern = prefix.map(|prefix| {
        let mut pattern = String::new();
        for ch in prefix.chars() {
            match ch {
                '*' => pattern.push_str("[*]"),
                '?' => pattern.push_str("[?]"),
                '[' => pattern.push_str("[[]"),
                _ => pattern.push(ch),
            }
        }
        pattern.push('*');
        pattern
    });
    let sql = if pattern.is_some() {
        "SELECT CASE WHEN typeof(id)='blob' THEN lower(hex(id)) ELSE id END FROM fact_rows WHERE bucket=?1 AND id GLOB ?2"
    } else {
        "SELECT CASE WHEN typeof(id)='blob' THEN lower(hex(id)) ELSE id END FROM fact_rows WHERE bucket=?1 AND ?2 IS NULL"
    };
    let obsolete = db
        .prepare_cached(sql)?
        .query_map(params![bucket, pattern], |r| r.get::<_, String>(0))?
        .filter_map(|r| match r {
            Ok(id) if wanted.contains(id.as_str()) => None,
            other => Some(other),
        })
        .collect::<rusqlite::Result<Vec<_>>>()?;
    // Group deletions so a retraction does not decode one block once per removed row.
    let mut grouped = BTreeMap::<i64, Vec<(String, usize)>>::new();
    for id in obsolete {
        let (block, slot) = db.query_row(
            "SELECT block,slot FROM fact_rows WHERE bucket=?1 AND id=(CASE WHEN length(?2)=64 AND ?2 NOT GLOB '*[^0-9a-f]*' THEN unhex(?2) ELSE ?2 END)",
            params![bucket, id],
            |r| Ok((r.get(0)?, r.get::<_, u32>(1)? as usize)),
        )?;
        grouped.entry(block).or_default().push((id, slot));
    }
    let mut encoder = codec::Encoder::default();
    for (block, items) in grouped {
        let mut rows = load(db, block)?;
        for (id, slot) in items {
            ensure_slot(&rows, slot, &id)?;
            rows[slot] = None;
            db.execute(
                "DELETE FROM fact_rows WHERE bucket=?1 AND id=(CASE WHEN length(?2)=64 AND ?2 NOT GLOB '*[^0-9a-f]*' THEN unhex(?2) ELSE ?2 END)",
                params![bucket, id],
            )?;
        }
        if rows.iter().all(Option::is_none) {
            changed_block(db, bucket, block)?;
            db.execute("DELETE FROM fact_blocks WHERE id=?1", [block])?;
        } else {
            store(db, bucket, Some(block), &rows, &mut encoder)?;
        }
    }
    Ok(())
}

/// Replace membership from an external stream. Neither the wanted IDs nor the
/// obsolete block directory is collected in a source-sized Rust container.
pub(crate) fn retain_streamed(
    db: &Connection,
    scope: &str,
    field: &str,
    ids: impl IntoIterator<Item = Result<String>>,
) -> Result<()> {
    let bucket = bucket(db, scope, field)?;
    db.execute_batch("CREATE TEMP TABLE IF NOT EXISTS wanted_facts(id PRIMARY KEY) WITHOUT ROWID; DELETE FROM wanted_facts;")?;
    let mut insert = db.prepare_cached("INSERT INTO wanted_facts VALUES(CASE WHEN length(?1)=64 AND ?1 NOT GLOB '*[^0-9a-f]*' THEN unhex(?1) ELSE ?1 END)")?;
    for id in ids {
        insert.execute([id?])?;
    }
    drop(insert);
    let mut query = db.prepare_cached("SELECT DISTINCT r.block FROM fact_rows r WHERE r.bucket=?1 AND NOT EXISTS(SELECT 1 FROM wanted_facts w WHERE w.id=r.id) ORDER BY r.block")?;
    let mut cursor = query.query([bucket])?;
    let mut encoder = codec::Encoder::default();
    while let Some(row) = cursor.next()? {
        let block: i64 = row.get(0)?;
        let mut data = load(db, block)?;
        let mut obsolete = db.prepare_cached("SELECT CASE WHEN typeof(r.id)='blob' THEN lower(hex(r.id)) ELSE r.id END,r.slot FROM fact_rows r WHERE r.bucket=?1 AND r.block=?2 AND NOT EXISTS(SELECT 1 FROM wanted_facts w WHERE w.id=r.id)")?;
        let items = obsolete
            .query_map(params![bucket, block], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, u32>(1)? as usize))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        for (id, slot) in items {
            ensure_slot(&data, slot, &id)?;
            data[slot] = None;
        }
        db.execute("DELETE FROM fact_rows WHERE bucket=?1 AND block=?2 AND NOT EXISTS(SELECT 1 FROM wanted_facts w WHERE w.id=fact_rows.id)",params![bucket,block])?;
        if data.iter().all(Option::is_none) {
            changed_block(db, bucket, block)?;
            db.execute("DELETE FROM fact_blocks WHERE id=?1", [block])?;
        } else {
            store(db, bucket, Some(block), &data, &mut encoder)?;
        }
    }
    db.execute("DELETE FROM wanted_facts", [])?;
    Ok(())
}

/// Decode each touched frame once per request; no unbounded process cache.
pub(crate) fn read_operations(
    db: &Connection,
    locations: &[FactLocation],
    mut charge: impl FnMut(u64) -> Result<()>,
) -> Result<Vec<std::sync::Arc<crate::adapters::contract::Operation>>> {
    let mut grouped = BTreeMap::<i64, Vec<(usize, &FactLocation)>>::new();
    for (index, location) in locations.iter().enumerate() {
        grouped
            .entry(location.block)
            .or_default()
            .push((index, location));
    }
    let mut out = vec![None; locations.len()];
    for (block, locations) in grouped {
        let bytes: i64 =
            db.query_row("SELECT bytes FROM fact_blocks WHERE id=?1", [block], |r| {
                r.get(0)
            })?;
        require_index(bytes > 0, "invalid cold operation block size")?;
        charge(bytes as u64)?;
        let data = load(db, block)?;
        for (index, location) in locations {
            ensure_slot(&data, location.slot, &location.id)?;
            let value: crate::adapters::contract::Operation =
                serde_json::from_str(data[location.slot].as_ref().unwrap().1.get())?;
            require_index(value.id == location.id, "cold operation identity mismatch")?;
            out[index] = Some(std::sync::Arc::new(value));
        }
    }
    out.into_iter()
        .map(|v| {
            v.ok_or_else(|| {
                crate::dto::operation_error("INDEX_UNAVAILABLE", "missing cold operation")
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn target_retention_matches_literal_prefixes_and_uses_the_existing_range_index() {
        let root = tempfile::tempdir().unwrap();
        let db = open(&root.path().join("index.sqlite")).unwrap();
        for prefix in ["a*", "a?", "a[", "a]", "任务:", ""] {
            let keep = format!("{prefix}keep");
            let remove = format!("{prefix}remove");
            let other = "other";
            upsert(
                &db,
                "literal",
                "associations",
                [keep.clone(), remove.clone(), other.into()]
                    .into_iter()
                    .map(|id| (id.clone(), json!({"id":id}))),
            )
            .unwrap();
            let before = load_map(&db, "literal").unwrap();
            retain_prefix(
                &db,
                "literal",
                "associations",
                Some(prefix),
                [keep.as_str()].into_iter(),
            )
            .unwrap();
            let after = load_map(&db, "literal").unwrap();
            for (id, value) in before["associations"].as_object().unwrap() {
                if id.starts_with(prefix) && id != &keep {
                    assert!(after["associations"].get(id).is_none());
                } else {
                    assert_eq!(&after["associations"][id], value);
                }
            }
            retain(&db, "literal", "associations", std::iter::empty()).unwrap();
        }
        let hash_prefix = format!("{}:", "a".repeat(64));
        let detail: String = db
            .query_row(
                "EXPLAIN QUERY PLAN SELECT id FROM fact_rows WHERE bucket=?1 AND id GLOB ?2",
                params![1, format!("{hash_prefix}*")],
                |row| row.get(3),
            )
            .unwrap();
        assert!(
            detail.contains("id>?") && detail.contains("id<?"),
            "{detail}"
        );
    }

    #[test]
    fn cold_operation_budget_stops_before_decoding_a_corrupt_frame() {
        let root = tempfile::tempdir().unwrap();
        let db = open(&root.path().join("index.sqlite")).unwrap();
        upsert(
            &db,
            "scope",
            "operations",
            [("operation", &json!({"id":"operation"}))],
        )
        .unwrap();
        let mut locations = Vec::new();
        each_located(&db, "scope", Some("operations"), |_, id, _, location| {
            locations.push(FactLocation {
                block: location.block,
                slot: location.slot,
                id: id.to_owned(),
            });
            Ok(())
        })
        .unwrap();
        db.execute("UPDATE fact_blocks SET data=x'00'", []).unwrap();
        let error = read_operations(&db, &locations, |_| {
            Err(crate::dto::operation_error(
                "RESOURCE_LIMIT",
                "test byte budget",
            ))
        })
        .unwrap_err();
        assert_eq!(failure_code(&error), "RESOURCE_LIMIT");
        assert!(read_operations(&db, &locations, |_| Ok(())).is_err());
    }
    #[test]
    fn enlarged_corrections_split_frames_and_binary_keys_preserve_identity() {
        let root = tempfile::tempdir().unwrap();
        let db = open(&root.path().join("index.sqlite")).unwrap();
        let a = "ab".repeat(32);
        let b = "AB".repeat(32);
        let original = json!({"small":0});
        upsert(
            &db,
            "scope",
            "operations",
            [(a.as_str(), &original), (b.as_str(), &original)],
        )
        .unwrap();
        assert_eq!(
            db.query_row(
                "SELECT count(*) FROM fact_rows WHERE typeof(id)='blob'",
                [],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
            1
        );
        let big = json!({"payload":"x".repeat(codec::TARGET)});
        upsert(
            &db,
            "scope",
            "operations",
            [(a.as_str(), &big), (b.as_str(), &big)],
        )
        .unwrap();
        let restored = load_map(&db, "scope").unwrap();
        assert_eq!(restored["operations"][&a], big);
        assert_eq!(restored["operations"][&b], big);
        assert_eq!(
            db.query_row("SELECT count(*) FROM fact_blocks", [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            2
        );
        let removed: i64 = db
            .query_row(
                "SELECT block FROM fact_rows WHERE typeof(id)='text' AND id=?1",
                [&b],
                |r| r.get(0),
            )
            .unwrap();
        db.execute("DELETE FROM changed_fact_blocks", []).unwrap();
        retain(&db, "scope", "operations", [a.as_str()].into_iter()).unwrap();
        assert_eq!(
            db.query_row(
                "SELECT count(*) FROM changed_fact_blocks WHERE block=?1",
                [removed],
                |r| r.get::<_, i64>(0)
            )
            .unwrap(),
            1,
            "a fully removed frame must invalidate its aggregate"
        );
        assert_eq!(
            load_map(&db, "scope").unwrap()["operations"]
                .as_object()
                .unwrap()
                .len(),
            1
        );
    }
    #[test]
    fn small_appends_share_frames_corrections_retractions_and_rollback_are_exact() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("index.sqlite");
        let mut db = open(&path).unwrap();
        for n in 0..140 {
            let id = format!("m{n:04}");
            let value = json!({"id":id,"n":u64::MAX,"missing":null,"zero":0,"text":"中文"});
            upsert(&db, "scope", "measurements", [(id.as_str(), &value)]).unwrap();
        }
        assert!(has_scope(&db, "scope").unwrap());
        assert_eq!(
            db.query_row("SELECT count(*) FROM fact_blocks", [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            2
        );
        let original = load_map(&db, "scope").unwrap();
        {
            let tx = db.transaction().unwrap();
            let change = json!({"n":0});
            upsert(
                &tx,
                "scope",
                "measurements",
                [("m0000", &change), ("new", &change)],
            )
            .unwrap();
            remove(&tx, bucket(&tx, "scope", "measurements").unwrap(), "m0001").unwrap();
            assert_eq!(
                load_map(&tx, "scope").unwrap()["measurements"]["m0000"],
                change
            );
        }
        assert_eq!(load_map(&db, "scope").unwrap(), original);
        let changed = json!({"n":1,"missing":null});
        upsert(&db, "scope", "measurements", [("m0000", &changed)]).unwrap();
        retain(&db, "scope", "measurements", ["m0000", "m0139"].into_iter()).unwrap();
        drop(db);
        let db = open(&path).unwrap();
        let restored = load_map(&db, "scope").unwrap();
        assert_eq!(restored["measurements"].as_object().unwrap().len(), 2);
        assert_eq!(restored["measurements"]["m0000"], changed);
        assert_eq!(
            restored["measurements"]["m0139"],
            original["measurements"]["m0139"]
        );
        db.execute("DELETE FROM fact_blocks", []).unwrap();
        assert!(
            load_map(&db, "scope").is_err(),
            "missing frames must not become zero usage"
        );
    }
}
