//! Locators are version/scope bound, never authority to select a different view or target.
use super::*;
use serde::{Deserialize, Serialize};
#[derive(Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Binding {
    snapshot: String,
    schema: u32,
    source: String,
    thread: String,
    turn: String,
    collection: EvidenceSet,
    object: Option<String>,
    method: u32,
    offset: usize,
}
fn binding(
    snapshot: &Snapshot,
    target: TurnTarget<'_>,
    collection: EvidenceSet,
    object: Option<&str>,
    offset: usize,
) -> Binding {
    Binding {
        snapshot: snapshot.manifest.snapshot_ref.snapshot_id.clone(),
        schema: snapshot.manifest.schema_version,
        source: target.source.into(),
        thread: target.thread.into(),
        turn: target.turn.into(),
        collection,
        object: object.map(str::to_owned),
        method: observations::METHOD_VERSION,
        offset,
    }
}
pub(super) fn encode(
    snapshot: &Snapshot,
    target: TurnTarget<'_>,
    collection: EvidenceSet,
    object: Option<&str>,
    offset: usize,
) -> Result<Cursor> {
    let bytes = serde_json::to_vec(&binding(snapshot, target, collection, object, offset))?;
    if bytes.len() > super::super::MAX_CURSOR_BYTES / 2 {
        return Err(limit());
    }
    Ok(Cursor {
        token: bytes.iter().map(|byte| format!("{byte:02x}")).collect(),
    })
}
pub(super) fn offset(
    snapshot: &Snapshot,
    target: TurnTarget<'_>,
    collection: EvidenceSet,
    object: Option<&str>,
    cursor: Option<&Cursor>,
) -> Result<usize> {
    let Some(cursor) = cursor else {
        return Ok(0);
    };
    let bytes = cursor.token.as_bytes();
    if bytes.is_empty()
        || bytes.len() > super::super::MAX_CURSOR_BYTES
        || !bytes.len().is_multiple_of(2)
        || !bytes.iter().all(u8::is_ascii_hexdigit)
    {
        return super::super::invalid_field(
            "/cursor/token",
            "use the unmodified token from nextCursor.token",
        );
    }
    let bytes = bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u8::from_str_radix(std::str::from_utf8(pair).unwrap(), 16).unwrap())
        .collect::<Vec<_>>();
    let decoded: Binding = serde_json::from_slice(&bytes)
        .map_err(|_| operation_error("INVALID_ARGUMENT", "Invalid use evidence cursor"))?;
    if decoded != binding(snapshot, target, collection, object, decoded.offset) {
        return super::super::invalid_field(
            "/cursor",
            "does not match the original snapshot, source, thread, turn, collection or objectRef",
        );
    }
    Ok(decoded.offset)
}
pub(super) fn reference(snapshot: &Snapshot, target: TurnTarget<'_>, fields: &[&str]) -> String {
    use sha2::{Digest, Sha256};
    let mut hash = Sha256::new();
    // Length prefixes preserve field boundaries without allocating serialized targets.
    for value in [
        "canonical_object_use_v1",
        &snapshot.manifest.snapshot_ref.snapshot_id,
        target.source,
        target.thread,
        target.turn,
    ]
    .into_iter()
    .chain(fields.iter().copied())
    {
        hash.update((value.len() as u64).to_le_bytes());
        hash.update(value.as_bytes());
    }
    format!("use:{:x}", hash.finalize())
}
