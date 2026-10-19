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
/// Inputs for accounting, ownership, activity and canonical operation linking.
/// Message content and unrelated safe observations stay solely in the archive.
pub(super) fn projection_input(event: &Event) -> bool {
    crate::operation_association::is_projection_input(event)
        || matches!(
            event.payload(),
            crate::session_events::Payload::Thread { .. }
                | crate::session_events::Payload::Turn { .. }
                | crate::session_events::Payload::Ancestry { .. }
                | crate::session_events::Payload::Measurement { .. }
        )
}
pub(super) fn prepare(db: &Connection) -> Result<()> {
    db.execute_batch("CREATE TEMP TABLE IF NOT EXISTS written_event_blocks(id INTEGER PRIMARY KEY,metadata TEXT NOT NULL); CREATE TEMP TABLE IF NOT EXISTS written_association_blocks(id INTEGER PRIMARY KEY,payload BLOB NOT NULL,bytes INTEGER NOT NULL,digest TEXT NOT NULL,codec INTEGER NOT NULL);")?;
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
        message_count: events
            .iter()
            .filter(|e| matches!(e.payload(), crate::session_events::Payload::Message { .. }))
            .count(),
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
    (associations, association_count): (&[u8], usize),
    encoder: &mut codec::Encoder,
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
        let (payload, bytes, digest) = encoder.encode_raw(associations)?;
        db.prepare_cached("INSERT INTO written_association_blocks VALUES(?1,?2,?3,?4,?5) ON CONFLICT(id) DO UPDATE SET payload=excluded.payload,bytes=excluded.bytes,digest=excluded.digest,codec=excluded.codec")?.execute(params![id,payload,bytes as i64,digest,codec::VERSION])?;
    }
    Ok(())
}
pub(super) fn associations(db: &Connection, id: i64, written: &Written) -> Result<Rows> {
    if written.association_count == 0 {
        return Ok(vec![]);
    }
    let (payload, bytes, digest, version) = db
        .prepare_cached(
            "SELECT payload,bytes,digest,codec FROM written_association_blocks WHERE id=?1",
        )?
        .query_row([id], |r| {
            Ok((
                r.get::<_, Vec<u8>>(0)?,
                r.get::<_, i64>(1)?,
                r.get::<_, String>(2)?,
                r.get::<_, i64>(3)?,
            ))
        })?;
    let events: Rows = codec::decode(&payload, bytes, &digest, version)?;
    require_index(
        events.len() == written.association_count
            && events.len() <= written.chunk.count
            && events.iter().all(|e| projection_input(e)),
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
