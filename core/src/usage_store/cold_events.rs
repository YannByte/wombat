//! Read-only SQLite snapshots bind cold evidence to a committed live revision.
//! No observation payload is decoded at view restoration. Leased views retain WAL
//! pages until released; this is bounded view retention, not a hard disk quota.
use super::*;
use rusqlite::{Connection, OpenFlags};

type DecodedMeasurementFrame = Option<(i64, Vec<(usize, PricedMeasurement)>)>;

pub(super) struct ColdEvents(Mutex<Connection>, crate::pricing_sync::Response);
impl ColdEvents {
    pub(super) fn restore_details(&self, snapshot: &Snapshot) -> Result<Snapshot> {
        let db = self
            .0
            .lock()
            .map_err(|_| corrupt("detail lease unavailable"))?;
        let mut collected = Collected::default();
        for source in &snapshot.manifest.sources {
            let value =
                crate::live::load_collected_except(&db, &source.source.id, true, true, true)?
                    .ok_or_else(|| corrupt("detail projection missing"))?;
            collected.threads.extend(value.threads);
            collected.turns.extend(value.turns);
            collected
                .title_observations
                .extend(value.title_observations);
        }
        collected.sources = snapshot.manifest.sources.clone();
        collected.issues = snapshot.manifest.issues.clone();
        collected.watermarks = snapshot.manifest.watermarks.clone();
        let key = snapshot
            .manifest
            .snapshot_ref
            .snapshot_id
            .split(':')
            .nth(1)
            .ok_or_else(|| corrupt("detail identity invalid"))?;
        let events = load_index(&db, key)?.ok_or_else(|| corrupt("detail event index missing"))?;
        let operations =
            super::cold_operations::load_operations(&db, &collected.sources, &collected.threads)?;
        let mut detailed = super::memory::memory_cold_operations(
            collected,
            snapshot.manifest.snapshot_ref.snapshot_id.clone(),
            self.1.clone(),
            events,
            operations,
        )?;
        detailed.manifest.snapshot_ref = snapshot.manifest.snapshot_ref.clone();
        detailed.cold_events = snapshot.cold_events.clone();
        detailed.publication_change = snapshot.publication_change.clone();
        drop(db);
        detailed.add_measurement_turns()?;
        Ok(detailed)
    }
    pub(super) fn capture(
        db: &Connection,
        key: &str,
        id: &str,
        sources: &[SourceReport],
        prices: crate::pricing_sync::Response,
    ) -> Result<Arc<Self>> {
        let path = db
            .path()
            .filter(|s| !s.is_empty())
            .ok_or_else(|| corrupt("冷证据缺少持久索引"))?;
        let reader = Connection::open_with_flags(
            path,
            OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
        )?;
        reader.busy_timeout(std::time::Duration::from_secs(2))?;
        reader.execute_batch("PRAGMA cache_size=-2048; PRAGMA query_only=ON; BEGIN;")?;
        let revision = crate::live_index::scalar(&reader, &format!("view:{key}"), "id")?;
        if revision.as_ref().and_then(serde_json::Value::as_str) != Some(id) {
            return Err(operation_error(
                "VIEW_EXPIRED",
                "读取版本在证据捕获前已变化",
            ));
        }
        for source in sources {
            crate::live_index::validate_cold_source(
                &reader,
                &format!("projection:{}", source.source.id),
            )?;
            let (facts,summaries):(i64,i64)=reader.query_row("SELECT (SELECT count(*) FROM fact_rows r JOIN buckets b ON b.id=r.bucket WHERE b.scope=?1 AND b.field='measurements'),(SELECT coalesce(sum(count),0) FROM measurement_summaries WHERE source=?2)",rusqlite::params![format!("projection:{}",source.source.id),source.source.id],|r|Ok((r.get(0)?,r.get(1)?)))?;
            anyhow::ensure!(
                facts == summaries,
                "measurement aggregate coverage mismatch"
            );
        }
        Ok(Arc::new(Self(Mutex::new(reader), prices)))
    }
    pub(super) fn groups(
        &self,
        sources: &[SourceReport],
        tz: chrono_tz::Tz,
        thread: Option<&str>,
        turn: Option<&str>,
        selection: Option<&dyn Fn(&super::ledger_projection::LedgerGroup) -> bool>,
        mut consume: impl FnMut(
            &super::ledger_projection::LedgerGroup,
            &dyn Fn() -> Result<Vec<PricedMeasurement>>,
        ) -> Result<()>,
    ) -> Result<()> {
        let decoded: std::cell::RefCell<DecodedMeasurementFrame> = std::cell::RefCell::new(None);
        for source in sources {
            let mut after = (-1, -1);
            loop {
                let page = {
                    let db = self
                        .0
                        .lock()
                        .map_err(|_| corrupt("cold measurement lock unavailable"))?;
                    super::ledger_projection::group_page(
                        &db,
                        &source.source.id,
                        thread,
                        turn,
                        after,
                        selection,
                    )?
                };
                if page.is_empty() {
                    break;
                }
                for stored in page {
                    after = (stored.block, stored.part);
                    let block = stored.block;
                    let group = stored.decode(tz)?;
                    consume(&group, &|| {
                        let mut decoded = decoded.borrow_mut();
                        if decoded.as_ref().is_none_or(|(id, _)| *id != block) {
                            let db = self
                                .0
                                .lock()
                                .map_err(|_| corrupt("cold measurement lock unavailable"))?;
                            *decoded = Some((
                                block,
                                super::ledger_projection::block_rows(&db, block, &self.1)?,
                            ));
                        }
                        super::ledger_projection::group_rows(
                            &decoded.as_ref().unwrap().1,
                            &group,
                            turn,
                        )
                    })?;
                }
            }
        }
        Ok(())
    }

