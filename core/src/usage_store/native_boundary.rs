//! Complete native boundary scalars remain readable without materializing target facts.
//! This is an index of observed facts, not a claim that source logs are complete.
use super::*;
use crate::session_events::Event;
use crate::timing::analysis::{BOUNDARY_METHOD_VERSION, BoundaryReducer, BoundarySummary};
use std::sync::atomic::{AtomicBool, Ordering};

const MAX_SUMMARY_BYTES: usize = 64 * 1024;
const MAX_OWNER_WORK: usize = 100_000;

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct NativeBoundaryIndex {
    pub method_version: String,
    pub event_count: usize,
    /// Exact safe message count, including partitions without a resolved turn.
    pub message_count: usize,
    /// The complete partition was examined; source coverage remains separate.
    pub complete: bool,
    /// None means a mixed source bucket; never infer one source from the first row.
    pub source_instance_id: Option<String>,
    pub summary: BoundarySummary,
}

struct Size(usize);
impl std::io::Write for Size {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0 = self
            .0
            .checked_add(bytes.len())
            .ok_or_else(|| std::io::Error::other("native summary size overflow"))?;
        if self.0 > MAX_SUMMARY_BYTES {
            return Err(std::io::Error::other("native summary size limit"));
        }
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}
fn bounded(index: &NativeBoundaryIndex) -> Result<()> {
    serde_json::to_writer(&mut Size(0), index)
        .map_err(|_| operation_error("RESOURCE_LIMIT", "原生轮次边界摘要超过64 KiB"))
}

/// Construction is O(E log min(D,100000)) over the already resident complete
/// partition, with O(min(D,100000)) auxiliary clock-domain storage, independently
/// of the target query's 100,000-fact budget. No event IDs or body fields are copied.
pub(super) fn build(target: &EventTarget, events: &[Arc<Event>]) -> Result<NativeBoundaryIndex> {
    let first_source = events
        .first()
        .map(|event| event.position().source_instance_id.as_ref());
    let same_source = events
        .iter()
        .all(|event| Some(event.position().source_instance_id.as_ref()) == first_source);
    let mut reducer = BoundaryReducer::default();
    if target.turn_id.is_some() && same_source {
        for event in events {
            reducer.observe(event);
        }
    }
    let index = NativeBoundaryIndex {
        method_version: BOUNDARY_METHOD_VERSION.into(),
        event_count: events.len(),
        message_count: events
            .iter()
            .filter(|e| matches!(e.payload(), crate::session_events::Payload::Message { .. }))
            .count(),
        complete: true,
        source_instance_id: same_source
            .then(|| first_source.map(str::to_owned))
            .flatten(),
        summary: reducer.finish(),
    };
    bounded(&index)?;
    Ok(index)
}

/// Current unpublished shape is required; unknown mappings never read historical
/// source logs to fill absent fields or upgrade a retained snapshot in place.
pub(super) fn check_versions(events: &serde_json::Value) -> Result<()> {
    let partitions = events
        .get("partitions")
        .and_then(serde_json::Value::as_array)
        .ok_or_else(|| corrupt("事件目标索引缺失"))?;
    for partition in partitions {
        let native = &partition["nativeBoundary"];
        if native["methodVersion"].as_str() != Some(BOUNDARY_METHOD_VERSION)
            || native["summary"]["formatVersion"].as_u64() != Some(1)
        {
            return Err(operation_error(
                "UNSUPPORTED_VERSION",
                "不支持此原生轮次边界索引",
            ));
        }
    }
    Ok(())
}
pub(super) fn validate(partition: &EventPartition) -> Result<()> {
    let index = &partition.native_boundary;
    if index.method_version != BOUNDARY_METHOD_VERSION || index.summary.format_version != 1 {
        return Err(operation_error(
            "UNSUPPORTED_VERSION",
            "不支持此原生轮次边界索引",
        ));
    }
    if !index.complete
        || index.message_count > index.event_count
        || !partition.chunks.is_empty()
            && index.message_count
                != partition
                    .chunks
                    .iter()
                    .map(|c| c.message_count)
                    .sum::<usize>()
        || index.event_count != partition.count
        || index.summary.candidates > partition.count
        || index
            .source_instance_id
            .as_ref()
            .is_some_and(String::is_empty)
        || partition.target.turn_id.is_none()
            && index.summary != BoundaryReducer::default().finish()
    {
        return Err(corrupt("原生轮次边界索引不完整或身份无效"));
    }
    if let Some(source) = &index.source_instance_id
        && partition.chunks.iter().any(|chunk| {
            chunk.first.source_instance_id.as_ref() != source.as_str()
                || chunk.last.source_instance_id.as_ref() != source.as_str()
        })
    {
        return Err(corrupt("原生轮次边界来源身份无效"));
    }
    bounded(index).map_err(|_| corrupt("原生轮次边界摘要超限"))
}

impl Snapshot {
    /// Lookup reads only the validated manifest summary, never ledger/event blocks.
    /// Construction/load validates its digest with the partition; an immutable view
    /// remains the authority. Owner lookup has an independent metadata work limit.
    pub fn timing_native_boundary(
        &self,
        target: timing_evidence::TurnTarget<'_>,
        cancelled: &AtomicBool,
    ) -> Result<Option<&BoundarySummary>> {
        if cancelled.load(Ordering::Relaxed) {
            return Err(operation_error("CANCELLED", "读取已取消"));
        }
        if [target.source, target.thread, target.turn]
            .iter()
            .any(|id| id.is_empty())
        {
            return Err(operation_error("INVALID_ARGUMENT", "轮次边界目标不能为空"));
        }
        let mut owner = None;
        for (work, entry) in self.manifest.threads.iter().enumerate() {
            if cancelled.load(Ordering::Relaxed) {
                return Err(operation_error("CANCELLED", "读取已取消"));
            }
            if work >= MAX_OWNER_WORK {
                return Err(operation_error(
                    "RESOURCE_LIMIT",
                    "轮次边界目标元数据读取超限",
                ));
            }
            if entry.thread.id == target.thread {
                owner = Some(entry);
                break;
            }
        }
        let owner = owner.ok_or_else(|| operation_error("NOT_FOUND", "未找到对话"))?;
        if owner.thread.source_instance_id != target.source {
            return Err(operation_error("INVALID_ARGUMENT", "轮次来源身份不匹配"));
        }
        if !owner.turns.contains_key(target.turn) {
            return Err(operation_error("NOT_FOUND", "未找到轮次"));
        }
        let Some(partition) =
            self.event_partition(&EventTarget::turn(target.thread, target.turn))?
        else {
            return Ok(None);
        };
        let index = &partition.native_boundary;
        if index.method_version != BOUNDARY_METHOD_VERSION || index.summary.format_version != 1 {
            return Err(operation_error(
                "UNSUPPORTED_VERSION",
                "不支持此原生轮次边界索引",
            ));
        }
        if !index.complete
            || index.event_count != partition.count
            || index.summary.candidates > partition.count
        {
            return Err(corrupt("原生轮次边界索引不完整"));
        }
        if index.source_instance_id.as_deref() != Some(target.source) {
            return Err(corrupt("轮次边界来源身份不匹配"));
        }
        Ok(Some(&index.summary))
    }
}

#[cfg(test)]
mod tests;
