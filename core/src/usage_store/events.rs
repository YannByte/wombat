//! Canonical events have exact-scope partitions; page reads never open other targets.
//! Construction still groups all E safe event Arcs and sorts in O(E log E); loading
//! retains the complete manifest index. These are not full-source residency limits.
//! A disk page locates its partition and first block in O(log P + log C), then reads
//! only intersecting blocks, with at most 200 facts/4 MiB per block and a 64 MiB
//! query byte budget. Whole-target reads reject >100,000 facts.
use super::*;
use crate::session_events::{Event, Position};
use sha2::{Digest, Sha256};
use std::{
    cmp::Ordering,
    collections::{BTreeSet, HashSet},
    sync::atomic::{AtomicBool, Ordering as AtomicOrdering},
};

const EVENT_INDEX_VERSION: u32 = 1;
pub const MAX_TARGET_EVENTS: usize = 100_000;
pub const MAX_EVENT_PAGE_ROWS: usize = 200;
const MAX_EVENT_CHUNK_BYTES: u64 = 4 * 1024 * 1024;
const MAX_EVENT_QUERY_BYTES: u64 = 64 * 1024 * 1024;

/// Exact attribution bucket. None/None retains source facts without a thread;
/// thread/None retains facts without a turn, separate from any native turn ID.
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EventTarget {
    pub thread_id: Option<String>,
    pub turn_id: Option<String>,
}
impl EventTarget {
    pub fn turn(thread_id: impl Into<String>, turn_id: impl Into<String>) -> Self {
        Self {
            thread_id: Some(thread_id.into()),
            turn_id: Some(turn_id.into()),
        }
    }
    fn of(event: &Event) -> Self {
        Self {
            thread_id: event.thread_id().map(str::to_owned),
            turn_id: event.turn_id().map(str::to_owned),
        }
    }
    fn valid(&self) -> bool {
        (self.turn_id.is_none() || self.thread_id.is_some())
            && [&self.thread_id, &self.turn_id]
                .into_iter()
                .all(|id| id.as_ref().is_none_or(|id| !id.is_empty()))
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EventChunk {
    pub offset: usize,
    pub file: FileRef,
    pub count: usize,
    pub message_count: usize,
    pub bytes: u64,
    pub first: Position,
    pub last: Position,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EventPartition {
    pub sha256: String,
    pub target: EventTarget,
    pub count: usize,
    pub chunks: Vec<EventChunk>,
    /// Required complete reduction of this immutable partition's native boundaries.
    pub native_boundary: super::native_boundary::NativeBoundaryIndex,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EventIndex {
    pub version: u32,
    pub partitions: Vec<Arc<EventPartition>>,
}
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct EventBlock {
    version: u32,
    target: EventTarget,
    events: Vec<Arc<Event>>,
}
/// Facts bound the result; bytes bound touched block encodings and the result array.
/// Callers may narrow these hard limits, but cannot widen the defaults.
#[derive(Clone, Copy, Debug)]
pub struct EventReadBudget {
    pub max_facts: usize,
    pub max_bytes: u64,
}
impl Default for EventReadBudget {
    fn default() -> Self {
        Self {
            max_facts: MAX_TARGET_EVENTS,
            max_bytes: MAX_EVENT_QUERY_BYTES,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EventCursor {
    snapshot_id: String,
    target: EventTarget,
    partition_hash: String,
    next_offset: usize,
    sha256: String,
}
#[derive(Debug)]
pub struct EventPage {
    pub events: Vec<Arc<Event>>,
    pub total: usize,
    pub next_cursor: Option<EventCursor>,
}
pub(super) type EventBuckets = BTreeMap<EventTarget, Arc<Vec<Arc<Event>>>>;

pub(super) fn position_order(a: &Position, b: &Position) -> Ordering {
    (
        &a.source_instance_id,
        &a.file_id,
        &a.generation,
        a.byte_offset,
        a.ordinal,
    )
        .cmp(&(
            &b.source_instance_id,
            &b.file_id,
            &b.generation,
            b.byte_offset,
            b.ordinal,
        ))
}
pub(super) fn validate(events: &[Arc<Event>]) -> Result<()> {
    let mut ids = HashSet::with_capacity(events.len());
    for event in events {
        if !ids.insert(event.id()) {
            return Err(operation_error("INVALID_FACTS", "事件身份重复"));
        }
    }
    Ok(())
}
pub(super) fn group(events: Vec<Arc<Event>>) -> Result<EventBuckets> {
    validate(&events)?;
    let mut buckets: BTreeMap<EventTarget, Vec<Arc<Event>>> = BTreeMap::new();
    for event in events {
        buckets
            .entry(EventTarget::of(&event))
            .or_default()
            .push(event);
    }
    for events in buckets.values_mut() {
        if !events.is_sorted_by(|a, b| position_order(a.position(), b.position()).is_le()) {
            events.sort_unstable_by(|a, b| position_order(a.position(), b.position()));
        }
    }
    Ok(buckets
        .into_iter()
        .map(|(target, events)| (target, Arc::new(events)))
        .collect())
}
fn rename_partition(partition: &mut EventPartition, number: usize) -> Result<()> {
    let mut renamed = false;
    for (chunk_number, chunk) in partition.chunks.iter_mut().enumerate() {
        let name = format!("events-{number}-{chunk_number}.json");
        renamed |= chunk.file.file != name;
        chunk.file.file = name;
    }
    if renamed {
        partition.sha256 = partition_hash(partition)?;
    }
    Ok(())
}
pub(super) fn reuse_memory_events(
    events: Vec<Arc<Event>>,
    prior: Option<&Snapshot>,
) -> Result<(EventIndex, EventBuckets)> {
    build_events(events, None, prior)
}
pub(super) fn save_events(directory: &Path, events: Vec<Arc<Event>>) -> Result<EventIndex> {
    Ok(build_events(events, Some(directory), None)?.0)
}
fn encode_bounded<T: Serialize>(value: &T, cancelled: &AtomicBool) -> Result<Vec<u8>> {
    let mut writer = BudgetWriter {
        bytes: 0,
        max_bytes: MAX_EVENT_CHUNK_BYTES,
        cancelled,
        output: Some(Vec::new()),
        hash: None,
    };
    // Buffer serde's small writes, retaining the same hard output limit.
    {
        use std::io::Write;
        let mut buffered = std::io::BufWriter::with_capacity(8192, &mut writer);
        serde_json::to_writer(&mut buffered, value)
            .map_err(|_| operation_error("RESOURCE_LIMIT", "单条事件及其块元数据超过4 MiB"))?;
        buffered
            .flush()
            .map_err(|_| operation_error("RESOURCE_LIMIT", "单条事件及其块元数据超过4 MiB"))?;
    }
    Ok(writer.output.expect("bounded encoding retains its bytes"))
}
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct EncodedBlock<'a> {
    version: u32,
    target: &'a EventTarget,
    events: &'a [Box<serde_json::value::RawValue>],
}

fn build_events(
    events: Vec<Arc<Event>>,
    directory: Option<&Path>,
    prior: Option<&Snapshot>,
) -> Result<(EventIndex, EventBuckets)> {
    let mut buckets = group(events)?;
    let mut partitions = Vec::new();
    let cancelled = AtomicBool::new(false);
    for (number, (target, events)) in buckets.iter_mut().enumerate() {
        if let Some(prior) = prior
            && let Some(previous) = prior.live_events.as_ref().and_then(|b| b.get(target))
            && previous.len() == events.len()
            && previous
                .iter()
                .zip(events.iter())
                .all(|(a, b)| Arc::ptr_eq(a, b))
            && let Some(partition) = prior.event_partition(target)?
        {
            let mut partition = partition.clone();
            rename_partition(&mut partition, number)?;
            *events = Arc::clone(previous);
            partitions.push(Arc::new(partition));
            continue;
        }
        let overhead = encode_bounded(
            &EventBlock {
                version: EVENT_INDEX_VERSION,
                target: target.clone(),
                events: vec![],
            },
            &cancelled,
        )?
        .len() as u64;
        let mut chunks = Vec::new();
        let mut start = 0usize;
        let mut pending = None;
        while start < events.len() {
            let mut end = start;
            let mut size = overhead;
            let mut encoded = Vec::new();
            while end < events.len() && end - start < MAX_EVENT_PAGE_ROWS {
                let raw = match pending.take() {
                    Some(raw) => raw,
                    None => serde_json::value::RawValue::from_string(String::from_utf8(
                        encode_bounded(&events[end], &cancelled)?,
                    )?)?,
                };
                let event_size = raw.get().len() as u64;
                if overhead
                    .checked_add(event_size)
                    .is_none_or(|n| n > MAX_EVENT_CHUNK_BYTES)
                {
                    return Err(operation_error(
                        "RESOURCE_LIMIT",
                        "单条事件及其块元数据超过4 MiB",
                    ));
                }
                let next_size = size + event_size + u64::from(end != start);
                if next_size > MAX_EVENT_CHUNK_BYTES {
                    pending = Some(raw);
                    break;
                }
                size = next_size;
                encoded.push(raw);
                end += 1;
            }
            let bytes = encode_bounded(
                &EncodedBlock {
                    version: EVENT_INDEX_VERSION,
                    target,
                    events: &encoded,
                },
                &cancelled,
            )?;
            let name = format!("events-{number}-{}.json", chunks.len());
            let sha256 = crate::hash(&bytes);
            if let Some(directory) = directory {
                crate::storage::write_unpublished(&directory.join(&name), &bytes)?;
            }
            chunks.push(EventChunk {
                offset: start,
                file: FileRef { file: name, sha256 },
                count: end - start,
                message_count: events[start..end]
                    .iter()
                    .filter(|e| {
                        matches!(e.payload(), crate::session_events::Payload::Message { .. })
                    })
                    .count(),
                bytes: bytes.len() as u64,
                first: events[start].position().clone(),
                last: events[end - 1].position().clone(),
            });
            start = end;
        }
        let mut partition = EventPartition {
            sha256: String::new(),
            target: target.clone(),
            count: events.len(),
            chunks,
            native_boundary: super::native_boundary::build(target, events)?,
        };
        partition.sha256 = partition_hash(&partition)?;
        partitions.push(Arc::new(partition));
    }
    let index = EventIndex {
        version: EVENT_INDEX_VERSION,
        partitions,
    };
    validate_index(&index)?;
    Ok((index, buckets))
}
struct HashWriter(Sha256);
impl std::io::Write for HashWriter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.update(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
pub(super) fn partition_hash(partition: &EventPartition) -> Result<String> {
    let mut writer = HashWriter(Sha256::new());
    serde_json::to_writer(
        &mut writer,
        &(
            &partition.target,
            partition.count,
            &partition.chunks,
            &partition.native_boundary,
        ),
    )?;
    Ok(format!("{:x}", writer.0.finalize()))
}
pub(super) fn check_index_version(version: Option<u64>) -> Result<()> {
    match version {
        Some(version) if version == u64::from(EVENT_INDEX_VERSION) => Ok(()),
        Some(_) => Err(operation_error(
            "UNSUPPORTED_VERSION",
            "不支持此事件索引版本",
        )),
        None => Err(corrupt("事件索引版本头无效")),
    }
}

#[derive(Deserialize)]
struct VersionHeader {
    version: u64,
}
#[derive(Deserialize)]
struct BlockHeader<'a> {
    version: u64,
    #[serde(borrow)]
    events: Option<&'a serde_json::value::RawValue>,
}
struct EventVersions(bool);
impl<'de> Deserialize<'de> for EventVersions {
    fn deserialize<D: serde::Deserializer<'de>>(
        deserializer: D,
    ) -> std::result::Result<Self, D::Error> {
        struct VersionsVisitor;
        impl<'de> serde::de::Visitor<'de> for VersionsVisitor {
            type Value = EventVersions;
            fn expecting(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                formatter.write_str("an event array containing at most 200 facts")
            }
            fn visit_seq<A: serde::de::SeqAccess<'de>>(
                self,
                mut sequence: A,
            ) -> std::result::Result<EventVersions, A::Error> {
                let mut count = 0;
                let mut unknown = false;
                while let Some(event) =
                    sequence.next_element::<&'de serde_json::value::RawValue>()?
                {
                    count += 1;
                    if count > MAX_EVENT_PAGE_ROWS {
                        return Err(serde::de::Error::custom("事件块数量超限"));
                    }
                    // Borrow each encoded fact; no bodies or full JSON tree are copied.
                    let header: VersionHeader =
                        serde_json::from_str(event.get()).map_err(serde::de::Error::custom)?;
                    unknown |= header.version != u64::from(crate::session_events::EVENT_VERSION);
                }
                Ok(EventVersions(unknown))
            }
        }
        deserializer.deserialize_seq(VersionsVisitor)
    }
}
fn check_block_versions(bytes: &[u8]) -> Result<()> {
    let header: BlockHeader<'_> =
        serde_json::from_slice(bytes).map_err(|e| corrupt(format!("事件块版本头无效：{e}")))?;
    if header.version != u64::from(EVENT_INDEX_VERSION) {
        return Err(operation_error("UNSUPPORTED_VERSION", "不支持此事件块版本"));
    }
    let events = header.events.ok_or_else(|| corrupt("事件块缺少事件数组"))?;
    let versions: EventVersions =
        serde_json::from_str(events.get()).map_err(|e| corrupt(format!("事件版本头无效：{e}")))?;
    if versions.0 {
        return Err(operation_error("UNSUPPORTED_VERSION", "不支持此事件版本"));
    }
    Ok(())
}
pub(super) fn validate_index(index: &EventIndex) -> Result<()> {
    check_index_version(Some(u64::from(index.version)))?;
    let mut files = BTreeSet::new();
    for (number, partition) in index.partitions.iter().enumerate() {
        super::native_boundary::validate(partition)?;
        if partition.sha256 != partition_hash(partition)?
            || !partition.target.valid()
            || partition.count == 0
            || number > 0 && index.partitions[number - 1].target >= partition.target
        {
            return Err(corrupt("事件目标索引无效"));
        }
        let mut count = 0usize;
        for (number, chunk) in partition.chunks.iter().enumerate() {
            if chunk.offset != count
                || chunk.count == 0
                || chunk.count > MAX_EVENT_PAGE_ROWS
                || chunk.bytes == 0
                || chunk.bytes > MAX_EVENT_CHUNK_BYTES
                || !files.insert(&chunk.file.file)
                || chunk.first.event_id().is_err()
                || chunk.last.event_id().is_err()
                || position_order(&chunk.first, &chunk.last)
                    != if chunk.count == 1 {
                        Ordering::Equal
                    } else {
                        Ordering::Less
                    }
                || number > 0
                    && position_order(&partition.chunks[number - 1].last, &chunk.first)
                        != Ordering::Less
            {
                return Err(corrupt("事件块索引无效"));
            }
            count = count
                .checked_add(chunk.count)
                .ok_or_else(|| corrupt("事件数量溢出"))?;
        }
        if count != partition.count {
            return Err(corrupt("事件索引数量不匹配"));
        }
    }
    Ok(())
}
fn check_cancel(cancelled: &AtomicBool) -> Result<()> {
    if cancelled.load(AtomicOrdering::Relaxed) {
        return Err(operation_error("CANCELLED", "事件查询已取消"));
    }
    Ok(())
}
fn resource_limit() -> anyhow::Error {
    operation_error("RESOURCE_LIMIT", "事件查询超过资源预算")
}
struct BudgetWriter<'a> {
    bytes: u64,
    max_bytes: u64,
    cancelled: &'a AtomicBool,
    output: Option<Vec<u8>>,
    hash: Option<Sha256>,
}
impl std::io::Write for BudgetWriter<'_> {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        if self.cancelled.load(AtomicOrdering::Relaxed) {
            return Err(std::io::Error::other("cancelled"));
        }
        if (bytes.len() as u64) > self.max_bytes.saturating_sub(self.bytes) {
            return Err(std::io::Error::other("event query byte budget exceeded"));
        }
        self.bytes += bytes.len() as u64;
        if let Some(output) = &mut self.output {
            output.extend_from_slice(bytes);
        }
        if let Some(hash) = &mut self.hash {
            hash.update(bytes);
        }
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
fn encoded_size(events: &[Arc<Event>], budget: u64, cancelled: &AtomicBool) -> Result<u64> {
    let mut writer = BudgetWriter {
        bytes: 0,
        max_bytes: budget,
        cancelled,
        output: None,
        hash: None,
    };
    if serde_json::to_writer(&mut writer, events).is_err() {
        check_cancel(cancelled)?;
        return Err(resource_limit());
    }
    check_cancel(cancelled)?;
    Ok(writer.bytes)
}
impl EventCursor {
    fn digest(&self) -> Result<String> {
        Ok(crate::hash(serde_json::to_vec(&(
            &self.snapshot_id,
            &self.target,
            &self.partition_hash,
            self.next_offset,
        ))?))
    }
}
impl Snapshot {
    pub(crate) fn event_partition(&self, target: &EventTarget) -> Result<Option<&EventPartition>> {
        if !target.valid() {
            return Err(operation_error("INVALID_ARGUMENT", "非法事件目标"));
        }
        if self.manifest.events.version != EVENT_INDEX_VERSION {
            return Err(operation_error(
                "UNSUPPORTED_VERSION",
                "不支持此事件索引版本",
            ));
        }
        Ok(self
            .manifest
            .events
            .partitions
            .binary_search_by(|p| p.target.cmp(target))
            .ok()
            .map(|i| self.manifest.events.partitions[i].as_ref()))
    }
    /// Returns the entire exact attribution bucket or an error, never a truncated success.
    /// Metadata checks reject oversized targets before opening an event block.
    pub fn events_for_target(
        &self,
        target: &EventTarget,
        budget: EventReadBudget,
        cancelled: &AtomicBool,
    ) -> Result<Vec<Arc<Event>>> {
        check_cancel(cancelled)?;
        let Some(partition) = self.event_partition(target)? else {
            encoded_size(&[], budget.max_bytes.min(MAX_EVENT_QUERY_BYTES), cancelled)?;
            return Ok(vec![]);
        };
        if partition.count > budget.max_facts.min(MAX_TARGET_EVENTS) {
            return Err(resource_limit());
        }
        self.event_range(partition, 0, partition.count, budget, cancelled)
    }
    /// Creates a local locator into this exact immutable partition without reading
    /// event blocks. Offset zero uses the normal first-page request. Callers obtain
    /// offsets from the already validated partition order, never analysis ordering.
    pub(crate) fn event_cursor_at_offset(
        &self,
        target: &EventTarget,
        offset: usize,
        cancelled: &AtomicBool,
    ) -> Result<Option<EventCursor>> {
        check_cancel(cancelled)?;
        let partition = self.event_partition(target)?;
        let total = partition.map_or(0, |partition| partition.count);
        if offset >= total && offset != 0 {
            return Err(operation_error("INVALID_ARGUMENT", "事件定位超出目标范围"));
        }
        if offset == 0 {
            return Ok(None);
        }
        let mut cursor = EventCursor {
            snapshot_id: self.manifest.snapshot_ref.snapshot_id.clone(),
            target: target.clone(),
            partition_hash: partition.unwrap().sha256.clone(),
            next_offset: offset,
            sha256: String::new(),
        };
        cursor.sha256 = cursor.digest()?;
        check_cancel(cancelled)?;
        Ok(Some(cursor))
    }
    /// Evidence pagination bounds the selected page independently of whole-turn computation.
    pub fn event_page(
        &self,
        target: &EventTarget,
        limit: usize,
        cursor: Option<&EventCursor>,
        budget: EventReadBudget,
        cancelled: &AtomicBool,
    ) -> Result<EventPage> {
        check_cancel(cancelled)?;
        if limit == 0 || limit > MAX_EVENT_PAGE_ROWS {
            return Err(operation_error(
                "INVALID_ARGUMENT",
                "事件页大小必须为1到200",
            ));
        }
        let partition = self.event_partition(target)?;
        let partition_hash = partition.map_or_else(String::new, |p| p.sha256.clone());
        let total = partition.map_or(0, |p| p.count);
        let offset = if let Some(cursor) = cursor {
            if cursor.snapshot_id != self.manifest.snapshot_ref.snapshot_id
                || &cursor.target != target
                || cursor.partition_hash != partition_hash
                || cursor.sha256 != cursor.digest()?
                || cursor.next_offset == 0
                || cursor.next_offset >= total
            {
                return Err(operation_error(
                    "INVALID_ARGUMENT",
                    "事件游标与读取版本或目标不匹配",
                ));
            }
            cursor.next_offset
        } else {
            0
        };
        let length = limit.min(total - offset);
        let events = match partition {
            Some(partition) => self.event_range(partition, offset, length, budget, cancelled)?,
            None => {
                encoded_size(&[], budget.max_bytes.min(MAX_EVENT_QUERY_BYTES), cancelled)?;
                vec![]
            }
        };
        let next_cursor = if offset + length < total {
            let mut cursor = EventCursor {
                snapshot_id: self.manifest.snapshot_ref.snapshot_id.clone(),
                target: target.clone(),
                partition_hash,
                next_offset: offset + length,
                sha256: String::new(),
            };
            cursor.sha256 = cursor.digest()?;
            Some(cursor)
        } else {
            None
        };
        check_cancel(cancelled)?;
        Ok(EventPage {
            events,
            total,
            next_cursor,
        })
    }
    pub(super) fn event_range(
        &self,
        partition: &EventPartition,
        offset: usize,
        length: usize,
        budget: EventReadBudget,
        cancelled: &AtomicBool,
    ) -> Result<Vec<Arc<Event>>> {
        let max_bytes = budget.max_bytes.min(MAX_EVENT_QUERY_BYTES);
        if length > budget.max_facts.min(MAX_TARGET_EVENTS) {
            return Err(resource_limit());
        }
        if let Some(buckets) = &self.live_events {
            let events = buckets
                .get(&partition.target)
                .ok_or_else(|| corrupt("事件目标缺失"))?;
            let range = events
                .get(offset..offset + length)
                .ok_or_else(|| corrupt("事件范围越界"))?;
            let mut read_bytes = 0u64;
            let first = partition
                .chunks
                .partition_point(|chunk| chunk.offset.saturating_add(chunk.count) <= offset);
            for chunk in &partition.chunks[first..] {
                check_cancel(cancelled)?;
                if chunk.offset >= offset + length {
                    break;
                }
                read_bytes = read_bytes
                    .checked_add(chunk.bytes)
                    .ok_or_else(resource_limit)?;
                if read_bytes > max_bytes {
                    return Err(resource_limit());
                }
            }
            encoded_size(range, max_bytes, cancelled)?;
            return Ok(range.to_vec());
        }
        let mut result = Vec::with_capacity(length);
        let mut read_bytes = 0u64;
        let first_chunk = partition
            .chunks
            .partition_point(|chunk| chunk.offset.saturating_add(chunk.count) <= offset);
        for chunk in &partition.chunks[first_chunk..] {
            let start = chunk.offset;
            check_cancel(cancelled)?;
            let end = start
                .checked_add(chunk.count)
                .ok_or_else(|| corrupt("事件数量溢出"))?;
            if start < offset + length && end > offset {
                let block = if let Some(cold) = &self.cold_events {
                    read_bytes = read_bytes
                        .checked_add(chunk.bytes)
                        .ok_or_else(resource_limit)?;
                    if read_bytes > max_bytes {
                        return Err(resource_limit());
                    }
                    EventBlock {
                        version: EVENT_INDEX_VERSION,
                        target: partition.target.clone(),
                        events: cold.read(chunk)?,
                    }
                } else {
                    // Check the actual filesystem length before allocation, not just manifest claims.
                    let path = safe_file(&self.directory, &chunk.file.file)?;
                    let file = fs::File::open(path)
                        .map_err(|e| corrupt(format!("事件块无法读取：{e}")))?;
                    let size = file.metadata()?.len();
                    if size != chunk.bytes {
                        return Err(corrupt("事件分片校验失败：长度不匹配"));
                    }
                    read_bytes = read_bytes.checked_add(size).ok_or_else(resource_limit)?;
                    if size > MAX_EVENT_CHUNK_BYTES || read_bytes > max_bytes {
                        return Err(resource_limit());
                    }
                    let mut bytes = Vec::with_capacity(size as usize);
                    file.take(size + 1).read_to_end(&mut bytes)?;
                    if bytes.len() as u64 != size || crate::hash(&bytes) != chunk.file.sha256 {
                        return Err(corrupt("事件分片校验失败"));
                    }
                    check_cancel(cancelled)?;
                    check_block_versions(&bytes)?;
                    let block: EventBlock = serde_json::from_slice(&bytes).map_err(|e| {
                        if e.to_string().contains("unsupported event version") {
                            operation_error("UNSUPPORTED_VERSION", "不支持此事件版本")
                        } else {
                            corrupt(format!("事件块无效：{e}"))
                        }
                    })?;
                    block
                };
                if block.version != EVENT_INDEX_VERSION {
                    return Err(operation_error("UNSUPPORTED_VERSION", "不支持此事件块版本"));
                }
                if block.target != partition.target
                    || block.events.len() != chunk.count
                    || block.events.first().map(|e| e.position()) != Some(&chunk.first)
                    || block.events.last().map(|e| e.position()) != Some(&chunk.last)
                    || block
                        .events
                        .iter()
                        .any(|e| EventTarget::of(e) != partition.target)
                    || block.events.windows(2).any(|pair| {
                        position_order(pair[0].position(), pair[1].position()) != Ordering::Less
                    })
                {
                    return Err(corrupt("事件块目标、数量或顺序不匹配"));
                }
                let from = offset.saturating_sub(start);
                let to = (offset + length).min(end) - start;
                result.extend(block.events[from..to].iter().cloned());
            }
            if end >= offset + length {
                break;
            }
        }
        if result.len() != length {
            return Err(corrupt("事件范围不完整"));
        }
        encoded_size(&result, max_bytes, cancelled)?;
        Ok(result)
    }
    /// Internal snapshot export only. This intentionally traverses all attribution buckets,
    /// including unattributed facts; target queries must use the bounded methods above.
    pub(crate) fn events(&self) -> Result<Vec<Arc<Event>>> {
        if let Some(buckets) = &self.live_events {
            return Ok(buckets
                .values()
                .flat_map(|events| events.iter())
                .cloned()
                .collect());
        }
        let mut events = Vec::new();
        let cancelled = AtomicBool::new(false);
        for partition in &self.manifest.events.partitions {
            for offset in (0..partition.count).step_by(MAX_EVENT_PAGE_ROWS) {
                events.extend(self.event_range(
                    partition,
                    offset,
                    MAX_EVENT_PAGE_ROWS.min(partition.count - offset),
                    EventReadBudget::default(),
                    &cancelled,
                )?);
            }
        }
        validate(&events).map_err(|_| corrupt("事件身份重复"))?;
        Ok(events)
    }
}

#[cfg(test)]
mod encoding_tests {
    use super::*;

    #[test]
    fn preencoded_chunks_keep_exact_wire_bytes_and_digest() {
        let events = (0..201)
            .map(|offset| {
                Arc::new(
                    Event::new_at(
                        Position {
                            source_instance_id: "源".into(),
                            file_id: "file".into(),
                            generation: "g".into(),
                            byte_offset: offset,
                            ordinal: 0,
                        },
                        Some("会话\"\\".into()),
                        None,
                        crate::session_events::Time::from_source(None).0,
                        vec![],
                        crate::session_events::Payload::ContextWindow {
                            model: Some("模型😀\n".into()),
                            tokens: u64::MAX,
                        },
                        "2026-10-11T00:00:00Z".into(),
                    )
                    .unwrap(),
                )
            })
            .collect::<Vec<_>>();
        let directory = tempfile::tempdir().unwrap();
        let index = save_events(directory.path(), events.clone()).unwrap();
        let partition = &index.partitions[0];
        assert_eq!(partition.chunks.len(), 2);
        for chunk in &partition.chunks {
            let expected = serde_json::to_vec(&EventBlock {
                version: EVENT_INDEX_VERSION,
                target: partition.target.clone(),
                events: events[chunk.offset..chunk.offset + chunk.count].to_vec(),
            })
            .unwrap();
            assert_eq!(chunk.bytes, expected.len() as u64);
            assert_eq!(chunk.file.sha256, crate::hash(&expected));
            assert_eq!(
                fs::read(directory.path().join(&chunk.file.file)).unwrap(),
                expected
            );
        }
    }
}
