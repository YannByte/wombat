//! Bounded seeks to independently verified JSON-string fragments. No record replay.
use super::*;
use crate::{
    conversation_source::{OriginalContent, TextChunk},
    file_metadata::FileMetadata,
};
use std::path::PathBuf;
struct OpenSource {
    source: String,
    file_id: String,
    file: File,
    path: PathBuf,
    physical: String,
    boundary: u64,
}
pub(super) struct Reader<'a> {
    snapshot: &'a Snapshot,
    files: VecDeque<OpenSource>,
    pub bytes: u64,
    budget: u64,
}
impl<'a> Reader<'a> {
    pub fn new(snapshot: &'a Snapshot, budget: u64) -> Self {
        Self {
            snapshot,
            files: VecDeque::new(),
            bytes: 0,
            budget,
        }
    }
    pub fn chunk(
        &mut self,
        event: &Event,
        chunk: &TextChunk,
        cancelled: &AtomicBool,
    ) -> Result<(String, ContentLocation)> {
        check(cancelled)?;
        if u64::from(chunk.bytes) > self.budget.saturating_sub(self.bytes) {
            return Err(error("RESOURCE_LIMIT"));
        }
        let p = event.position();
        let index = match self.files.iter().position(|s| {
            s.source == p.source_instance_id.as_ref() && s.file_id == p.file_id.as_ref()
        }) {
            Some(index) => index,
            None => {
                let watermark = self
                    .snapshot
                    .manifest
                    .watermarks
                    .iter()
                    .find(|w| {
                        w.source_instance_id == p.source_instance_id.as_ref()
                            && w.file_id == p.file_id.as_ref()
                    })
                    .ok_or_else(|| error("CONTENT_UNAVAILABLE"))?;
                if watermark.generation.as_deref() != Some(p.generation.as_ref()) {
                    return Err(error("SOURCE_CHANGED"));
                }
                let original = watermark
                    .original_source
                    .as_ref()
                    .ok_or_else(|| error("CONTENT_UNAVAILABLE"))?;
                let source = self
                    .snapshot
                    .manifest
                    .sources
                    .iter()
                    .find(|s| s.source.id == watermark.source_instance_id)
                    .ok_or_else(|| error("CONTENT_UNAVAILABLE"))?;
                let path =
                    dunce::canonicalize(&original.path).map_err(|_| error("SOURCE_UNREADABLE"))?;
                let root = dunce::canonicalize(&source.source.root)
                    .map_err(|_| error("SOURCE_UNREADABLE"))?;
                if !path.starts_with(&root) {
                    return Err(error("SOURCE_CHANGED"));
                }
                let file = File::open(&path).map_err(|_| error("SOURCE_UNREADABLE"))?;
                if self.files.len() == 8 {
                    self.files.pop_front();
                }
                self.files.push_back(OpenSource {
                    source: watermark.source_instance_id.clone(),
                    file_id: watermark.file_id.clone(),
                    file,
                    path,
                    physical: original.physical_id.clone(),
                    boundary: watermark.committed_offset,
                });
                self.files.len() - 1
            }
        };
        let source = &mut self.files[index];
        let before =
            FileMetadata::from_file(&source.file).map_err(|_| error("SOURCE_UNREADABLE"))?;
        if before.physical != source.physical || before.len() < source.boundary || !before.is_file()
        {
            return Err(error("SOURCE_CHANGED"));
        }
        let start = p
            .byte_offset
            .checked_add(chunk.offset)
            .ok_or_else(|| error("SNAPSHOT_CORRUPT"))?;
        let end = start
            .checked_add(u64::from(chunk.bytes))
            .ok_or_else(|| error("SNAPSHOT_CORRUPT"))?;
        if end > source.boundary {
            return Err(error("SNAPSHOT_CORRUPT"));
        }
        source
            .file
            .seek(SeekFrom::Start(start))
            .map_err(|_| error("SOURCE_UNREADABLE"))?;
        let mut bytes = vec![0; chunk.bytes as usize];
        source
            .file
            .read_exact(&mut bytes)
            .map_err(|_| error("SOURCE_CHANGED"))?;
        self.bytes += u64::from(chunk.bytes);
        let after =
            FileMetadata::from_file(&source.file).map_err(|_| error("SOURCE_UNREADABLE"))?;
        let current = FileMetadata::read(&source.path).map_err(|_| error("SOURCE_CHANGED"))?;
        if after.physical != source.physical
            || current.physical != source.physical
            || after.len() < before.len()
            || after.len() == before.len() && after.modified().ok() != before.modified().ok()
            || crate::hash(&bytes) != chunk.sha256
        {
            return Err(error("SOURCE_CHANGED"));
        }
        let mut encoded = Vec::with_capacity(bytes.len() + 2);
        if !chunk.empty {
            encoded.push(b'"');
        }
        encoded.extend(bytes);
        if !chunk.empty {
            encoded.push(b'"');
        }
        let text = if chunk.empty {
            match serde_json::from_slice::<serde_json::Value>(&encoded)
                .map_err(|_| error("CONTENT_UNAVAILABLE"))?
            {
                serde_json::Value::String(s) if s.is_empty() => s,
                serde_json::Value::Array(a) if a.is_empty() => String::new(),
                _ => return Err(error("CONTENT_UNAVAILABLE")),
            }
        } else {
            serde_json::from_slice::<String>(&encoded).map_err(|_| error("CONTENT_UNAVAILABLE"))?
        };
        check(cancelled)?;
        Ok((
            text,
            ContentLocation {
                file: source.path.to_string_lossy().into_owned(),
                byte_start: start,
                byte_end: end,
                part: chunk.part,
            },
        ))
    }
}
fn suffix(text: &str, bytes: usize) -> &str {
    let mut start = text.len().saturating_sub(bytes);
    while !text.is_char_boundary(start) {
        start += 1;
    }
    &text[start..]
}
pub(super) fn search(
    reader: &mut Reader<'_>,
    event: &Event,
    content: &OriginalContent,
    needle: &str,
    next: &mut usize,
    cancelled: &AtomicBool,
) -> Result<bool> {
    if *next > content.chunks.len() {
        return Err(error("INVALID_ARGUMENT"));
    }
    let mut start = *next;
    let mut raw = 0;
    while start > 0
        && start < content.chunks.len()
        && content.chunks[start - 1].part == content.chunks[*next].part
        && raw < needle.len() * 6 + 16
    {
        start -= 1;
        raw += content.chunks[start].bytes as usize;
    }
    let mut tail = String::new();
    let mut part = None;
    let started = Instant::now();
    for (index, chunk) in content.chunks.iter().enumerate().skip(start) {
        check(cancelled)?;
        if started.elapsed().as_millis() >= 60 {
            return Err(error("RESOURCE_LIMIT"));
        }
        if part.is_some_and(|p| p != chunk.part) {
            tail.clear();
        }
        let (value, _) = reader.chunk(event, chunk, cancelled)?;
        tail.push_str(&value);
        part = Some(chunk.part);
        if index >= *next {
            *next = index + 1;
            if tail.contains(needle) {
                return Ok(true);
            }
        }
        tail = suffix(&tail, needle.len().saturating_sub(1)).to_owned();
    }
    Ok(false)
}
