//! Bounded result reuse owned by one immutable live view, never shared across revisions.
use crate::usage_app_dto::Response;
use std::{
    collections::VecDeque,
    io::{self, Write},
};

struct BudgetWriter(usize);
impl Write for BudgetWriter {
    fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
        if bytes.len() > MAX_ENTRY_BYTES.saturating_sub(self.0) {
            return Err(io::Error::other("cache entry budget exceeded"));
        }
        self.0 += bytes.len();
        Ok(bytes.len())
    }
    fn flush(&mut self) -> io::Result<()> {
        Ok(())
    }
}

const MAX_ENTRIES: usize = 16;
const MAX_BYTES: usize = 2 * 1024 * 1024;
const MAX_ENTRY_BYTES: usize = 512 * 1024;

#[derive(Default)]
pub(crate) struct QueryCache {
    entries: VecDeque<(String, Response, usize)>,
    bytes: usize,
}
impl QueryCache {
    pub fn get(&mut self, key: &str) -> Option<Response> {
        let index = self.entries.iter().position(|(k, _, _)| k == key)?;
        let entry = self.entries.remove(index)?;
        let result = entry.1.clone();
        self.entries.push_back(entry);
        Some(result)
    }
    // Count encoded bytes without allocating a second response or holding the cache lock.
    pub fn entry_size(key: &str, response: &impl serde::Serialize) -> Option<usize> {
        if key.len() > MAX_ENTRY_BYTES {
            return None;
        }
        let mut writer = BudgetWriter(key.len());
        serde_json::to_writer(&mut writer, response).ok()?;
        Some(writer.0)
    }
    pub fn insert(&mut self, key: String, response: &Response, bytes: usize) {
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
        self.entries.push_back((key, response.clone(), bytes));
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
        let mut cache = QueryCache::default();
        for i in 0..MAX_ENTRIES {
            cache.insert(i.to_string(), &result, 100);
        }
        let mut copy = cache.get("0").unwrap();
        copy.snapshot_ref.snapshot_id = "changed".into();
        assert_ne!(cache.get("0").unwrap().snapshot_ref.snapshot_id, "changed");
        cache.insert("next".into(), &result, 100);
        assert!(cache.get("1").is_none());
        cache.insert("0".into(), &result, 200);
        assert_eq!(cache.bytes, MAX_ENTRIES * 100 + 100);
        assert_eq!(cache.entries.len(), MAX_ENTRIES);
        for i in 0..20 {
            cache.insert(format!("large{i}"), &result, MAX_ENTRY_BYTES);
        }
        assert_eq!(cache.bytes, MAX_BYTES);
        assert_eq!(cache.entries.len(), MAX_BYTES / MAX_ENTRY_BYTES);
    }
    #[test]
    fn encoded_budget_counts_utf8_and_rejects_large_values() {
        let value = serde_json::json!({"text":"中文\n"});
        assert_eq!(
            QueryCache::entry_size("key", &value),
            Some(3 + serde_json::to_vec(&value).unwrap().len())
        );
        assert_eq!(
            QueryCache::entry_size("", &"x".repeat(MAX_ENTRY_BYTES)),
            None
        );
        assert_eq!(
            QueryCache::entry_size(&"x".repeat(MAX_ENTRY_BYTES + 1), &0),
            None
        );
    }
}
