//! Committed, frame-local sufficient statistics. Complete measurements stay in
//! packed canonical blocks; only recently accessed aggregates enter the hot cache.
use super::*;
use crate::usage_app::Aggregate;
use chrono::{DateTime, TimeZone, Utc};
use chrono_tz::Tz;
use rusqlite::{Connection, params};
use std::collections::{HashMap, VecDeque};
use std::sync::OnceLock;

const VERSION: i64 = 3;
const HOT_BYTES: usize = 8 * 1024 * 1024;
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub(crate) struct GroupKey {
    pub agent: String,
    pub source: String,
    pub thread: Option<String>,
    pub turn: Option<String>,
    pub model: Option<String>,
    pub effort: Option<String>,
    time: GroupTime,
}
#[derive(Clone, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
enum GroupTime {
    Hour(i64),
    Date(String),
    Unknown,
}
#[derive(Clone, Serialize, Deserialize)]
pub(crate) struct LedgerGroup {
    pub key: GroupKey,
    pub aggregate: Aggregate,
    pub slots: Vec<usize>,
    pub members: Vec<(Option<String>, Vec<usize>)>,
    pub earliest: Option<i64>,
    pub latest: Option<i64>,
    pub latest_text: Option<String>,
}
#[derive(Clone, Serialize, Deserialize)]
struct Catalog {
    key: GroupKey,
    earliest: Option<i64>,
    latest: Option<i64>,
    latest_text: Option<String>,
    count: i64,
}
impl Catalog {
    fn from_group(group: &LedgerGroup) -> Self {
        Self {
            key: group.key.clone(),
            earliest: group.earliest,
            latest: group.latest,
            latest_text: group.latest_text.clone(),
            count: group.slots.len() as i64,
        }
    }
    fn stub(&self, empty: &Aggregate) -> LedgerGroup {
        LedgerGroup {
            key: self.key.clone(),
            aggregate: empty.clone(),
            slots: vec![],
            members: vec![],
            earliest: self.earliest,
            latest: self.latest,
            latest_text: self.latest_text.clone(),
        }
    }
}
impl LedgerGroup {
    fn key(row: &PricedMeasurement) -> GroupKey {
        let time = row
            .fact
            .timestamp
            .as_deref()
            .and_then(|t| DateTime::parse_from_rfc3339(t).ok());
        let time = if let Some(t) = time {
            GroupTime::Hour(t.timestamp().div_euclid(3600))
        } else if row.fact.time_precision.as_ref() == "date"
            && row.fact.timestamp.as_deref().is_some_and(|t| {
                chrono::NaiveDate::parse_from_str(t, "%Y-%m-%d")
                    .is_ok_and(|d| d.format("%Y-%m-%d").to_string() == t)
            })
        {
            GroupTime::Date(row.fact.timestamp.clone().unwrap())
        } else {
            GroupTime::Unknown
        };
        GroupKey {
            agent: row.fact.agent_kind.to_string(),
            source: row.fact.source_instance_id.to_string(),
            thread: row.fact.thread_id.as_deref().map(str::to_owned),
            turn: None,
            model: row.fact.model.raw.as_deref().map(str::to_owned),
            effort: row.fact.reasoning_effort.as_deref().map(str::to_owned),
            time,
        }
    }
    pub(crate) fn selected_slots(&self, turn: Option<&str>) -> Vec<usize> {
        if let Some(turn) = turn {
            self.members
                .iter()
                .filter(|(id, _)| {
                    id.as_deref() == Some(turn) || (turn == "unassigned" && id.is_none())
                })
                .flat_map(|(_, slots)| slots.iter().copied())
                .collect()
        } else {
            self.slots.clone()
        }
    }
    fn targets(&self) -> Result<String> {
        Ok(serde_json::to_string(
            &self
                .members
                .iter()
                .map(|(id, _)| id.as_deref())
                .collect::<Vec<_>>(),
        )?)
    }
    pub(crate) fn dates(&self, tz: Tz) -> (Option<chrono::NaiveDate>, Option<chrono::NaiveDate>) {
        match &self.key.time {
            GroupTime::Date(d) => {
                let d = chrono::NaiveDate::parse_from_str(d, "%Y-%m-%d").ok();
                (d, d)
            }
            GroupTime::Unknown => (None, None),
            GroupTime::Hour(_) => {
                let date = |s: Option<i64>| {
                    s.and_then(|s| Utc.timestamp_opt(s, 0).single())
                        .map(|t| t.with_timezone(&tz).date_naive())
                };
                (date(self.earliest), date(self.latest))
            }
        }
    }
    pub(crate) fn single_date(&self, tz: Tz) -> Option<Option<chrono::NaiveDate>> {
        let (first, last) = self.dates(tz);
        if first != last {
            return None;
        }
        if let GroupTime::Hour(hour) = self.key.time {
            let start = Utc.timestamp_opt(hour.checked_mul(3600)?, 0).single()?;
            let end = start.checked_add_signed(chrono::Duration::seconds(3599))?;
            // A rollback shortly after midnight can hide another date between
            // equal endpoint dates. Any offset transition disables this shortcut.
            if start.with_timezone(&tz).offset() != end.with_timezone(&tz).offset() {
                return None;
            }
        }
        Some(first)
    }
    fn resident_bytes(&self) -> usize {
        // Include container capacity and every owned/shared text allocation. Shared
        // price strings are conservatively charged for each reference.
        let text = |s: &str| s.len() + 2 * std::mem::size_of::<usize>();
        let optional = |s: &Option<String>| s.as_ref().map_or(0, |s| s.capacity());
        let p = &self.aggregate.usage.price;
        std::mem::size_of::<Self>()
            + self.slots.capacity() * std::mem::size_of::<usize>()
            + self.members.capacity() * std::mem::size_of::<(Option<String>, Vec<usize>)>()
            + self
                .members
                .iter()
                .map(|(id, slots)| {
                    id.as_ref().map_or(0, String::capacity)
                        + slots.capacity() * std::mem::size_of::<usize>()
                })
                .sum::<usize>()
            + self.key.agent.capacity()
            + self.key.source.capacity()
            + optional(&self.key.thread)
            + optional(&self.key.turn)
            + optional(&self.key.model)
            + optional(&self.key.effort)
            + match &self.key.time {
                GroupTime::Date(d) => d.capacity(),
                _ => 0,
            }
            + optional(&self.latest_text)
            + p.components.capacity() * std::mem::size_of::<Arc<crate::pricing::PriceComponent>>()
            + p.components
                .iter()
                .map(|c| {
                    std::mem::size_of::<crate::pricing::PriceComponent>()
                        + text(&c.category)
                        + c.cost.as_ref().map_or(0, String::capacity)
                        + c.known_cost.capacity()
                        + text(&c.status)
                        + c.rate_per_million.as_deref().map_or(0, text)
                })
                .sum::<usize>()
            + p.basis.capacity() * std::mem::size_of::<crate::pricing::PriceBasis>()
            + p.basis
                .iter()
                .map(|b| {
                    [
                        &*b.original_model,
                        &*b.pricing_model,
                        &*b.model_provider,
                        &*b.match_method,
                        &*b.source,
                        &*b.verified_at,
                        &*b.price_revision,
                        &*b.catalog_hash,
                        &*b.condition,
                    ]
                    .into_iter()
                    .map(text)
                    .sum::<usize>()
                        + b.api_provider.as_deref().map_or(0, text)
                })
                .sum::<usize>()
            + p.issues.capacity() * std::mem::size_of::<Arc<str>>()
            + p.issues.iter().map(|s| text(s)).sum::<usize>()
            + [&*p.currency, &*p.policy, &*p.price_revision, &*p.status]
                .into_iter()
                .map(text)
                .sum::<usize>()
            + p.cost.as_ref().map_or(0, String::capacity)
            + p.known_cost.capacity()
    }
}

