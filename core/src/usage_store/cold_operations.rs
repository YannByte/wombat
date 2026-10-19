//! Restore operation locators and use counts; payloads stay in verified packed frames.
use super::*;

#[derive(Default)]
pub(crate) struct ColdOperations {
    pub(super) turns: BTreeMap<(String, String), Vec<crate::live_index::FactLocation>>,
    pub(super) unassigned: BTreeMap<String, UnassignedUseRecords>,
}
pub(crate) fn load_operations(
    db: &rusqlite::Connection,
    sources: &[SourceReport],
    threads: &[Thread],
) -> Result<ColdOperations> {
    load(db, sources, threads, true)
}
pub(crate) fn load_metadata(
    db: &rusqlite::Connection,
    sources: &[SourceReport],
    threads: &[Thread],
) -> Result<ColdOperations> {
    let mut result = load(db, sources, threads, false)?;
    let mut query=db.prepare_cached("SELECT DISTINCT thread,j.value FROM measurement_summaries,json_each(targets) j WHERE source=?1 AND thread IS NOT NULL")?;
    for source in sources {
        let mut rows = query.query([&source.source.id])?;
        while let Some(row) = rows.next()? {
            let thread: String = row.get(0)?;
            let turn: Option<String> = row.get(1)?;
            result
                .turns
                .entry((thread, turn.unwrap_or_else(|| "unassigned".into())))
                .or_default();
        }
    }
    Ok(result)
}
fn load(
    db: &rusqlite::Connection,
    sources: &[SourceReport],
    threads: &[Thread],
    locations: bool,
) -> Result<ColdOperations> {
    let owners: std::collections::HashMap<_, _> = threads
        .iter()
        .map(|t| (t.id.as_str(), t.source_instance_id.as_str()))
        .collect();
    let mut index = ColdOperations::default();
    for source in sources {
        let unpacked: bool = db.query_row("SELECT EXISTS(SELECT 1 FROM entries e JOIN buckets b ON b.id=e.bucket WHERE b.scope=?1 AND b.field='operations' AND e.id!='')", [format!("projection:{}", source.source.id)], |r| r.get(0))?;
        anyhow::ensure!(!unpacked, "unexpected unpacked operations");
        crate::live_index::each_located(
            db,
            &format!("projection:{}", source.source.id),
            Some("operations"),
            |field, id, payload, location| {
                anyhow::ensure!(field == "operations", "cold operation field mismatch");
                let operation: Operation = serde_json::from_str(payload)?;
                anyhow::ensure!(operation.id == id, "cold operation identity mismatch");
                anyhow::ensure!(
                    owners.get(operation.thread_id.as_ref()).copied()
                        == Some(source.source.id.as_str()),
                    "cold operation source mismatch"
                );
                if operation.turn_id.as_deref().is_none_or(str::is_empty) {
                    index
                        .unassigned
                        .entry(operation.thread_id.to_string())
                        .or_default()
                        .observe(&operation)?;
                }
                let target = index
                    .turns
                    .entry((
                        operation.thread_id.to_string(),
                        operation
                            .turn_id
                            .as_deref()
                            .unwrap_or("unassigned")
                            .to_owned(),
                    ))
                    .or_default();
                if locations {
                    target.push(crate::live_index::FactLocation {
                        block: location.block,
                        slot: location.slot,
                        id: id.to_owned(),
                    });
                }
                Ok(())
            },
        )?;
    }
    Ok(index)
}
impl Snapshot {
    pub(super) fn turn_operations(
        &self,
        thread: &str,
        turn: &str,
        data: &MemoryTurn,
    ) -> Result<Vec<Arc<Operation>>> {
        let mut touched = 0_u64;
        self.turn_operations_budgeted(thread, turn, data, |bytes| {
            touched = touched
                .checked_add(bytes)
                .ok_or_else(|| operation_error("RESOURCE_LIMIT", "冷操作读取大小溢出"))?;
            if touched > MAX_FILE_BYTES {
                return Err(operation_error("RESOURCE_LIMIT", "轮次操作分片过大"));
            }
            Ok(())
        })
    }
    pub(super) fn turn_operations_budgeted(
        &self,
        thread: &str,
        turn: &str,
        data: &MemoryTurn,
        charge: impl FnMut(u64) -> Result<()>,
    ) -> Result<Vec<Arc<Operation>>> {
        if data.cold_operations.is_empty() {
            return Ok(data.operations.clone());
        }
        let reader = self
            .cold_events
            .as_ref()
            .ok_or_else(|| corrupt("冷操作读取版本缺失"))?;
        let rows = reader.operations(&data.cold_operations, charge)?;
        if rows.iter().any(|op| {
            op.thread_id.as_ref() != thread || op.turn_id.as_deref().unwrap_or("unassigned") != turn
        }) {
            return Err(corrupt("冷操作归属不匹配"));
        }
        Ok(rows)
    }
}
