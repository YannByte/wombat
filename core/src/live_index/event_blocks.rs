//! Source-local observation windows; projection membership is one bucket link.
//! An append rewrites only touched windows, never the whole source archive.
use super::*;
use crate::session_events::Event;
use std::{collections::BTreeMap, sync::Arc};

const WINDOW: u64 = 256 * 1024;
type Window = (String, i64);
type Rows = Vec<Arc<Event>>;
mod metadata;
mod reduction;
#[cfg(test)]
mod selection_tests;
pub(crate) use reduction::each_group;
#[cfg(test)]
thread_local! { static DECODED: std::cell::Cell<usize> = const { std::cell::Cell::new(0) }; }
// json_each has no source cardinality statistics. Keep targets outside the
// frame loop so each target uses the complete existing expression index.
const TARGET_ROWS: &str = "SELECT e.rowid FROM json_each(?2) t CROSS JOIN event_blocks e ON coalesce(json_extract(e.target,'$[0]'),'')=coalesce(json_extract(t.value,'$[0]'),'') AND coalesce(json_extract(e.target,'$[1]'),'')=coalesce(json_extract(t.value,'$[1]'),'') WHERE e.bucket IN (SELECT coalesce(l.origin,b.id) FROM buckets b LEFT JOIN event_links l ON l.bucket=b.id WHERE b.field='events' AND b.scope IN (SELECT value FROM json_each(?1))) ORDER BY e.target,e.origin,e.window,e.part";

#[cfg(test)]
pub(crate) fn decoded_event_count() -> usize {
    DECODED.get()
}