pub(crate) fn schema(db: &Connection) -> Result<()> {
    db.execute_batch("CREATE TABLE measurement_summaries(source TEXT NOT NULL,block INTEGER NOT NULL,part INTEGER NOT NULL,thread TEXT,targets TEXT NOT NULL,version INTEGER NOT NULL,bytes INTEGER NOT NULL,digest TEXT NOT NULL,count INTEGER NOT NULL,catalog TEXT NOT NULL,catalog_digest TEXT NOT NULL,data BLOB NOT NULL,PRIMARY KEY(source,block,part)) WITHOUT ROWID; CREATE INDEX measurement_summaries_target ON measurement_summaries(source,thread,block,part);")?;
    Ok(())
}
/// Rewrite statistics only for changed canonical frames, in the source transaction.
/// Corrections rebuild one bounded frame, so removal/min/max/distinct semantics
/// do not require inverse arithmetic or an approximate aggregate.
pub(crate) fn update(
    db: &Connection,
    source: &str,
    prices: &crate::pricing_sync::Response,
    force: bool,
) -> Result<()> {
    db.execute_batch("CREATE TEMP TABLE IF NOT EXISTS changed_fact_blocks(bucket INTEGER,block INTEGER,PRIMARY KEY(bucket,block)) WITHOUT ROWID;")?;
    db.execute("DELETE FROM measurement_summaries WHERE source=?1 AND block NOT IN (SELECT f.id FROM fact_blocks f JOIN buckets b ON b.id=f.bucket WHERE b.scope=?2 AND b.field='measurements')",params![source,format!("projection:{source}")])?;
    let mut query=db.prepare("SELECT f.id FROM fact_blocks f JOIN buckets b ON b.id=f.bucket WHERE b.scope=?1 AND b.field='measurements' AND (?2 OR EXISTS(SELECT 1 FROM changed_fact_blocks c WHERE c.bucket=f.bucket AND c.block=f.id)) ORDER BY f.id")?;
    let mut cursor = query.query(params![format!("projection:{source}"), force])?;
    let mut pool = super::price_pool::PricePool::new(prices);
    let mut insert = db.prepare_cached(
        "INSERT INTO measurement_summaries(source,block,part,thread,targets,version,bytes,digest,data,count,catalog,catalog_digest) VALUES(?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12)",
    )?;
    while let Some(row) = cursor.next()? {
        let block: i64 = row.get(0)?;
        db.execute(
            "DELETE FROM measurement_summaries WHERE source=?1 AND block=?2",
            params![source, block],
        )?;
        let mut groups = BTreeMap::<GroupKey, LedgerGroup>::new();
        for (slot, fact) in crate::live_index::measurement_block(db, block)? {
            // Packed rows removed from membership are tombstoned in the frame.
            anyhow::ensure!(
                fact.source_instance_id.as_ref() == source,
                "aggregate source mismatch"
            );
            let fact = Arc::new(fact);
            let value = PricedMeasurement {
                price: pool.price(&fact),
                fact,
            };
            let key = LedgerGroup::key(&value);
            let time = value
                .fact
                .timestamp
                .as_deref()
                .and_then(|t| DateTime::parse_from_rfc3339(t).ok())
                .map(|t| t.timestamp());
            let aggregate = Aggregate::new(&[&value])?;
            if let Some(group) = groups.get_mut(&key) {
                group.aggregate.combine(&aggregate)?;
                group.slots.push(slot);
                if let Some((_, slots)) = group
                    .members
                    .iter_mut()
                    .find(|(id, _)| id.as_deref() == value.fact.turn_id.as_deref())
                {
                    slots.push(slot);
                } else {
                    group
                        .members
                        .push((value.fact.turn_id.as_deref().map(str::to_owned), vec![slot]));
                }
                group.earliest = group.earliest.into_iter().chain(time).min();
                group.latest = group.latest.into_iter().chain(time).max();
                group.latest_text = group
                    .latest_text
                    .take()
                    .into_iter()
                    .chain(value.fact.timestamp.clone())
                    .max();
            } else {
                groups.insert(
                    key.clone(),
                    LedgerGroup {
                        key,
                        aggregate,
                        slots: vec![slot],
                        members: vec![(
                            value.fact.turn_id.as_deref().map(str::to_owned),
                            vec![slot],
                        )],
                        earliest: time,
                        latest: time,
                        latest_text: value.fact.timestamp.clone(),
                    },
                );
            }
        }
        for (part, mut group) in groups.into_values().enumerate() {
            group.members.sort_unstable_by(|a, b| a.0.cmp(&b.0));
            let (data, bytes, digest) = crate::live_index::encode_projection(&group)?;
            let catalog = serde_json::to_string(&Catalog::from_group(&group))?;
            let catalog_digest = crate::hash(catalog.as_bytes());
            insert.execute(params![
                source,
                block,
                part as i64,
                group.key.thread,
                group.targets()?,
                VERSION,
                bytes as i64,
                digest,
                data,
                group.slots.len() as i64,
                catalog,
                catalog_digest
            ])?;
        }
    }
    Ok(())
}