    pub(super) fn measurement_changes(
        &self,
        current: &Connection,
        sources: &[SourceReport],
    ) -> Result<(usize, usize, usize)> {
        let old = self
            .0
            .lock()
            .map_err(|_| corrupt("cold measurement lock unavailable"))?;
        let blocks = |db: &Connection| -> Result<BTreeMap<i64, String>> {
            let mut out = BTreeMap::new();
            let mut query=db.prepare_cached("SELECT f.id,f.digest FROM fact_blocks f JOIN buckets b ON b.id=f.bucket WHERE b.scope=?1 AND b.field='measurements'")?;
            for source in sources {
                let mut cursor = query.query([format!("projection:{}", source.source.id)])?;
                while let Some(row) = cursor.next()? {
                    out.insert(row.get(0)?, row.get(1)?);
                }
            }
            Ok(out)
        };
        let previous = blocks(&old)?;
        let next = blocks(current)?;
        let identities = |db: &Connection,
                          own: &BTreeMap<i64, String>,
                          other: &BTreeMap<i64, String>|
         -> Result<BTreeMap<String, String>> {
            let mut out = BTreeMap::new();
            for (block, digest) in own {
                if other.get(block) == Some(digest) {
                    continue;
                }
                for (_, fact) in crate::live_index::measurement_block(db, *block)? {
                    out.insert(fact.id.clone(), crate::hash(serde_json::to_vec(&fact)?));
                }
            }
            Ok(out)
        };
        let a = identities(&old, &previous, &next)?;
        let b = identities(current, &next, &previous)?;
        Ok((
            b.keys().filter(|id| !a.contains_key(*id)).count(),
            a.keys().filter(|id| !b.contains_key(*id)).count(),
            b.iter()
                .filter(|(id, digest)| a.get(*id).is_some_and(|old| old != *digest))
                .count(),
        ))
    }

    pub(super) fn operations(
        &self,
        locations: &[crate::live_index::FactLocation],
        charge: impl FnMut(u64) -> Result<()>,
    ) -> Result<Vec<Arc<Operation>>> {
        let db = self.0.lock().map_err(|_| corrupt("冷证据读取锁不可用"))?;
        crate::live_index::read_operations(&db, locations, charge)
    }
    pub(super) fn read(
        &self,
        chunk: &EventChunk,
    ) -> Result<Vec<Arc<crate::session_events::Event>>> {
        let db = self.0.lock().map_err(|_| corrupt("冷证据读取锁不可用"))?;
        crate::live_index::read_cold_chunk(&db, chunk)
    }
}

