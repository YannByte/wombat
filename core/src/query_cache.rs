//! Bounded result reuse owned by one immutable live view, never shared across revisions.
//! CBOR preserves floats in internally tagged responses while accounting JSON
//! retains arbitrary precision; no persistent index or public protocol changes.
use crate::usage_app_dto::{Action, Request, Response};
use std::{
    collections::VecDeque,
    io::{self, Write},
};

struct BudgetWriter {
    bytes: Vec<u8>,
    key_bytes: usize,
}
impl Write for BudgetWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > MAX_ENTRY_BYTES.saturating_sub(self.key_bytes + self.bytes.len()) {
            return Err(io::Error::other("cache entry budget exceeded"));
        }
        self.bytes.extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

const MAX_ENTRIES: usize = 16;
const MAX_BYTES: usize = 2 * 1024 * 1024;
const MAX_ENTRY_BYTES: usize = 512 * 1024;

/// Compact pages share one default-sized window; all business filters remain in
/// the request key. Crossing a window or using location-driven paging stays exact.
pub(crate) struct PageWindow {
    offset: usize,
    limit: usize,
    start: usize,
}
impl PageWindow {
    pub fn for_request(r: &Request) -> Option<Self> {
        let offset = r.offset.unwrap_or(0);
        let limit = r.limit.unwrap_or(50);
        let start = offset / 50 * 50;
        (r.compact == Some(true)
            && matches!(
                r.action,
                Action::Usage | Action::Threads | Action::Statistics
            )
            && r.locate_thread_id.is_none()
            && r.locate_turn_id.is_none()
            && r.locate_operation_id.is_none()
            && limit > 0
            && limit <= 50
            && offset.saturating_add(limit) <= start.saturating_add(50))
        .then_some(Self {
            offset,
            limit,
            start,
        })
    }
    pub fn request(&self, r: &Request) -> Request {
        let mut r = r.clone();
        r.offset = Some(self.start);
        r.limit = Some(50);
        r
    }
    pub fn project(&self, mut r: Response) -> Response {
        let skip = self.offset - self.start;
        r.items = r.items.into_iter().skip(skip).take(self.limit).collect();
        let count = if r.action == Action::Statistics {
            if let Some(stats) = &mut r.statistics {
                stats.groups = std::mem::take(&mut stats.groups)
                    .into_iter()
                    .skip(skip)
                    .take(self.limit)
                    .collect();
                stats.groups.len()
            } else {
                0
            }
        } else {
            r.items.len()
        };
        r.page.offset = self.offset;
        r.page.limit = self.limit;
        let next = self.offset.saturating_add(count);
        r.page.next_offset = (next < r.page.total).then_some(next);
        r
    }
}

