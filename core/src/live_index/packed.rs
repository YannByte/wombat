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
fn store(db: &Connection, bucket: i64, block: Option<i64>, rows: &Rows) -> Result<i64> {
    let (data, size, digest) = codec::encode(rows)?;
    if let Some(block) = block {
        db.prepare_cached(
            "UPDATE fact_blocks SET data=?1,bytes=?2,digest=?3 WHERE id=?4 AND digest!=?3",
        )?
        .execute(params![data, size as i64, digest, block])?;
        Ok(block)
    } else {
        db.prepare_cached(
            "INSERT INTO fact_blocks(bucket,codec,bytes,digest,data) VALUES(?1,?2,?3,?4,?5)",
        )?
        .execute(params![bucket, codec::VERSION, size as i64, digest, data])?;
        Ok(db.last_insert_rowid())
    }
}
fn publish(db: &Connection, bucket: i64, block: Option<i64>, rows: &Rows) -> Result<()> {
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
        publish_frame(db, bucket, active, &rows[start..end].to_vec())?;
        active = None;
        start = end;
    }
    Ok(())
}
fn publish_frame(db: &Connection, bucket: i64, block: Option<i64>, rows: &Rows) -> Result<i64> {
    let block = store(db, bucket, block, rows)?;
    let mut insert=db.prepare_cached("INSERT INTO fact_rows(bucket,id,block,slot) VALUES(?1,CASE WHEN length(?2)=64 AND ?2 NOT GLOB '*[^0-9a-f]*' THEN unhex(?2) ELSE ?2 END,?3,?4) ON CONFLICT(bucket,id) DO UPDATE SET block=excluded.block,slot=excluded.slot")?;
    for (slot, row) in rows.iter().enumerate() {
        if let Some((id, _)) = row {
            insert.execute(params![bucket, id, block, slot as i64])?;
        }
    }
    Ok(block)
}
pub(crate) fn upsert<'a, T: serde::Serialize + 'a>(
    db: &Connection,
    scope: &str,
    field: &str,
    values: impl IntoIterator<Item = (&'a str, &'a T)>,
) -> Result<()> {
    let bucket = bucket(db, scope, field)?;
    // Keep just one decoded frame. The staging relation groups corrections without
    // retaining another complete set of serialized facts in process memory.
    db.execute_batch("CREATE TEMP TABLE IF NOT EXISTS pending_facts(bucket INTEGER,id TEXT,payload TEXT,block INTEGER,slot INTEGER,PRIMARY KEY(bucket,id));")?;
    db.execute("DELETE FROM pending_facts WHERE bucket=?1", [bucket])?;
    let mut stage=db.prepare_cached("INSERT INTO pending_facts SELECT ?1,?2,?3,r.block,r.slot FROM (SELECT 1) LEFT JOIN fact_rows r ON r.bucket=?1 AND r.id=(CASE WHEN length(?2)=64 AND ?2 NOT GLOB '*[^0-9a-f]*' THEN unhex(?2) ELSE ?2 END) ON CONFLICT(bucket,id) DO UPDATE SET payload=excluded.payload")?;
    for (id, value) in values {
        stage.execute(params![bucket, id, serde_json::to_string(value)?])?;
    }
    drop(stage);
    let mut query=db.prepare_cached("SELECT id,payload,block,slot FROM pending_facts WHERE bucket=?1 ORDER BY block,json_extract(payload,'$.threadId'),id")?;
    let mut cursor = query.query([bucket])?;
    let mut active = None;
    let mut data = Rows::new();
    let mut bytes = 0;
    let mut started = false;
    while let Some(row) = cursor.next()? {
        let id: String = row.get(0)?;
        let payload: String = row.get(1)?;
        let block: Option<i64> = row.get(2)?;
        if let Some(block) = block {
            if active != Some(block) {
                if !data.is_empty() {
                    publish(db, bucket, active, &data)?;
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
                    publish(db, bucket, active, &data)?;
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
        publish(db, bucket, active, &data)?;
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
pub(super) fn each(
    db: &Connection,
    scope: &str,
    mut consume: impl FnMut(&str, &str, &str) -> Result<()>,
) -> Result<bool> {
    // Never join a compressed BLOB onto every row before sorting: that would
    // materialize the same frame up to 128 times in SQLite temporary storage.
    let missing: bool=db.query_row("SELECT EXISTS(SELECT 1 FROM buckets b JOIN fact_rows r ON r.bucket=b.id LEFT JOIN fact_blocks f ON f.id=r.block AND f.bucket=b.id WHERE b.scope=?1 AND f.id IS NULL)",[scope],|r|r.get(0))?;
    require_index(!missing, "missing fact block")?;
    let mut query=db.prepare_cached("SELECT b.field,f.id,f.bucket,f.data,f.bytes,f.digest,f.codec FROM buckets b JOIN fact_blocks f ON f.bucket=b.id WHERE b.scope=?1")?;
    let mut cursor = query.query([scope])?;
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
            consume(field, id, data[slot].as_ref().unwrap().1.get())?;
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
            db.execute("DELETE FROM fact_blocks WHERE id=?1", [block])?;
        } else {
            store(db, bucket, Some(block), &rows)?;
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
    let bucket = bucket(db, scope, field)?;
    let wanted: HashSet<_> = ids.collect();
    let obsolete = db
        .prepare_cached("SELECT CASE WHEN typeof(id)='blob' THEN lower(hex(id)) ELSE id END FROM fact_rows WHERE bucket=?1")?
        .query_map([bucket], |r| r.get::<_, String>(0))?
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
            db.execute("DELETE FROM fact_blocks WHERE id=?1", [block])?;
        } else {
            store(db, bucket, Some(block), &rows)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
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
        retain(&db, "scope", "operations", [a.as_str()].into_iter()).unwrap();
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