#[derive(Default)]
pub(super) struct HotCache {
    entries: HashMap<String, (Arc<LedgerGroup>, usize)>,
    order: VecDeque<String>,
    bytes: usize,
}
pub(super) fn hot_cache() -> &'static Mutex<HotCache> {
    static CACHE: OnceLock<Mutex<HotCache>> = OnceLock::new();
    CACHE.get_or_init(Mutex::default)
}
impl HotCache {
    fn get(&self, key: &str) -> Option<Arc<LedgerGroup>> {
        self.entries.get(key).map(|(v, _)| Arc::clone(v))
    }
    fn insert(&mut self, key: String, value: Arc<LedgerGroup>, tz: Tz) {
        let today = Utc::now().with_timezone(&tz).date_naive();
        let (first, last) = value.dates(tz);
        if last.is_none_or(|d| d < today - chrono::Duration::days(29))
            || first.is_none_or(|d| d > today)
        {
            return;
        }
        let bytes = 2 * key.capacity()
            + value.resident_bytes()
            + std::mem::size_of::<(String, Arc<LedgerGroup>, usize)>();
        if bytes > HOT_BYTES || self.entries.contains_key(&key) {
            return;
        }
        while self.bytes + bytes > HOT_BYTES || self.entries.len() >= 4096 {
            if let Some(old) = self.order.pop_front() {
                if let Some((_, size)) = self.entries.remove(&old) {
                    self.bytes -= size;
                }
            } else {
                break;
            }
        }
        self.bytes += bytes;
        self.order.push_back(key.clone());
        self.entries.insert(key, (value, bytes));
    }
}
/// Copy one bounded page while holding the leased connection, then release it
/// before reduction/decompression. Historical work cannot hold the hot cache lock.
pub(super) struct StoredGroup {
    catalog: Catalog,
    excluded: bool,
    stub: Option<Arc<LedgerGroup>>,
    pub block: i64,
    pub part: i64,
    version: i64,
    bytes: i64,
    digest: String,
    data: Vec<u8>,
    source: String,
    thread: Option<String>,
    targets: String,
    count: i64,
}
pub(super) fn group_page(
    db: &Connection,
    source: &str,
    thread: Option<&str>,
    turn: Option<&str>,
    after: (i64, i64),
    selection: Option<&dyn Fn(&LedgerGroup) -> bool>,
) -> Result<Vec<StoredGroup>> {
    let mut query=db.prepare_cached("SELECT block,part,version,bytes,digest,CASE WHEN ?6 THEN NULL ELSE data END,thread,targets,count,catalog,catalog_digest FROM measurement_summaries WHERE source=?1 AND (?2 IS NULL OR thread=?2) AND (?3 IS NULL OR EXISTS(SELECT 1 FROM json_each(targets) j WHERE j.value=?3 OR (?3='unassigned' AND j.value IS NULL))) AND (block,part)>(?4,?5) ORDER BY block,part LIMIT ?7")?;
    let mut cursor = query.query(params![
        source,
        thread,
        turn,
        after.0,
        after.1,
        selection.is_some(),
        if selection.is_some() { 256 } else { 16 }
    ])?;
    let mut out = vec![];
    let mut bytes = 0;
    let empty = Aggregate::new(&[])?;
    let mut payload_query = db.prepare_cached(
        "SELECT data FROM measurement_summaries WHERE source=?1 AND block=?2 AND part=?3",
    )?;
    while let Some(row) = cursor.next()? {
        let encoded: i64 = row.get(3)?;
        anyhow::ensure!(
            encoded > 0 && encoded <= 4 * 1024 * 1024,
            "aggregate size exceeds frame limit"
        );
        // The first valid frame is allowed; further frames wait for the next page.
        let raw: String = row.get(9)?;
        let digest: String = row.get(10)?;
        anyhow::ensure!(
            raw.len() <= 16 * 1024 && crate::hash(raw.as_bytes()) == digest,
            "aggregate catalog checksum mismatch"
        );
        let catalog: Catalog = serde_json::from_str(&raw)?;
        anyhow::ensure!(
            catalog.key.source == source
                && catalog.key.thread == row.get::<_, Option<String>>(6)?
                && catalog.count == row.get::<_, i64>(8)?
                && catalog.count > 0,
            "aggregate catalog identity mismatch"
        );
        let stub = selection.map(|_| Arc::new(catalog.stub(&empty)));
        let excluded = selection.is_some_and(|select| !select(stub.as_ref().unwrap()));
        if !excluded && !out.is_empty() && bytes + encoded > 512 * 1024 {
            break;
        }
        if !excluded {
            bytes += encoded;
        }
        let block: i64 = row.get(0)?;
        let part: i64 = row.get(1)?;
        let data = if excluded {
            vec![]
        } else if selection.is_some() {
            payload_query.query_row(params![source, block, part], |r| r.get(0))?
        } else {
            row.get(5)?
        };
        out.push(StoredGroup {
            catalog,
            excluded,
            stub: excluded.then_some(stub).flatten(),
            block,
            part,
            version: row.get(2)?,
            bytes: encoded,
            digest: row.get(4)?,
            data,
            source: source.into(),
            thread: row.get(6)?,
            targets: row.get(7)?,
            count: row.get(8)?,
        });
    }
    Ok(out)
}
impl StoredGroup {
    pub(super) fn decode(self, tz: Tz) -> Result<Arc<LedgerGroup>> {
        if self.version != VERSION {
            return Err(operation_error(
                "INDEX_UNSUPPORTED_VERSION",
                "Unsupported measurement aggregate version",
            ));
        }
        if self.excluded {
            return self
                .stub
                .ok_or_else(|| corrupt("aggregate catalog selection missing"));
        }
        let cache_key = format!("{}:{}", self.digest, crate::hash(&self.data));
        let cached = hot_cache()
            .lock()
            .map_err(|_| corrupt("hot aggregate cache lock unavailable"))?
            .get(&cache_key);
        let group = match cached {
            Some(group) => group,
            None => Arc::new(crate::live_index::decode_projection::<LedgerGroup>(
                &self.data,
                self.bytes,
                &self.digest,
                1,
            )?),
        };
        anyhow::ensure!(
            group.key.source == self.source
                && group.key == self.catalog.key
                && group.earliest == self.catalog.earliest
                && group.latest == self.catalog.latest
                && group.latest_text == self.catalog.latest_text
                && group.key.thread == self.thread
                && group.key.turn.is_none()
                && group.targets()? == self.targets
                && !group.slots.is_empty()
                && group.slots.len() <= 128
                && group.slots.iter().all(|s| *s < 128)
                && group.slots.windows(2).all(|s| s[0] < s[1])
                && group.aggregate.usage.measurement_count == group.slots.len()
                && self.count == group.slots.len() as i64,
            "aggregate identity or count mismatch"
        );
        let mut slots = group
            .members
            .iter()
            .flat_map(|(_, slots)| slots.iter().copied())
            .collect::<Vec<_>>();
        slots.sort_unstable();
        anyhow::ensure!(
            slots == group.slots
                && group
                    .members
                    .iter()
                    .all(|(_, slots)| !slots.is_empty() && slots.windows(2).all(|s| s[0] < s[1]))
                && group.members.windows(2).all(|m| m[0].0 < m[1].0),
            "aggregate membership mismatch"
        );
        hot_cache()
            .lock()
            .map_err(|_| corrupt("hot aggregate cache lock unavailable"))?
            .insert(cache_key, Arc::clone(&group), tz);
        Ok(group)
    }
}

