//! Query reductions spill to a private SQLite file after their admitted working
//! set fills. SQLite owns ordering/indexing; this never writes the live ledger.
use crate::dto::operation_error;
use anyhow::Result;
use rusqlite::{Connection, OptionalExtension, params};
use serde::{Serialize, de::DeserializeOwned};
use std::collections::BTreeMap;

const MEMORY_BYTES: usize = 4 * 1024 * 1024;
const RECORD_BYTES: usize = 4 * 1024 * 1024;
const DISK_BYTES: usize = 128 * 1024 * 1024;
#[derive(Clone, Copy)]
pub(super) enum Order {
    Period,
    Dimensions,
    Names,
}
impl Order {
    fn sql(self) -> &'static str {
        match self {
            Self::Period => "json_extract(key,'$[0]') DESC,json_extract(key,'$[1]') DESC",
            Self::Dimensions => "json_extract(key,'$[1][0]'),json_extract(key,'$[1][1]')",
            Self::Names => "json_extract(key,'$')",
        }
    }
}
struct Disk {
    db: Connection,
    _file: tempfile::NamedTempFile,
    encoded_bytes: usize,
}
pub(super) struct WorkingSet<K, V> {
    memory: BTreeMap<K, (V, usize)>,
    bytes: usize,
    budget: usize,
    disk: Option<Disk>,
    order: Order,
}
impl<K: Ord + Serialize + DeserializeOwned, V: Clone + Serialize + DeserializeOwned>
    WorkingSet<K, V>
{
    pub(super) fn new(order: Order) -> Self {
        Self {
            memory: BTreeMap::new(),
            bytes: 0,
            budget: MEMORY_BYTES,
            disk: None,
            order,
        }
    }
    fn encode(key: &K, value: &V) -> Result<(String, String, usize)> {
        let key = serde_json::to_string(key)?;
        let payload = serde_json::to_string(value)?;
        let bytes = key
            .len()
            .checked_add(payload.len())
            .and_then(|b| b.checked_mul(3))
            .and_then(|b| b.checked_add(std::mem::size_of::<(K, V)>() + 128))
            .ok_or_else(|| operation_error("RESOURCE_LIMIT", "Query working size overflow"))?;
        if key.len() + payload.len() > RECORD_BYTES {
            return Err(operation_error(
                "RESOURCE_LIMIT",
                "Query reduction exceeds its record budget",
            ));
        }
        Ok((key, payload, bytes))
    }
    fn write(disk: &mut Disk, key: &str, payload: &str) -> Result<()> {
        let old:Option<(usize,usize)>=disk.db.query_row("SELECT length(CAST(key AS BLOB)),length(CAST(payload AS BLOB)) FROM reductions WHERE key=?1",[key],|r|Ok((r.get::<_, i64>(0)? as usize,r.get::<_, i64>(1)? as usize))).optional()?;
        let bytes = disk
            .encoded_bytes
            .saturating_sub(old.map_or(0, |(a, b)| a + b))
            .checked_add(key.len() + payload.len())
            .ok_or_else(|| operation_error("RESOURCE_LIMIT", "Query spill size overflow"))?;
        // Leave room for SQLite pages and the prefix index; check file growth too.
        if bytes > DISK_BYTES / 3 {
            return Err(operation_error(
                "RESOURCE_LIMIT",
                "Query reduction exceeds its disk budget",
            ));
        }
        disk.db.execute("INSERT INTO reductions VALUES(?1,?2) ON CONFLICT(key) DO UPDATE SET payload=excluded.payload",params![key,payload])?;
        disk.encoded_bytes = bytes;
        if disk._file.as_file().metadata()?.len() > DISK_BYTES as u64 {
            return Err(operation_error(
                "RESOURCE_LIMIT",
                "Query spill file exceeds its disk budget",
            ));
        }
        Ok(())
    }
    fn spill(&mut self) -> Result<()> {
        let file = tempfile::NamedTempFile::new()?;
        let db = Connection::open(file.path())?;
        db.execute_batch("PRAGMA journal_mode=OFF;PRAGMA synchronous=OFF;PRAGMA cache_size=-512;PRAGMA temp_store=FILE;CREATE TABLE reductions(key TEXT PRIMARY KEY,payload TEXT NOT NULL) WITHOUT ROWID;CREATE INDEX reduction_prefix ON reductions(json_extract(key,'$[0]'));")?;
        let mut disk = Disk {
            db,
            _file: file,
            encoded_bytes: 0,
        };
        for (key, (value, _)) in &self.memory {
            let (key, payload, _) = Self::encode(key, value)?;
            Self::write(&mut disk, &key, &payload)?;
        }
        self.memory.clear();
        self.bytes = 0;
        self.disk = Some(disk);
        Ok(())
    }
    pub(super) fn update(
        &mut self,
        key: K,
        create: impl FnOnce() -> Result<V>,
        change: impl FnOnce(&mut V) -> Result<()>,
    ) -> Result<()> {
        if let Some(disk) = &mut self.disk {
            let encoded = serde_json::to_string(&key)?;
            let payload: Option<String> = disk
                .db
                .query_row(
                    "SELECT payload FROM reductions WHERE key=?1",
                    [&encoded],
                    |r| r.get(0),
                )
                .optional()?;
            let mut value = match payload {
                Some(p) => serde_json::from_str(&p)?,
                None => create()?,
            };
            change(&mut value)?;
            let (key, payload, _) = Self::encode(&key, &value)?;
            return Self::write(disk, &key, &payload);
        }
        let (mut value, old) = match self.memory.remove(&key) {
            Some(value) => value,
            None => (create()?, 0),
        };
        self.bytes -= old;
        change(&mut value)?;
        let (_, _, bytes) = Self::encode(&key, &value)?;
        // Spill before admission, including when one valid record exceeds the
        // in-memory working budget. The record bound applies to either tier.
        if bytes > self.budget || self.bytes + bytes > self.budget {
            self.spill()?;
            let (key, payload, _) = Self::encode(&key, &value)?;
            return Self::write(self.disk.as_mut().unwrap(), &key, &payload);
        }
        self.bytes += bytes;
        self.memory.insert(key, (value, bytes));
        Ok(())
    }
    pub(super) fn get(&self, key: &K) -> Result<Option<V>> {
        if let Some(disk) = &self.disk {
            let key = serde_json::to_string(key)?;
            let payload: Option<String> = disk
                .db
                .query_row("SELECT payload FROM reductions WHERE key=?1", [key], |r| {
                    r.get(0)
                })
                .optional()?;
            return payload
                .map(|p| serde_json::from_str(&p).map_err(Into::into))
                .transpose();
        }
        Ok(self.memory.get(key).map(|(v, _)| v.clone()))
    }
    pub(super) fn each(
        &self,
        prefix: Option<&str>,
        mut consume: impl FnMut(&K, &V) -> Result<()>,
    ) -> Result<()> {
        if let Some(disk) = &self.disk {
            let sql = format!(
                "SELECT key,payload FROM reductions {} ORDER BY {}",
                if prefix.is_some() {
                    "WHERE json_extract(key,'$[0]') IS json_extract(?1,'$')"
                } else {
                    ""
                },
                self.order.sql()
            );
            let mut query = disk.db.prepare(&sql)?;
            let mut cursor = if let Some(prefix) = prefix {
                query.query([prefix])?
            } else {
                query.query([])?
            };
            while let Some(row) = cursor.next()? {
                let key: K = serde_json::from_str(row.get_ref(0)?.as_str()?)?;
                let value: V = serde_json::from_str(row.get_ref(1)?.as_str()?)?;
                consume(&key, &value)?;
            }
            return Ok(());
        }
        let matches = |key: &K| -> Result<bool> {
            Ok(prefix.is_none_or(|p| {
                serde_json::to_value(key)
                    .ok()
                    .and_then(|v| v.get(0).cloned())
                    == serde_json::from_str(p).ok()
            }))
        };
        if matches!(self.order, Order::Period) {
            for (key, (value, _)) in self.memory.iter().rev() {
                if matches(key)? {
                    consume(key, value)?;
                }
            }
        } else {
            for (key, (value, _)) in &self.memory {
                if matches(key)? {
                    consume(key, value)?;
                }
            }
        }
        Ok(())
    }
}

