//! Pass-local facts from frames we just encoded. SQLite spills metadata to disk;
//! publication need not decode these same observation payloads again.
use super::*;
use crate::usage_store::{EventChunk, FileRef};

#[derive(serde::Serialize, serde::Deserialize)]
pub(super) struct Written {
    pub target: crate::usage_store::EventTarget,
    pub chunk: EventChunk,
    pub boundaries: Rows,
    pub association_count: usize,
}
pub(super) fn prepare(db: &Connection) -> Result<()> {
    db.execute_batch("CREATE TEMP TABLE IF NOT EXISTS written_event_blocks(id INTEGER PRIMARY KEY,metadata TEXT NOT NULL); CREATE TEMP TABLE IF NOT EXISTS written_association_blocks(id INTEGER PRIMARY KEY,payload TEXT NOT NULL);")?;
    Ok(())
}
pub(super) fn clear(db: &Connection) -> Result<()> {
    prepare(db)?;
    db.execute("DELETE FROM written_event_blocks", [])?;
    db.execute("DELETE FROM written_association_blocks", [])?;
    Ok(())
}
pub(super) fn chunk(id: i64, digest: String, bytes: u64, events: &[Arc<Event>]) -> EventChunk {
    EventChunk {
        offset: 0,
        file: FileRef {
            file: format!("live-block-{id}.json"),
            sha256: digest,
        },
        count: events.len(),
        bytes,
        first: events[0].position().clone(),
        last: events[events.len() - 1].position().clone(),
    }
}
pub(super) fn save(
    db: &Connection,
    id: i64,
    digest: String,
    bytes: u64,
    events: &[Arc<Event>],
    associations: &[u8],
    association_count: usize,
) -> Result<()> {
    let written = Written {
        target: crate::usage_store::EventTarget {
            thread_id: events[0].thread_id().map(str::to_owned),
            turn_id: events[0].turn_id().map(str::to_owned),
        },
        chunk: chunk(id, digest, bytes, events),
        boundaries: events
            .iter()
            .filter(|event| {
                matches!(
                    event.payload(),
                    crate::session_events::Payload::Lifecycle {
                        lifecycle: crate::session_events::LifecycleKind::Turn,
                        ..
                    }
                )
            })
            .cloned()
            .collect(),
        association_count,
    };
    db.prepare_cached("INSERT INTO written_event_blocks VALUES(?1,?2) ON CONFLICT(id) DO UPDATE SET metadata=excluded.metadata")?
        .execute(params![id,serde_json::to_string(&written)?])?;
    if association_count == 0 {
        db.execute("DELETE FROM written_association_blocks WHERE id=?1", [id])?;
    } else {
        db.prepare_cached("INSERT INTO written_association_blocks VALUES(?1,?2) ON CONFLICT(id) DO UPDATE SET payload=excluded.payload")?.execute(params![id,std::str::from_utf8(associations)?])?;
    }
    Ok(())
}
pub(super) fn associations(db: &Connection, id: i64, written: &Written) -> Result<Rows> {
    if written.association_count == 0 {
        return Ok(vec![]);
    }
    let payload: String = db
        .prepare_cached("SELECT payload FROM written_association_blocks WHERE id=?1")?
        .query_row([id], |r| r.get(0))?;
    let events: Rows = serde_json::from_str(&payload)?;
    require_index(
        events.len() == written.association_count
            && events.len() <= written.chunk.count
            && events
                .iter()
                .all(|e| crate::operation_association::is_input(e)),
        "written association count mismatch",
    )?;
    Ok(events)
}
pub(super) fn load(
    db: &Connection,
    id: i64,
    digest: &str,
    bytes: i64,
    count: i64,
) -> Result<Option<Written>> {
    let raw: Option<String> = db
        .prepare_cached("SELECT metadata FROM written_event_blocks WHERE id=?1")?
        .query_row([id], |r| r.get(0))
        .optional()?;
    raw.map(|raw| {
        let written: Written = serde_json::from_str(&raw)?;
        require_index(
            written.chunk.file.sha256 == digest
                && written.chunk.bytes == bytes as u64
                && written.chunk.count as i64 == count
                && written.chunk.file.file == format!("live-block-{id}.json"),
            "written observation metadata mismatch",
        )?;
        Ok(written)
    })
    .transpose()
}