/// Complete reductions and locators need only one decoded frame, even for a
/// single target with millions of observations. The index itself remains O(blocks).
pub(crate) fn build_from_sources(
    db: &Connection,
    sources: &[SourceReport],
    prior: Option<&EventIndex>,
) -> Result<EventIndex> {
    use crate::timing::analysis::{BOUNDARY_METHOD_VERSION, BoundaryReducer};
    let scopes: Vec<_> = sources
        .iter()
        .map(|s| format!("projection:{}", s.source.id))
        .collect();
    let changed = prior
        .map(|_| crate::live_index::changed_event_targets(db))
        .transpose()?;
    let targets: std::collections::BTreeSet<EventTarget> = changed
        .iter()
        .flatten()
        .map(|t| {
            let (thread_id, turn_id) = serde_json::from_str::<(Option<String>, Option<String>)>(t)?;
            Ok(EventTarget { thread_id, turn_id })
        })
        .collect::<Result<_>>()?;
    let mut partitions = Vec::new();
    let mut active: Option<EventTarget> = None;
    let mut chunks = Vec::new();
    let mut reducer = BoundaryReducer::default();
    let mut source: Option<String> = None;
    let mut mixed = false;
    let finish = |target: EventTarget,
                  mut chunks: Vec<EventChunk>,
                  reducer: BoundaryReducer,
                  source: Option<String>,
                  mixed: bool|
     -> Result<Arc<EventPartition>> {
        chunks.sort_unstable_by(|a, b| super::events::position_order(&a.first, &b.first));
        let mut count = 0;
        let mut message_count = 0;
        for chunk in &mut chunks {
            chunk.offset = count;
            count += chunk.count;
            message_count += chunk.message_count;
        }
        let summary = if target.turn_id.is_some() && !mixed {
            reducer.finish()
        } else {
            BoundaryReducer::default().finish()
        };
        let mut partition = EventPartition {
            sha256: String::new(),
            target,
            count,
            chunks,
            native_boundary: super::native_boundary::NativeBoundaryIndex {
                method_version: BOUNDARY_METHOD_VERSION.into(),
                event_count: count,
                message_count,
                complete: true,
                source_instance_id: if mixed { None } else { source },
                summary,
            },
        };
        partition.sha256 = super::events::partition_hash(&partition)?;
        Ok(Arc::new(partition))
    };
    crate::live_index::cold_blocks(db, &scopes, changed.as_deref(), |target, chunk, events| {
        if active.as_ref().is_some_and(|a| *a != target) {
            partitions.push(finish(
                active.take().unwrap(),
                std::mem::take(&mut chunks),
                std::mem::take(&mut reducer),
                source.take(),
                mixed,
            )?);
            mixed = false;
        }
        active = Some(target.clone());
        let id = chunk.first.source_instance_id.as_ref();
        if source.as_ref().is_some_and(|s| s != id) {
            mixed = true;
        }
        if source.is_none() {
            source = Some(id.to_owned());
        }
        for event in events {
            if target.turn_id.is_some() {
                reducer.observe(&event);
            }
        }
        chunks.push(chunk);
        Ok(())
    })?;
    if let Some(target) = active {
        partitions.push(finish(target, chunks, reducer, source, mixed)?);
    }
    if let Some(prior) = prior {
        partitions.extend(
            prior
                .partitions
                .iter()
                .filter(|p| !targets.contains(&p.target))
                .cloned(),
        );
    }
    partitions.sort_unstable_by(|a, b| a.target.cmp(&b.target));
    let index = EventIndex {
        version: 1,
        partitions,
    };
    super::events::validate_index(&index)?;
    Ok(index)
}
pub(crate) fn save_index(
    db: &Connection,
    key: &str,
    index: &EventIndex,
    prior: Option<&EventIndex>,
) -> Result<()> {
    let scope = format!("view:{key}:evidence");
    crate::live_index::put(db, &scope, "version", "", &1_u32)?;
    let previous: BTreeMap<_, _> = prior
        .into_iter()
        .flat_map(|p| &p.partitions)
        .map(|p| (&p.target, p.sha256.as_str()))
        .collect();
    let values: Vec<_> = index
        .partitions
        .iter()
        .map(|p| Ok((crate::hash(serde_json::to_vec(&p.target)?), p)))
        .collect::<Result<_>>()?;
    crate::live_index::put_packed(
        db,
        &scope,
        "partitions",
        values
            .iter()
            .filter(|(_, p)| previous.get(&p.target).copied() != Some(p.sha256.as_str()))
            .map(|(id, p)| (id.as_str(), *p)),
    )?;
    crate::live_index::retain_packed(
        db,
        &scope,
        "partitions",
        values.iter().map(|(id, _)| id.as_str()),
    )?;
    crate::live_index::put(db, &scope, "count", "", &index.partitions.len())?;
    Ok(())
}
pub(crate) fn load_index(db: &Connection, key: &str) -> Result<Option<EventIndex>> {
    let scope = format!("view:{key}:evidence");
    let Some(version) = crate::live_index::scalar(db, &scope, "version")? else {
        return Ok(None);
    };
    if version.as_u64() != Some(1) {
        return Err(operation_error("UNSUPPORTED_VERSION", "不支持此冷证据索引"));
    }
    let mut values = BTreeMap::new();
    crate::live_index::each_without_events(db, &scope, |field, id, payload| {
        match field {
            "version" | "count" => {}
            "partitions" => {
                let partition: EventPartition = serde_json::from_str(payload)?;
                anyhow::ensure!(
                    id == crate::hash(serde_json::to_vec(&partition.target)?),
                    "cold partition identity mismatch"
                );
                anyhow::ensure!(
                    values.insert(partition.target.clone(), partition).is_none(),
                    "duplicate cold partition"
                );
            }
            _ => return Err(corrupt("未知冷证据索引字段")),
        }
        Ok(())
    })?;
    let count = crate::live_index::scalar(db, &scope, "count")?
        .and_then(|v| v.as_u64())
        .ok_or_else(|| corrupt("冷证据索引数量缺失"))?;
    if count as usize != values.len() {
        return Err(corrupt("冷证据索引不完整"));
    }
    let index = EventIndex {
        version: 1,
        partitions: values.into_values().map(Arc::new).collect(),
    };
    super::events::validate_index(&index)?;
    Ok(Some(index))
}
impl Snapshot {
    pub(crate) fn has_cold_lease(&self) -> bool {
        self.cold_events.is_some()
    }
    pub(crate) fn exclusive_cold_lease(&self) -> bool {
        self.is_summary_view()
            && self
                .cold_events
                .as_ref()
                .is_some_and(|r| Arc::strong_count(r) == 1)
            && self
                .lazy_details
                .as_ref()
                .is_some_and(|v| v.lock().is_ok_and(|v| v.is_none()))
    }
    pub(crate) fn release_cold_lease(&mut self) -> crate::pricing_sync::Response {
        let reader = self.cold_events.take().expect("exclusive lease checked");
        reader.1.clone()
    }
    pub(crate) fn reattach_cold_lease(
        &mut self,
        db: &Connection,
        key: &str,
        prices: crate::pricing_sync::Response,
    ) -> Result<()> {
        self.cold_events = Some(ColdEvents::capture(
            db,
            key,
            &self.manifest.snapshot_ref.snapshot_id,
            &self.manifest.sources,
            prices,
        )?);
        Ok(())
    }
    pub(crate) fn measurement_changes(
        &self,
        db: &Connection,
        sources: &[SourceReport],
    ) -> Result<Option<(usize, usize, usize)>> {
        self.cold_events
            .as_ref()
            .map(|reader| reader.measurement_changes(db, sources))
            .transpose()
    }
    pub(crate) fn capture_cold_events(
        &mut self,
        db: &Connection,
        key: &str,
        prices: crate::pricing_sync::Response,
    ) -> Result<()> {
        anyhow::ensure!(
            prices.catalog_hash == self.manifest.price_catalog_hash,
            "cold pricing catalog mismatch"
        );
        self.cold_events = Some(ColdEvents::capture(
            db,
            key,
            &self.manifest.snapshot_ref.snapshot_id,
            &self.manifest.sources,
            prices,
        )?);
        self.live_events = None;
        // Committed queries use aggregates and cold facts. The reader lease, not
        // a second canonical object graph, owns historical payload availability.
        self.add_measurement_turns()?;
        self.live_rows = Some(Arc::new(vec![]));
        self.project_rows = Some(Arc::new(BTreeMap::new()));
        self.summary_index = Arc::default();
        Ok(())
    }
    fn add_measurement_turns(&mut self) -> Result<()> {
        let reader = self
            .cold_events
            .as_ref()
            .ok_or_else(|| corrupt("measurement lease missing"))?;
        let db = reader
            .0
            .lock()
            .map_err(|_| corrupt("measurement lease unavailable"))?;
        if let Some(turns) = &mut self.memory_turns {
            let turns = Arc::make_mut(turns);
            let owners: BTreeMap<_, _> = self
                .manifest
                .threads
                .iter()
                .enumerate()
                .map(|(n, t)| (t.thread.id.clone(), n))
                .collect();
            let mut query=db.prepare_cached("SELECT DISTINCT thread,j.value FROM measurement_summaries,json_each(targets) j WHERE source=?1 AND thread IS NOT NULL")?;
            for source in &self.manifest.sources {
                let mut cursor = query.query([&source.source.id])?;
                while let Some(row) = cursor.next()? {
                    let thread: String = row.get(0)?;
                    let turn: Option<String> = row.get(1)?;
                    let turn = turn.unwrap_or_else(|| "unassigned".into());
                    turns.entry((thread.clone(), turn.clone())).or_default();
                    let position = owners
                        .get(&thread)
                        .ok_or_else(|| corrupt("measurement target metadata missing"))?;
                    Arc::make_mut(&mut self.manifest.threads[*position])
                        .turns
                        .entry(turn)
                        .or_insert_with(|| TurnEntry {
                            turn: None,
                            slice: Slice {
                                offset: 0,
                                length: 0,
                                sha256: String::new(),
                            },
                        });
                }
            }
        }
        Ok(())
    }
}