#[derive(Default)]
pub(crate) struct QueryCache {
    entries: VecDeque<(String, Vec<u8>, usize)>,
    bytes: usize,
}
impl QueryCache {
    pub fn get(&mut self, key: &str) -> Option<Response> {
        let index = self.entries.iter().position(|(k, _, _)| k == key)?;
        let entry = self.entries.remove(index)?;
        let mut input = entry.1.as_slice();
        match ciborium::from_reader(&mut input) {
            Ok(result) if input.is_empty() => {
                self.entries.push_back(entry);
                Some(result)
            }
            _ => {
                self.bytes -= entry.2;
                None
            }
        }
    }
    // Retain the same bounded encoding used for admission. This avoids keeping
    // thousands of cloned row objects alive on request threads' retired heaps.
    pub fn encode(key: &str, response: &impl serde::Serialize) -> Option<Vec<u8>> {
        if key.len() > MAX_ENTRY_BYTES {
            return None;
        }
        let mut writer = BudgetWriter {
            bytes: Vec::new(),
            key_bytes: key.len(),
        };
        ciborium::into_writer(response, &mut writer).ok()?;
        writer.bytes.shrink_to_fit();
        Some(writer.bytes)
    }
    pub fn insert(&mut self, key: String, payload: Vec<u8>) {
        let bytes = key.len().saturating_add(payload.len());
        if bytes > MAX_ENTRY_BYTES {
            return;
        }
        if let Some(index) = self.entries.iter().position(|(k, _, _)| k == &key) {
            self.bytes -= self.entries.remove(index).unwrap().2;
        }
        while self.entries.len() >= MAX_ENTRIES || self.bytes + bytes > MAX_BYTES {
            if let Some((_, _, size)) = self.entries.pop_front() {
                self.bytes -= size;
            } else {
                break;
            }
        }
        self.bytes += bytes;
        self.entries.push_back((key, payload, bytes));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn response() -> Response {
        let root = tempfile::tempdir().unwrap();
        let prices = crate::pricing_sync::current_at(root.path()).unwrap();
        let snapshot =
            crate::usage_store::memory(Default::default(), "live:test:one".into(), prices, None)
                .unwrap();
        crate::usage_app::execute_snapshot(
            serde_json::from_value(
                serde_json::json!({"action":"usage","scope":{"timezone":"UTC"}}),
            )
            .unwrap(),
            &snapshot,
        )
        .unwrap()
    }
    #[test]
    fn lru_bounds_replacement_and_response_ownership() {
        let result = response();
        let encoded = QueryCache::encode("", &result).unwrap();
        let mut cache = QueryCache::default();
        for i in 0..MAX_ENTRIES {
            cache.insert(i.to_string(), encoded.clone());
        }
        let mut copy = cache.get("0").unwrap();
        copy.snapshot_ref.snapshot_id = "changed".into();
        assert_ne!(cache.get("0").unwrap().snapshot_ref.snapshot_id, "changed");
        cache.insert("next".into(), encoded.clone());
        assert!(cache.get("1").is_none());
        let before = cache.bytes;
        cache.insert("0".into(), encoded.clone());
        assert_eq!(cache.bytes, before);
        assert_eq!(cache.entries.len(), MAX_ENTRIES);
        for i in 0..20 {
            let key = format!("large{i}");
            let n = MAX_ENTRY_BYTES - key.len();
            cache.insert(key, vec![0; n]);
        }
        assert_eq!(cache.bytes, MAX_BYTES);
        assert_eq!(cache.entries.len(), MAX_BYTES / MAX_ENTRY_BYTES);
        assert!(cache.get("large19").is_none()); // Invalid memory-only encoding is a miss.
        assert_eq!(cache.bytes, MAX_BYTES - MAX_ENTRY_BYTES);
        cache.insert("oversized".into(), vec![0; MAX_ENTRY_BYTES]);
        assert!(!cache.entries.iter().any(|(k, _, _)| k == "oversized"));
    }
    #[test]
    fn encoded_budget_counts_utf8_and_rejects_large_values() {
        let value = serde_json::json!({"text":"中文\n"});
        assert_eq!(
            QueryCache::encode("key", &value).unwrap(),
            vec![
                0xa1, 0x64, b't', b'e', b'x', b't', 0x67, 0xe4, 0xb8, 0xad, 0xe6, 0x96, 0x87, 0x0a
            ]
        );
        assert!(QueryCache::encode("", &"x".repeat(MAX_ENTRY_BYTES)).is_none());
        assert!(QueryCache::encode(&"x".repeat(MAX_ENTRY_BYTES + 1), &0).is_none());
    }
    #[test]
    fn cache_preserves_opaque_json_numbers_without_losing_precision() {
        let value: serde_json::Value = serde_json::from_str(r#"{"integer":9007199254740993,"fraction":0.125,"integralFloat":1.0,"huge":123456789012345678901234567890}"#).unwrap();
        let encoded = QueryCache::encode("value", &value).unwrap();
        let decoded: serde_json::Value = ciborium::from_reader(encoded.as_slice()).unwrap();
        assert_eq!(decoded, value);
    }
}
