//! Original text locators only. JSON decoding stays in serde_json; no body is stored.
use anyhow::{Result, ensure};
use serde::{Deserialize, Serialize};
use serde_json::value::RawValue;

pub(crate) const CHUNK_BYTES: usize = 8192;
const MAX_CHUNKS: usize = 4096;
#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TextChunk {
    pub offset: u64,
    pub bytes: u32,
    pub part: u32,
    pub sha256: String,
    pub empty: bool,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OriginalContent {
    pub chunks: Vec<TextChunk>,
    pub partial: bool,
}
impl OriginalContent {
    pub(crate) fn empty_value(&mut self, raw: &RawValue, row: (usize, usize)) {
        let encoded = raw.get();
        let base = (encoded.as_ptr() as usize).checked_sub(row.0);
        if let Some(base) = base
            && encoded.len() <= CHUNK_BYTES
            && base
                .checked_add(encoded.len())
                .is_some_and(|end| end <= row.1)
        {
            self.chunks.push(TextChunk {
                offset: base as u64,
                bytes: encoded.len() as u32,
                part: 0,
                sha256: crate::hash(encoded.as_bytes()),
                empty: true,
            });
        } else {
            self.partial = true;
        }
    }
    pub(crate) fn validate(&self) -> Result<()> {
        ensure!(self.chunks.len() <= MAX_CHUNKS, "too many content chunks");
        let mut end = 0;
        for chunk in &self.chunks {
            ensure!(
                chunk.bytes > 0 && chunk.bytes as usize <= CHUNK_BYTES + 12,
                "invalid content chunk size"
            );
            ensure!(
                chunk.offset >= end
                    && chunk.sha256.len() == 64
                    && chunk.sha256.bytes().all(|b| b.is_ascii_hexdigit()),
                "invalid content locator"
            );
            end = chunk
                .offset
                .checked_add(u64::from(chunk.bytes))
                .ok_or_else(|| anyhow::anyhow!("content offset overflow"))?;
        }
        Ok(())
    }
    /// The borrowed RawValue was validated by serde_json and lies in this source row.
    /// Split only at UTF-8/JSON escape boundaries, keeping surrogate pairs together.
    pub(crate) fn text(&mut self, raw: &RawValue, row: (usize, usize), part: u32) {
        let encoded = raw.get();
        if !encoded.starts_with('"') || !encoded.ends_with('"') {
            self.partial = true;
            return;
        }
        let Some(base) = (encoded.as_ptr() as usize).checked_sub(row.0) else {
            self.partial = true;
            return;
        };
        if base.checked_add(encoded.len()).is_none_or(|n| n > row.1) {
            self.partial = true;
            return;
        }
        let text = &encoded[1..encoded.len() - 1];
        if text.is_empty() {
            if self.chunks.len() == MAX_CHUNKS {
                self.partial = true;
                return;
            }
            self.chunks.push(TextChunk {
                offset: base as u64,
                bytes: 2,
                part,
                sha256: crate::hash(encoded.as_bytes()),
                empty: true,
            });
            return;
        }
        let bytes = text.as_bytes();
        let mut start = 0;
        while start < text.len() {
            if self.chunks.len() == MAX_CHUNKS {
                self.partial = true;
                return;
            }
            let mut end = start;
            while end < text.len() {
                let width = if bytes[end] == b'\\' {
                    if bytes[end + 1] == b'u' {
                        let high = u16::from_str_radix(&text[end + 2..end + 6], 16).unwrap_or(0);
                        if (0xd800..=0xdbff).contains(&high)
                            && text.get(end + 6..end + 8) == Some("\\u")
                            && text
                                .get(end + 8..end + 12)
                                .and_then(|s| u16::from_str_radix(s, 16).ok())
                                .is_some_and(|low| (0xdc00..=0xdfff).contains(&low))
                        {
                            12
                        } else {
                            6
                        }
                    } else {
                        2
                    }
                } else {
                    text[end..].chars().next().unwrap().len_utf8()
                };
                if end + width > text.len() {
                    self.partial = true;
                    return;
                }
                if end > start && end + width - start > CHUNK_BYTES {
                    break;
                }
                end += width;
            }
            self.chunks.push(TextChunk {
                offset: (base + 1 + start) as u64,
                bytes: (end - start) as u32,
                part,
                sha256: crate::hash(&bytes[start..end]),
                empty: false,
            });
            start = end;
        }
    }
}
