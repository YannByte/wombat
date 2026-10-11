//! Source-local observation windows; projection membership is one bucket link.
//! An append rewrites only touched windows, never the whole source archive.
use super::*;
use crate::session_events::Event;
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

const WINDOW: u64 = 256 * 1024;
type Window = (String, i64);
type Rows = Vec<Arc<Event>>;
pub(super) fn schema(db: &Connection) -> Result<()> {
    db.execute_batch("CREATE TABLE event_blocks(bucket INTEGER NOT NULL,origin TEXT NOT NULL,window INTEGER NOT NULL,part INTEGER NOT NULL,codec INTEGER NOT NULL,bytes INTEGER NOT NULL,digest TEXT NOT NULL,data BLOB NOT NULL,count INTEGER NOT NULL,UNIQUE(bucket,origin,window,part));
      CREATE TABLE event_targets(bucket INTEGER NOT NULL,thread TEXT NOT NULL,origin TEXT NOT NULL,window INTEGER NOT NULL,part INTEGER NOT NULL,PRIMARY KEY(bucket,thread,origin,window,part)) WITHOUT ROWID;
      CREATE TABLE event_links(bucket INTEGER PRIMARY KEY,origin INTEGER NOT NULL);
      CREATE TABLE event_heads(bucket INTEGER PRIMARY KEY,event_count INTEGER NOT NULL,block_count INTEGER NOT NULL);")?;
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
        for event in &events {
            require_index(self::key(event)? == *key, "event window identity mismatch")?;
        }
        result.extend(events);
    }
    Ok(result)
}
fn save_window(db: &Connection, bucket: i64, key: &Window, events: &[Arc<Event>]) -> Result<()> {
    let mut start = 0;
    let mut part = 0;
    while start < events.len() {
        let mut end = start;
        let mut size = 2;
        while end < events.len() {
            let n = serde_json::to_vec(&events[end])?.len() + 1;
            if end > start && (size + n > codec::TARGET || end - start >= 200) {
                break;
            }
            size += n;
            end += 1;
        }
        let (data, bytes, digest) = codec::encode(&events[start..end])?;
        let changed=db.prepare_cached("INSERT INTO event_blocks(bucket,origin,window,part,codec,bytes,digest,data,count) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9) ON CONFLICT(bucket,origin,window,part) DO UPDATE SET bytes=excluded.bytes,digest=excluded.digest,data=excluded.data,count=excluded.count WHERE digest!=excluded.digest")?.execute(params![bucket,key.0,key.1,part,codec::VERSION,bytes as i64,digest,data,(end-start) as i64])?;
        if changed > 0 {
            db.execute(
                "DELETE FROM event_targets WHERE bucket=?1 AND origin=?2 AND window=?3 AND part=?4",
                params![bucket, key.0, key.1, part],
            )?;
            let threads: BTreeSet<_> = events[start..end]
                .iter()
                .map(|e| e.thread_id().unwrap_or(""))
                .collect();
            let mut insert =
                db.prepare_cached("INSERT INTO event_targets VALUES(?1,?2,?3,?4,?5)")?;
            for thread in threads {
                insert.execute(params![bucket, thread, key.0, key.1, part])?;
            }
        }
        start = end;
        part += 1;
    }
    for table in ["event_targets", "event_blocks"] {
        db.execute(
            &format!(
                "DELETE FROM {table} WHERE bucket=?1 AND origin=?2 AND window=?3 AND part>=?4"
            ),
            params![bucket, key.0, key.1, part],
        )?;
    }
    Ok(())
}
pub(crate) fn save<'a>(
    db: &Connection,
    scope: &str,
    events: impl IntoIterator<Item = &'a Arc<Event>>,
    replace: bool,
    cancelled: &std::sync::atomic::AtomicBool,
) -> Result<()> {
    let bucket = bucket(db, scope, "events")?;
    let mut groups = BTreeMap::<Window, Vec<&Arc<Event>>>::new();
    for event in events {
        crate::operation_association::check(cancelled)?;
        groups.entry(key(event)?).or_default().push(event);
    }
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
        save_window(db, bucket, key, &rows)?;
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
                for table in ["event_targets", "event_blocks"] {
                    db.execute(
                        &format!("DELETE FROM {table} WHERE bucket=?1 AND origin=?2 AND window=?3"),
                        params![bucket, key.0, key.1],
                    )?;
                }
            }
        }
    }
    db.execute("INSERT INTO event_heads SELECT ?1,coalesce(sum(count),0),count(*) FROM event_blocks WHERE bucket=?1 ON CONFLICT(bucket) DO UPDATE SET event_count=excluded.event_count,block_count=excluded.block_count WHERE event_count!=excluded.event_count OR block_count!=excluded.block_count",[bucket])?;
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
    let origin:Option<i64>=db.prepare_cached("SELECT coalesce(l.origin,b.id) FROM buckets b LEFT JOIN event_links l ON l.bucket=b.id WHERE b.scope=?1 AND b.field='events'")?.query_row([scope],|r|r.get(0)).optional()?;
    let Some(origin) = origin else {
        return Ok(false);
    };
    check_head(db, origin)?;
    let sql = if threads.is_some() {
        "SELECT e.origin,e.window,e.part,e.data,e.bytes,e.digest,e.codec,e.count FROM (SELECT DISTINCT bucket,origin,window,part FROM event_targets WHERE bucket=?1 AND thread IN (SELECT value FROM json_each(?2))) t LEFT JOIN event_blocks e USING(bucket,origin,window,part) ORDER BY e.origin,e.window,e.part"
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
        require_index(
            events.len() as i64 == row.get::<_, i64>(7)?,
            "event block count mismatch",
        )?;
        for event in events {
            require_index(key(&event)? == expected, "event block source mismatch")?;
            if threads.is_none() || selected_ids.contains(event.thread_id().unwrap_or("")) {
                consume(event.id(), &serde_json::to_string(&event)?)?;
            }
        }
    }
    Ok(found)
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
    }
}