impl WorkingSet<(String, String), u8> {
    pub(super) fn insert(&mut self, key: (String, String)) -> Result<()> {
        if self.disk.is_none() && self.memory.contains_key(&key) {
            return Ok(());
        }
        let (encoded, _, bytes) = Self::encode(&key, &1)?;
        if self.disk.is_none() && self.bytes + bytes > self.budget {
            self.spill()?;
        }
        if let Some(disk) = &mut self.disk {
            // A distinct relation needs one insert, with no payload read or rewrite.
            let changed = disk
                .db
                .prepare_cached("INSERT OR IGNORE INTO reductions VALUES(?1,'1')")?
                .execute([&encoded])?;
            disk.encoded_bytes += changed * (encoded.len() + 1);
            if disk.encoded_bytes > DISK_BYTES / 3
                || disk._file.as_file().metadata()?.len() > DISK_BYTES as u64
            {
                return Err(operation_error(
                    "RESOURCE_LIMIT",
                    "Query distinct relation exceeds its disk budget",
                ));
            }
        } else {
            self.bytes += bytes;
            self.memory.insert(key, (1, bytes));
        }
        Ok(())
    }

    pub(super) fn count(&self, thread: &str) -> Result<usize> {
        if let Some(disk) = &self.disk {
            return Ok(disk.db.query_row("SELECT count(*) FROM reductions WHERE json_extract(key,'$[0]') IS json_extract(?1,'$')", [serde_json::to_string(thread)?], |r| r.get::<_, i64>(0))? as usize);
        }
        Ok(self
            .memory
            .range((thread.to_owned(), String::new())..)
            .take_while(|((id, _), _)| id == thread)
            .count())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn distinct_turn_relation_keeps_exact_counts_in_memory_and_spill() {
        for budget in [1, MEMORY_BYTES] {
            let mut turns = WorkingSet::<(String, String), u8>::new(Order::Names);
            turns.budget = budget;
            for thread in ["a", "b"] {
                for turn in ["x", "y", "x"] {
                    turns.insert((thread.into(), turn.into())).unwrap();
                }
            }
            assert_eq!(turns.count("a").unwrap(), 2);
            assert_eq!(turns.count("b").unwrap(), 2);
            assert_eq!(turns.count("missing").unwrap(), 0);
        }
    }
    #[test]
    fn numeric_calendar_keys_keep_chronological_order_across_eras() {
        for budget in [1, MEMORY_BYTES] {
            let mut values = WorkingSet::<Option<(i32, i32)>, u64>::new(Order::Period);
            values.budget = budget;
            for key in [None, Some((-1000, -999)), Some((0, 1)), Some((1000, 1001))] {
                values
                    .update(
                        key,
                        || Ok(0),
                        |v| {
                            *v += 1;
                            Ok(())
                        },
                    )
                    .unwrap();
            }
            let mut keys = vec![];
            values
                .each(None, |key, _| {
                    keys.push(*key);
                    Ok(())
                })
                .unwrap();
            assert_eq!(
                keys,
                vec![Some((1000, 1001)), Some((0, 1)), Some((-1000, -999)), None]
            );
        }
    }
    #[test]
    fn spill_retains_exact_updates_order_and_private_cleanup() {
        let mut values = WorkingSet::<Option<(String, String)>, u64>::new(Order::Period);
        values.budget = 256;
        for n in 0..100 {
            let key = Some((format!("2026-10-{n:03}"), "end".into()));
            values
                .update(
                    key,
                    || Ok(0),
                    |v| {
                        *v += 110;
                        Ok(())
                    },
                )
                .unwrap();
        }
        values
            .update(
                None,
                || Ok(0),
                |v| {
                    *v += 7;
                    Ok(())
                },
            )
            .unwrap();
        values
            .update(
                Some(("2026-10-050".into(), "end".into())),
                || Ok(0),
                |v| {
                    *v += 20;
                    Ok(())
                },
            )
            .unwrap();
        assert!(values.disk.is_some());
        assert!(values.bytes <= values.budget);
        let mut sum = 0;
        let mut keys = vec![];
        values
            .each(None, |k, v| {
                sum += v;
                keys.push(k.clone());
                Ok(())
            })
            .unwrap();
        assert_eq!(sum, 11027);
        assert_eq!(keys.last(), Some(&None));
        assert!(keys.windows(2).all(|p| p[0] >= p[1]));
        let path = values.disk.as_ref().unwrap()._file.path().to_owned();
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            assert_eq!(
                std::fs::metadata(&path).unwrap().permissions().mode() & 0o077,
                0
            );
        }
        drop(values);
        assert!(!path.exists());
    }
    #[test]
    fn memory_and_spilled_nullable_dimension_order_match() {
        let mut memory = WorkingSet::<
            (Option<(String, String)>, (Option<String>, Option<String>)),
            u64,
        >::new(Order::Dimensions);
        let mut disk = WorkingSet::<
            (Option<(String, String)>, (Option<String>, Option<String>)),
            u64,
        >::new(Order::Dimensions);
        disk.budget = 1;
        let period = Some(("2026-10-11".into(), "2026-10-12".into()));
        for key in [
            (None, None),
            (Some("a".into()), None),
            (Some("a".into()), Some("x".into())),
        ] {
            for set in [&mut memory, &mut disk] {
                set.update(
                    (period.clone(), key.clone()),
                    || Ok(0),
                    |v| {
                        *v += 1;
                        Ok(())
                    },
                )
                .unwrap();
            }
        }
        let prefix = serde_json::to_string(&period).unwrap();
        let mut a = vec![];
        let mut b = vec![];
        memory
            .each(Some(&prefix), |k, v| {
                a.push((k.clone(), *v));
                Ok(())
            })
            .unwrap();
        disk.each(Some(&prefix), |k, v| {
            b.push((k.clone(), *v));
            Ok(())
        })
        .unwrap();
        assert_eq!(a, b);
    }
}