pub(crate) fn identity_digests(
    db: &Connection,
    sources: &[SourceReport],
) -> Result<BTreeMap<String, String>> {
    let mut out = BTreeMap::new();
    let mut query=db.prepare_cached("SELECT f.id FROM fact_blocks f JOIN buckets b ON b.id=f.bucket WHERE b.scope=?1 AND b.field='measurements'")?;
    for source in sources {
        let mut cursor = query.query([format!("projection:{}", source.source.id)])?;
        while let Some(row) = cursor.next()? {
            for (_, fact) in crate::live_index::measurement_block(db, row.get(0)?)? {
                out.insert(fact.id.clone(), crate::hash(serde_json::to_vec(&fact)?));
            }
        }
    }
    Ok(out)
}
pub(crate) fn summary(
    db: &Connection,
    sources: &[SourceReport],
) -> Result<crate::usage_app_dto::UsageSummary> {
    let mut total = Aggregate::new(&[])?;
    for source in sources {
        let mut after = (-1, -1);
        loop {
            let page = group_page(db, &source.source.id, None, None, after, None)?;
            if page.is_empty() {
                break;
            }
            for stored in page {
                after = (stored.block, stored.part);
                total.combine(&stored.decode(chrono_tz::UTC)?.aggregate)?;
            }
        }
    }
    Ok(total.usage)
}
pub(super) fn block_rows(
    db: &Connection,
    block: i64,
    prices: &crate::pricing_sync::Response,
) -> Result<Vec<(usize, PricedMeasurement)>> {
    let mut pool = super::price_pool::PricePool::new(prices);
    crate::live_index::measurement_block(db, block)?
        .into_iter()
        .map(|(slot, fact)| {
            let fact = Arc::new(fact);
            Ok((
                slot,
                PricedMeasurement {
                    price: pool.price(&fact),
                    fact,
                },
            ))
        })
        .collect()
}
pub(super) fn group_rows(
    rows: &[(usize, PricedMeasurement)],
    group: &LedgerGroup,
    turn: Option<&str>,
) -> Result<Vec<PricedMeasurement>> {
    let slots = group.selected_slots(turn);
    let out = rows
        .iter()
        .filter(|(slot, _)| slots.contains(slot))
        .map(|(slot, row)| {
            anyhow::ensure!(
                group.members.iter().any(|(id, slots)| slots.contains(slot)
                    && id.as_deref() == row.fact.turn_id.as_deref()),
                "aggregate member turn mismatch"
            );
            anyhow::ensure!(
                LedgerGroup::key(row) == group.key,
                "aggregate member attribution mismatch"
            );
            Ok(row.clone())
        })
        .collect::<Result<Vec<_>>>()?;
    anyhow::ensure!(out.len() == slots.len(), "aggregate member missing");
    Ok(out)
}