pub(crate) fn validate_cold_source(db: &Connection, scope: &str) -> Result<()> {
    let origin: Option<i64> = db.prepare_cached("SELECT coalesce(l.origin,b.id) FROM buckets b LEFT JOIN event_links l ON l.bucket=b.id WHERE b.scope=?1 AND b.field='events'")?.query_row([scope], |r| r.get(0)).optional()?;
    if let Some(origin) = origin {
        check_head(db, origin)?;
        require_index(
            !db.query_row(
                "SELECT EXISTS(SELECT 1 FROM entries WHERE bucket=?1)",
                [origin],
                |r| r.get::<_, bool>(0),
            )?,
            "unexpected unpacked observation rows",
        )?;
        if db.query_row(
            "SELECT EXISTS(SELECT 1 FROM event_blocks WHERE bucket=?1 AND codec!=?2)",
            params![origin, codec::VERSION],
            |r| r.get::<_, bool>(0),
        )? {
            return Err(crate::dto::operation_error(
                "INDEX_UNSUPPORTED_VERSION",
                "Unsupported index block codec",
            ));
        }
    }
    Ok(())
}
pub(super) fn schema(db: &Connection) -> Result<()> {
    db.execute_batch("CREATE TABLE event_blocks(bucket INTEGER NOT NULL,origin TEXT NOT NULL,window INTEGER NOT NULL,part INTEGER NOT NULL,target TEXT NOT NULL,codec INTEGER NOT NULL,bytes INTEGER NOT NULL,digest TEXT NOT NULL,data BLOB NOT NULL,count INTEGER NOT NULL,UNIQUE(bucket,origin,window,part));
      CREATE INDEX event_blocks_thread ON event_blocks(bucket,coalesce(json_extract(target,'$[0]'),''),coalesce(json_extract(target,'$[1]'),''),origin,window,part);
      CREATE TABLE event_links(bucket INTEGER PRIMARY KEY,origin INTEGER NOT NULL);
      CREATE TABLE event_heads(bucket INTEGER PRIMARY KEY,event_count INTEGER NOT NULL,block_count INTEGER NOT NULL);")?;
    Ok(())
}
pub(crate) fn begin_changes(db: &Connection) -> Result<()> {
    metadata::clear(db)?;
    db.execute_batch("CREATE TEMP TABLE IF NOT EXISTS changed_event_targets(target TEXT PRIMARY KEY); DELETE FROM changed_event_targets;")?;
    Ok(())
}
pub(crate) fn changed_targets(db: &Connection) -> Result<Vec<String>> {
    Ok(db
        .prepare_cached("SELECT target FROM changed_event_targets ORDER BY target")?
        .query_map([], |r| r.get(0))?
        .collect::<rusqlite::Result<_>>()?)
}
fn track_window(db: &Connection, bucket: i64, key: &Window) -> Result<()> {
    db.execute("INSERT OR IGNORE INTO changed_event_targets SELECT DISTINCT target FROM event_blocks WHERE bucket=?1 AND origin=?2 AND window=?3", params![bucket,key.0,key.1])?;
    Ok(())
}
fn check_head(db: &Connection, bucket: i64) -> Result<()> {
    let expected: Option<(i64, i64)> = db
        .query_row(
            "SELECT event_count,block_count FROM event_heads WHERE bucket=?1",
            [bucket],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?;
    let actual: (i64, i64) = db.query_row(
        "SELECT coalesce(sum(count),0),count(*) FROM event_blocks WHERE bucket=?1",
        [bucket],
        |r| Ok((r.get(0)?, r.get(1)?)),
    )?;
    if expected != Some(actual) {
        return Err(crate::dto::operation_error(
            "INDEX_UNAVAILABLE",
            "Incomplete observation blocks",
        ));
    }
    Ok(())
}

fn key(event: &Event) -> Result<Window> {
    let p = event.position();
    Ok((
        crate::hash(serde_json::to_vec(&(
            &p.source_instance_id,
            &p.file_id,
            &p.generation,
        ))?),
        (p.byte_offset / WINDOW) as i64,
    ))
}
/// A bounded pass-local cache: millions of events commonly share only a few
/// thousand file generations. Unknown domains beyond the cap use the same hash.
#[derive(Default)]
struct WindowKeys<'a> {
    origins: BTreeMap<(&'a str, &'a str, &'a str), String>,
}
impl<'a> WindowKeys<'a> {
    fn key(&mut self, event: &'a Event) -> Result<Window> {
        let p = event.position();
        let domain = (
            p.source_instance_id.as_ref(),
            p.file_id.as_ref(),
            p.generation.as_ref(),
        );
        if let Some(origin) = self.origins.get(&domain) {
            return Ok((origin.clone(), (p.byte_offset / WINDOW) as i64));
        }
        let result = key(event)?;
        if self.origins.len() < 4096 {
            self.origins.insert(domain, result.0.clone());
        }
        Ok(result)
    }
}
fn validate_window(events: &[Arc<Event>], expected: &Window) -> Result<()> {
    let Some(first) = events.first() else {
        return Ok(());
    };
    require_index(key(first)? == *expected, "event window identity mismatch")?;
    let first = first.position();
    for event in &events[1..] {
        let position = event.position();
        require_index(
            position.source_instance_id == first.source_instance_id
                && position.file_id == first.file_id
                && position.generation == first.generation
                && (position.byte_offset / WINDOW) as i64 == expected.1,
            "event block source mismatch",
        )?;
    }
    Ok(())
}
fn load_window(db: &Connection, bucket: i64, key: &Window) -> Result<Rows> {
    let mut q=db.prepare_cached("SELECT data,bytes,digest,codec,count FROM event_blocks WHERE bucket=?1 AND origin=?2 AND window=?3 ORDER BY part")?;
    let mut rows = q.query(params![bucket, key.0, key.1])?;
    let mut result = vec![];
    while let Some(row) = rows.next()? {
        let events: Rows = codec::decode(
            row.get_ref(0)?.as_blob()?,
            row.get(1)?,
            row.get_ref(2)?.as_str()?,
            row.get(3)?,
        )?;
        require_index(
            events.len() as i64 == row.get::<_, i64>(4)?,
            "event block count mismatch",
        )?;
        validate_window(&events, key)?;
        result.extend(events);
    }
    Ok(result)
}
fn save_window(
    db: &Connection,
    bucket: i64,
    key: &Window,
    events: &[Arc<Event>],
    encoder: &mut codec::Encoder,
) -> Result<()> {
    let mut targets = BTreeMap::<_, Vec<Arc<Event>>>::new();
    for event in events {
        targets
            .entry((event.thread_id(), event.turn_id()))
            .or_default()
            .push(Arc::clone(event));
    }
    let mut part = 0;
    for ((thread, turn), events) in targets {
        let target = serde_json::to_string(&(thread, turn))?;
        part = save_target(db, bucket, key, &target, &events, part, encoder)?;
    }
    db.execute(
        "DELETE FROM event_blocks WHERE bucket=?1 AND origin=?2 AND window=?3 AND part>=?4",
        params![bucket, key.0, key.1, part],
    )?;
    Ok(())
}
fn save_target(
    db: &Connection,
    bucket: i64,
    key: &Window,
    target: &str,
    events: &[Arc<Event>],
    first_part: i64,
    encoder: &mut codec::Encoder,
) -> Result<i64> {
    let mut start = 0;
    let mut part = first_part;
    let mut pending = None;
    while start < events.len() {
        let mut end = start;
        let mut size = 2;
        let mut raw = Vec::with_capacity(codec::TARGET);
        raw.push(b'[');
        let mut associations = vec![b'['];
        let mut association_count = 0;
        while end < events.len() {
            let encoded = match pending.take() {
                Some(encoded) => encoded,
                None => serde_json::to_vec(&events[end])?,
            };
            let n = encoded.len() + 1;
            if end > start && (size + n > codec::TARGET || end - start >= 200) {
                pending = Some(encoded);
                break;
            }
            if end > start {
                raw.push(b',');
            }
            raw.extend_from_slice(&encoded);
            if metadata::projection_input(&events[end]) {
                if association_count > 0 {
                    associations.push(b',');
                }
                associations.extend_from_slice(&encoded);
                association_count += 1;
            }
            size += n;
            end += 1;
        }
        raw.push(b']');
        associations.push(b']');
        let (data, bytes, digest) = encoder.encode_raw(&raw)?;
        db.prepare_cached("INSERT INTO event_blocks(bucket,origin,window,part,target,codec,bytes,digest,data,count) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10) ON CONFLICT(bucket,origin,window,part) DO UPDATE SET target=excluded.target,bytes=excluded.bytes,digest=excluded.digest,data=excluded.data,count=excluded.count WHERE digest!=excluded.digest")?.execute(params![bucket,key.0,key.1,part,target,codec::VERSION,bytes as i64,digest,data,(end-start) as i64])?;
        let id: i64 = db.prepare_cached("SELECT rowid FROM event_blocks WHERE bucket=?1 AND origin=?2 AND window=?3 AND part=?4")?.query_row(params![bucket,key.0,key.1,part], |r| r.get(0))?;
        metadata::save(
            db,
            id,
            digest,
            bytes as u64,
            &events[start..end],
            (&associations, association_count),
            encoder,
        )?;
        start = end;
        part += 1;
    }
    Ok(part)
}
pub(crate) fn save<'a>(
    db: &Connection,
    scope: &str,
    events: impl IntoIterator<Item = &'a Arc<Event>>,
    replace: bool,
    cancelled: &std::sync::atomic::AtomicBool,
) -> Result<()> {
    let bucket = bucket(db, scope, "events")?;
    metadata::prepare(db)?;
    db.execute_batch(
        "CREATE TEMP TABLE IF NOT EXISTS changed_event_targets(target TEXT PRIMARY KEY);",
    )?;
    let mut groups = BTreeMap::<Window, Vec<&Arc<Event>>>::new();
    let mut keys = WindowKeys::default();
    for event in events {
        crate::operation_association::check(cancelled)?;
        groups.entry(keys.key(event)?).or_default().push(event);
    }
    let mut totals: (i64, i64) = db
        .query_row(
            "SELECT event_count,block_count FROM event_heads WHERE bucket=?1",
            [bucket],
            |r| Ok((r.get(0)?, r.get(1)?)),
        )
        .optional()?
        .unwrap_or((0, 0));
    let window_counts = |key: &Window| -> Result<(i64, i64)> {
        Ok(db.query_row("SELECT coalesce(sum(count),0),count(*) FROM event_blocks WHERE bucket=?1 AND origin=?2 AND window=?3", params![bucket,key.0,key.1], |r| Ok((r.get(0)?,r.get(1)?)))?)
    };
    let mut encoder = codec::Encoder::default();
    for (key, added) in &groups {
        crate::operation_association::check(cancelled)?;
        let mut rows: BTreeMap<_, _> = if replace {
            BTreeMap::new()
        } else {
            load_window(db, bucket, key)?
                .into_iter()
                .map(|e| (e.id().to_owned(), e))
                .collect()
        };
        for event in added {
            rows.insert(event.id().to_owned(), Arc::clone(event));
        }
        let mut rows: Rows = rows.into_values().collect();
        rows.sort_unstable_by(|a, b| {
            (a.position().byte_offset, a.position().ordinal)
                .cmp(&(b.position().byte_offset, b.position().ordinal))
        });
        let before = window_counts(key)?;
        track_window(db, bucket, key)?;
        save_window(db, bucket, key, &rows, &mut encoder)?;
        track_window(db, bucket, key)?;
        let after = window_counts(key)?;
        totals.0 += after.0 - before.0;
        totals.1 += after.1 - before.1;
    }
    crate::operation_association::check(cancelled)?;
    if replace {
        let old = db
            .prepare_cached("SELECT DISTINCT origin,window FROM event_blocks WHERE bucket=?1")?
            .query_map([bucket], |r| {
                Ok((r.get::<_, String>(0)?, r.get::<_, i64>(1)?))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        for key in old {
            if !groups.contains_key(&key) {
                track_window(db, bucket, &key)?;
                let before = window_counts(&key)?;
                totals.0 -= before.0;
                totals.1 -= before.1;
                db.execute(
                    "DELETE FROM event_blocks WHERE bucket=?1 AND origin=?2 AND window=?3",
                    params![bucket, key.0, key.1],
                )?;
            }
        }
    }
    require_index(
        totals.0 >= 0 && totals.1 >= 0,
        "invalid observation count delta",
    )?;
    db.execute("INSERT INTO event_heads VALUES(?1,?2,?3) ON CONFLICT(bucket) DO UPDATE SET event_count=excluded.event_count,block_count=excluded.block_count WHERE event_count!=excluded.event_count OR block_count!=excluded.block_count", params![bucket,totals.0,totals.1])?;
    Ok(())
}
/// Retire replaced physical generations before replay, preserving untouched
/// windows so read-only verification does not rewrite identical observations.
pub(crate) fn discard_origins(
    db: &Connection,
    scope: &str,
    origins: &std::collections::BTreeSet<String>,
) -> Result<()> {
    let bucket = bucket(db, scope, "events")?;
    for origin in origins {
        db.execute("INSERT OR IGNORE INTO changed_event_targets SELECT DISTINCT target FROM event_blocks WHERE bucket=?1 AND origin=?2",params![bucket,origin])?;
        db.execute(
            "DELETE FROM event_blocks WHERE bucket=?1 AND origin=?2",
            params![bucket, origin],
        )?;
    }
    db.execute("UPDATE event_heads SET event_count=(SELECT coalesce(sum(count),0) FROM event_blocks WHERE bucket=?1),block_count=(SELECT count(*) FROM event_blocks WHERE bucket=?1) WHERE bucket=?1",[bucket])?;
    Ok(())
}
pub(crate) fn share(db: &Connection, scope: &str, origin: &str) -> Result<()> {
    let target = bucket(db, scope, "events")?;
    let source = bucket(db, origin, "events")?;
    anyhow::ensure!(source != target, "event bucket cannot reference itself");
    let indirect: bool = db.query_row(
        "SELECT EXISTS(SELECT 1 FROM event_links WHERE bucket=?1)",
        [source],
        |r| r.get(0),
    )?;
    anyhow::ensure!(!indirect, "event bucket links must have one hop");
    db.execute("INSERT INTO event_links VALUES(?1,?2) ON CONFLICT(bucket) DO UPDATE SET origin=excluded.origin WHERE origin!=excluded.origin",params![target,source])?;
    Ok(())
}
pub(super) fn each(
    db: &Connection,
    scope: &str,
    threads: Option<(&[&str], bool)>,
    mut consume: impl FnMut(&str, &str) -> Result<()>,
) -> Result<bool> {
    each_typed(db, scope, threads, |event| {
        consume(event.id(), &serde_json::to_string(&event)?)
    })
}

pub(super) fn each_typed(
    db: &Connection,
    scope: &str,
    threads: Option<(&[&str], bool)>,
    mut consume: impl FnMut(Arc<Event>) -> Result<()>,
) -> Result<bool> {
    let origin:Option<i64>=db.prepare_cached("SELECT coalesce(l.origin,b.id) FROM buckets b LEFT JOIN event_links l ON l.bucket=b.id WHERE b.scope=?1 AND b.field='events'")?.query_row([scope],|r|r.get(0)).optional()?;
    let Some(origin) = origin else {
        return Ok(false);
    };
    require_index(
        !db.query_row(
            "SELECT EXISTS(SELECT 1 FROM entries WHERE bucket=?1)",
            [origin],
            |r| r.get::<_, bool>(0),
        )?,
        "unexpected unpacked observation rows",
    )?;
    check_head(db, origin)?;
    let sql = if threads.is_some() {
        "SELECT origin,window,part,data,bytes,digest,codec,count FROM event_blocks WHERE bucket=?1 AND coalesce(json_extract(target,'$[0]'),'') IN (SELECT value FROM json_each(?2)) ORDER BY origin,window,part"
    } else {
        "SELECT origin,window,part,data,bytes,digest,codec,count FROM event_blocks WHERE bucket=?1 ORDER BY origin,window,part"
    };
    let mut query = db.prepare_cached(sql)?;
    let mut selected = threads.map_or_else(Vec::new, |(names, _)| names.to_vec());
    if threads.is_some_and(|(_, missing)| missing) {
        selected.push("");
    }
    let encoded = serde_json::to_string(&selected)?;
    let selected_ids: HashSet<&str> = selected.iter().copied().collect();
    let mut cursor = if threads.is_some() {
        query.query(params![origin, encoded])?
    } else {
        query.query([origin])?
    };
    let mut found = false;
    while let Some(row) = cursor.next()? {
        found = true;
        let expected = (row.get::<_, String>(0)?, row.get::<_, i64>(1)?);
        let events: Rows = codec::decode(
            row.get_ref(3)?.as_blob()?,
            row.get(4)?,
            row.get_ref(5)?.as_str()?,
            row.get(6)?,
        )?;
        #[cfg(test)]
        DECODED.set(DECODED.get() + events.len());
        require_index(
            events.len() as i64 == row.get::<_, i64>(7)?,
            "event block count mismatch",
        )?;
        validate_window(&events, &expected)?;
        for event in events {
            if threads.is_none() || selected_ids.contains(event.thread_id().unwrap_or("")) {
                consume(event)?;
            }
        }
    }
    Ok(found)
}

/// Sort block identities, never compressed BLOBs; decode one verified frame at a time.
pub(crate) fn cold_blocks(
    db: &Connection,
    scopes: &[String],
    targets: Option<&[String]>,
    mut consume: impl FnMut(
        crate::usage_store::EventTarget,
        crate::usage_store::EventChunk,
        Rows,
    ) -> Result<()>,
) -> Result<()> {
    metadata::prepare(db)?;
    for scope in scopes {
        validate_cold_source(db, scope)?;
    }
    let sql = if targets.is_some() {
        TARGET_ROWS
    } else {
        "SELECT e.rowid FROM event_blocks e WHERE bucket IN (SELECT coalesce(l.origin,b.id) FROM buckets b LEFT JOIN event_links l ON l.bucket=b.id WHERE b.field='events' AND b.scope IN (SELECT value FROM json_each(?1))) ORDER BY target,origin,window,part"
    };
    let mut query = db.prepare_cached(sql)?;
    let mut cursor = if let Some(targets) = targets {
        query.query(params![
            serde_json::to_string(scopes)?,
            serde_json::to_string(targets)?
        ])?
    } else {
        query.query([serde_json::to_string(scopes)?])?
    };
    let mut block = db.prepare_cached(
        "SELECT bytes,digest,codec,count,origin,window,target FROM event_blocks WHERE rowid=?1",
    )?;
    while let Some(row) = cursor.next()? {
        let id: i64 = row.get(0)?;
        let (bytes, digest, codec_version, count, origin, window, target) =
            block.query_row([id], |row| {
                Ok((
                    row.get::<_, i64>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                    row.get::<_, i64>(3)?,
                    row.get::<_, String>(4)?,
                    row.get::<_, i64>(5)?,
                    row.get::<_, String>(6)?,
                ))
            })?;
        let target: (Option<String>, Option<String>) = serde_json::from_str(&target)?;
        let owner = crate::usage_store::EventTarget {
            thread_id: target.0.clone(),
            turn_id: target.1.clone(),
        };
        if let Some(written) = metadata::load(db, id, &digest, bytes, count)? {
            require_index(
                written.target == owner,
                "written observation target mismatch",
            )?;
            consume(owner, written.chunk, written.boundaries)?;
            continue;
        }
        let data: Vec<u8> = db
            .prepare_cached("SELECT data FROM event_blocks WHERE rowid=?1")?
            .query_row([id], |r| r.get(0))?;
        let events: Rows = codec::decode(&data, bytes, &digest, codec_version)?;
        require_index(
            !events.is_empty() && events.len() <= 200 && events.len() as i64 == count,
            "invalid cold event block count",
        )?;
        validate_window(&events, &(origin, window))?;
        #[cfg(test)]
        DECODED.set(DECODED.get() + events.len());
        require_index(
            events.iter().all(|e| {
                (e.thread_id(), e.turn_id()) == (target.0.as_deref(), target.1.as_deref())
            }),
            "cold block target mismatch",
        )?;
        consume(
            owner,
            metadata::chunk(id, digest, bytes as u64, &events),
            events,
        )?;
    }
    metadata::clear(db)?;
    Ok(())
}

pub(crate) fn read_cold_chunk(
    db: &Connection,
    chunk: &crate::usage_store::EventChunk,
) -> Result<Rows> {
    let events: Rows = read_cold_chunk_as(db, chunk)?;
    require_index(
        events.len() == chunk.count,
        "cold event chunk count mismatch",
    )?;
    #[cfg(test)]
    DECODED.set(DECODED.get() + events.len());
    require_index(
        events.first().map(|e| e.position()) == Some(&chunk.first)
            && events.last().map(|e| e.position()) == Some(&chunk.last),
        "cold event chunk boundary mismatch",
    )?;
    validate_window(&events, &key(&events[0])?)?;
    Ok(events)
}

/// Both full evidence reads and message-only reads verify the same immutable frame.
pub(crate) fn read_cold_chunk_as<T: serde::de::DeserializeOwned>(
    db: &Connection,
    chunk: &crate::usage_store::EventChunk,
) -> Result<T> {
    let id = chunk
        .file
        .file
        .strip_prefix("live-block-")
        .and_then(|s| s.strip_suffix(".json"))
        .and_then(|s| s.parse::<i64>().ok())
        .filter(|id| *id > 0)
        .ok_or_else(|| {
            crate::dto::operation_error("INDEX_UNAVAILABLE", "invalid cold event locator")
        })?;
    let mut query = db.prepare_cached(
        "SELECT data,bytes,digest,codec,count,origin,window FROM event_blocks WHERE rowid=?1",
    )?;
    let mut cursor = query.query([id])?;
    let row = cursor.next()?.ok_or_else(|| {
        crate::dto::operation_error("INDEX_UNAVAILABLE", "missing cold event chunk")
    })?;
    require_index(
        row.get::<_, i64>(1)? == chunk.bytes as i64
            && row.get_ref(2)?.as_str()? == chunk.file.sha256
            && row.get::<_, i64>(4)? == chunk.count as i64,
        "cold event chunk identity mismatch",
    )?;
    let domain = crate::hash(serde_json::to_vec(&(
        &chunk.first.source_instance_id,
        &chunk.first.file_id,
        &chunk.first.generation,
    ))?);
    require_index(
        row.get::<_, String>(5)? == domain
            && row.get::<_, i64>(6)? == (chunk.first.byte_offset / WINDOW) as i64
            && chunk.last.source_instance_id == chunk.first.source_instance_id
            && chunk.last.file_id == chunk.first.file_id
            && chunk.last.generation == chunk.first.generation
            && chunk.last.byte_offset / WINDOW == chunk.first.byte_offset / WINDOW,
        "cold event chunk source mismatch",
    )?;
    codec::decode(
        row.get_ref(0)?.as_blob()?,
        row.get(1)?,
        row.get_ref(2)?.as_str()?,
        row.get(3)?,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::session_events::{ActivityKind, Payload, Position, Time};
    fn event(offset: u64, thread: Option<&str>) -> Arc<Event> {
        Arc::new(
            Event::new(
                Position {
                    source_instance_id: "source".into(),
                    file_id: "file".into(),
                    generation: "generation".into(),
                    byte_offset: offset,
                    ordinal: 0,
                },
                thread.map(Into::into),
                None,
                Time::from_source(None).0,
                vec![],
                Payload::Activity {
                    activity: ActivityKind::Assistant,
                },
            )
            .unwrap(),
        )
    }
    #[test]
    fn cached_window_keys_preserve_domains_offsets_and_capacity() {
        let template = event(0, None);
        let events: Vec<_> = (0..4100)
            .map(|i| {
                Event::new(
                    Position {
                        file_id: format!("file-{i}").into(),
                        byte_offset: i * WINDOW,
                        ..template.position().clone()
                    },
                    None,
                    None,
                    Time::from_source(None).0,
                    vec![],
                    template.payload().clone(),
                )
                .unwrap()
            })
            .collect();
        let mut cache = WindowKeys::default();
        for event in events.iter().chain(events.iter().rev()) {
            assert_eq!(cache.key(event).unwrap(), key(event).unwrap());
        }
        assert_eq!(cache.origins.len(), 4096);
        let next = Event::new(
            Position {
                byte_offset: WINDOW + 1,
                ..events[0].position().clone()
            },
            None,
            None,
            Time::from_source(None).0,
            vec![],
            template.payload().clone(),
        )
        .unwrap();
        assert_eq!(cache.key(&next).unwrap(), key(&next).unwrap());
        assert_eq!(cache.origins.len(), 4096);
    }
    #[test]
    fn window_validation_checks_every_event_domain_and_offset() {
        let first = event(1, Some("a"));
        let key = key(&first).unwrap();
        validate_window(&[Arc::clone(&first), event(2, Some("b"))], &key).unwrap();
        for changed in [
            Position {
                file_id: "foreign".into(),
                ..first.position().clone()
            },
            Position {
                generation: "new".into(),
                ..first.position().clone()
            },
            Position {
                source_instance_id: "other".into(),
                ..first.position().clone()
            },
            Position {
                byte_offset: WINDOW,
                ..first.position().clone()
            },
        ] {
            let foreign = Arc::new(
                Event::new(
                    changed,
                    Some("a".into()),
                    None,
                    Time::from_source(None).0,
                    vec![],
                    Payload::Activity {
                        activity: ActivityKind::Assistant,
                    },
                )
                .unwrap(),
            );
            assert!(validate_window(&[Arc::clone(&first), foreign], &key).is_err());
        }
    }
    #[test]
    fn source_windows_share_once_seek_exact_threads_and_survive_rollback_and_reopen() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("index.sqlite");
        let mut db = open(&path).unwrap();
        let initial = vec![
            event(1, Some("a")),
            event(2, Some("b")),
            event(WINDOW + 1, None),
        ];
        save(
            &db,
            "parser",
            &initial,
            true,
            &std::sync::atomic::AtomicBool::new(false),
        )
        .unwrap();
        share(&db, "projection", "parser").unwrap();
        assert!(share(&db, "third", "projection").is_err());
        assert!(share(&db, "parser", "parser").is_err());
        let before = load_map(&db, "projection").unwrap();
        assert_eq!(
            db.query_row("SELECT count(*) FROM event_links", [], |r| r
                .get::<_, i64>(0))
                .unwrap(),
            1
        );
        assert_eq!(
            db.query_row("SELECT count(*) FROM entries", [], |r| r.get::<_, i64>(0))
                .unwrap(),
            0
        );
        let cold: String = db
            .query_row("SELECT digest FROM event_blocks WHERE window=1", [], |r| {
                r.get(0)
            })
            .unwrap();
        {
            let tx = db.transaction().unwrap();
            save(
                &tx,
                "parser",
                &[event(3, Some("a"))],
                false,
                &std::sync::atomic::AtomicBool::new(false),
            )
            .unwrap();
            assert_eq!(
                load_map(&tx, "projection").unwrap()["events"]
                    .as_object()
                    .unwrap()
                    .len(),
                4
            );
        }
        assert_eq!(load_map(&db, "projection").unwrap(), before);
        save(
            &db,
            "parser",
            &[event(3, Some("a"))],
            false,
            &std::sync::atomic::AtomicBool::new(false),
        )
        .unwrap();
        assert_eq!(
            db.query_row("SELECT digest FROM event_blocks WHERE window=1", [], |r| {
                r.get::<_, String>(0)
            })
            .unwrap(),
            cold
        );
        drop(db);
        let db = reader(&path).unwrap();
        let mut ids = vec![];
        each(&db, "projection", Some((&["a"], true)), |id, _| {
            ids.push(id.to_owned());
            Ok(())
        })
        .unwrap();
        let mut typed = vec![];
        each_typed(&db, "projection", Some((&["a"], true)), |event| {
            typed.push(event.id().to_owned());
            Ok(())
        })
        .unwrap();
        assert_eq!(typed, ids);
        assert_eq!(ids.len(), 3);
        assert!(!ids.contains(&initial[1].id().to_owned()));
        assert_eq!(db.total_changes(), 0);
        drop(db);
        let db = open(&path).unwrap();
        save(
            &db,
            "parser",
            [&initial[1]],
            true,
            &std::sync::atomic::AtomicBool::new(false),
        )
        .unwrap();
        assert_eq!(
            load_map(&db, "projection").unwrap()["events"]
                .as_object()
                .unwrap()
                .len(),
            1
        );
        {
            let tx = db.unchecked_transaction().unwrap();
            tx.execute("DELETE FROM event_blocks", []).unwrap();
            assert!(
                load_map(&tx, "projection").is_err(),
                "missing observation blocks must not become empty history"
            );
        }
        db.execute("UPDATE event_blocks SET data=x'00'", [])
            .unwrap();
        assert!(load_map(&db, "projection").is_err());
        assert!(each_typed(&db, "projection", None, |_| Ok(())).is_err());
    }
}