impl Snapshot {
    pub(crate) fn has_stored_ledger(&self) -> bool {
        self.cold_events.is_some() && self.is_live()
    }
    pub(crate) fn visit_ledger_groups(
        &self,
        tz: Tz,
        thread: Option<&str>,
        turn: Option<&str>,
        consume: impl FnMut(&LedgerGroup, &dyn Fn() -> Result<Vec<PricedMeasurement>>) -> Result<()>,
    ) -> Result<()> {
        self.visit_ledger_catalog(tz, thread, turn, None, consume)
    }
    pub(crate) fn visit_ledger_catalog(
        &self,
        tz: Tz,
        thread: Option<&str>,
        turn: Option<&str>,
        selection: Option<&dyn Fn(&LedgerGroup) -> bool>,
        mut consume: impl FnMut(&LedgerGroup, &dyn Fn() -> Result<Vec<PricedMeasurement>>) -> Result<()>,
    ) -> Result<()> {
        let ready: std::collections::HashSet<_> = self
            .manifest
            .threads
            .iter()
            .map(|t| t.thread.id.as_str())
            .collect();
        let unassigned_ready = self
            .project_loads
            .iter()
            .any(|p| p.project.is_none() && p.state == crate::live::ProjectLoadState::Ready);
        self.cold_events
            .as_ref()
            .ok_or_else(|| corrupt("cold ledger is unavailable"))?
            .groups(
                &self.manifest.sources,
                tz,
                thread,
                turn,
                selection,
                |group, read| {
                    if !self.project_loads.is_empty() {
                        let ready = group
                            .key
                            .thread
                            .as_deref()
                            .map_or(unassigned_ready, |id| ready.contains(id));
                        if !ready {
                            return Ok(());
                        }
                    }
                    consume(group, read)
                },
            )
    }
}
impl Snapshot {
    pub(crate) fn stored_measurements(
        &self,
        thread: Option<&str>,
        turn: Option<&str>,
    ) -> Result<Vec<PricedMeasurement>> {
        let mut out = vec![];
        self.visit_ledger_groups(chrono_tz::UTC, thread, turn, |group, read| {
            // Legacy detail/export consumers retain their complete result;
            // summary/ranking consumers never materialize these rows.
            out.extend(read()?);
            anyhow::ensure!(
                group.aggregate.usage.measurement_count > 0,
                "empty stored measurement group"
            );
            Ok(())
        })?;
        out.sort_unstable_by(|a, b| a.fact.id.cmp(&b.fact.id));
        super::query::validate_measurements(&out)?;
        Ok(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn group(days: i64) -> LedgerGroup {
        let mut aggregate = Aggregate::new(&[]).unwrap();
        aggregate.usage.measurement_count = 1;
        LedgerGroup {
            key: GroupKey {
                agent: "codex".into(),
                source: "test".into(),
                thread: Some("task".into()),
                turn: None,
                model: None,
                effort: None,
                time: GroupTime::Date(
                    (Utc::now().date_naive() - chrono::Duration::days(days)).to_string(),
                ),
            },
            aggregate,
            slots: vec![0],
            members: vec![(None, vec![0])],
            earliest: None,
            latest: None,
            latest_text: None,
        }
    }
    #[test]
    fn midnight_rollback_cannot_hide_an_interior_date() {
        let mut value = group(0);
        let times = [
            "2007-11-04T02:10:00Z",
            "2007-11-04T02:30:30Z",
            "2007-11-04T02:50:00Z",
        ]
        .map(|t| DateTime::parse_from_rfc3339(t).unwrap().timestamp());
        value.key.time = GroupTime::Hour(times[0].div_euclid(3600));
        value.earliest = Some(times[0]);
        value.latest = Some(times[2]);
        let tz = chrono_tz::America::St_Johns;
        assert_eq!(value.dates(tz).0, value.dates(tz).1);
        assert_ne!(
            Utc.timestamp_opt(times[1], 0)
                .single()
                .unwrap()
                .with_timezone(&tz)
                .date_naive(),
            value.dates(tz).0.unwrap()
        );
        assert!(value.single_date(tz).is_none());
        assert!(value.single_date(chrono_tz::UTC).is_some());
    }
    #[test]
    fn hot_admission_is_by_bytes_and_calendar_window_without_deleting_history() {
        let mut cache = HotCache::default();
        cache.insert("old".into(), Arc::new(group(30)), chrono_tz::UTC);
        cache.insert("future".into(), Arc::new(group(-1)), chrono_tz::UTC);
        let mut unknown = group(0);
        unknown.key.time = GroupTime::Unknown;
        cache.insert("unknown".into(), Arc::new(unknown), chrono_tz::UTC);
        assert_eq!(cache.bytes, 0);
        cache.insert("edge".into(), Arc::new(group(29)), chrono_tz::UTC);
        assert!(cache.get("edge").is_some());
        for n in 0..2000 {
            let mut value = group(0);
            value.key.thread = Some(format!("{n}:{}", "x".repeat(10000)));
            cache.insert(n.to_string(), Arc::new(value), chrono_tz::UTC);
            assert!(cache.bytes <= HOT_BYTES);
            assert!(cache.entries.len() <= 4096);
        }
        assert!(cache.get("edge").is_none());
        assert!(cache.get("1999").is_some());
        let before = cache.bytes;
        let mut oversized = group(0);
        oversized.key.thread = Some("x".repeat(HOT_BYTES));
        cache.insert("oversized".into(), Arc::new(oversized), chrono_tz::UTC);
        assert_eq!(cache.bytes, before);
    }
    fn stored(value: &LedgerGroup) -> StoredGroup {
        let (data, bytes, digest) = crate::live_index::encode_projection(value).unwrap();
        StoredGroup {
            catalog: Catalog::from_group(value),
            excluded: false,
            stub: None,
            block: 1,
            part: 0,
            version: VERSION,
            bytes: bytes as i64,
            digest,
            data,
            source: value.key.source.clone(),
            thread: value.key.thread.clone(),
            targets: value.targets().unwrap(),
            count: 1,
        }
    }
    #[test]
    fn cache_hits_still_validate_header_identity_and_corrupted_payload() {
        let value = group(0);
        stored(&value).decode(chrono_tz::UTC).unwrap();
        let mut invalid = stored(&value);
        invalid.thread = Some("other".into());
        assert!(invalid.decode(chrono_tz::UTC).is_err());
        let mut invalid = stored(&value);
        invalid.version = 99;
        assert_eq!(
            invalid
                .decode(chrono_tz::UTC)
                .err()
                .unwrap()
                .downcast_ref::<crate::dto::OperationError>()
                .unwrap()
                .code,
            "INDEX_UNSUPPORTED_VERSION"
        );
        let mut invalid = stored(&value);
        invalid.data[0] ^= 0xff;
        assert!(invalid.decode(chrono_tz::UTC).is_err());
        let mut invalid = value.clone();
        invalid.slots = vec![0, 0];
        invalid.aggregate.usage.measurement_count = 2;
        assert!(stored(&invalid).decode(chrono_tz::UTC).is_err());
    }
}
