//! Modification time orders preparation, never establishes event time or usage.
//! Complete files keep counter baselines; related copies and fork families load together.
use super::*;
use std::io::BufReader;
use std::time::{Duration, SystemTime};

const HEADER_BYTES: u64 = 16 * 1024 * 1024;
#[derive(Deserialize)]
struct Header {
    #[serde(rename = "type")]
    kind: String,
    payload: HeaderPayload,
}

#[derive(Deserialize)]
struct HeaderPayload {
    id: Option<String>,
    forked_from_id: Option<String>,
}

pub(super) fn select(
    files: &BTreeSet<PathBuf>,
    checkpoints: &BTreeMap<String, Checkpoint>,
) -> Result<BTreeSet<PathBuf>> {
    if files
        .iter()
        .all(|path| checkpoints.contains_key(path.to_string_lossy().as_ref()))
    {
        return Ok(files.clone());
    }
    let cutoff = SystemTime::now()
        .checked_sub(Duration::from_secs(30 * 86400))
        .unwrap_or(UNIX_EPOCH);
    let mut identities = BTreeMap::<String, Vec<PathBuf>>::new();
    let mut families = BTreeMap::<String, BTreeSet<String>>::new();
    let mut selected = BTreeSet::new();
    let mut pending = Vec::new();
    for path in files {
        let metadata = FileMetadata::read(path)?;
        let hot = metadata.modified().map_or(true, |at| at >= cutoff)
            || checkpoints.contains_key(path.to_string_lossy().as_ref());
        let mut reader = BufReader::new(File::open(path)?.take(HEADER_BYTES + 1));
        let mut buffer = Vec::new();
        let line = crate::log_io::next_line(&mut reader, &mut buffer)?;
        let bytes = line.as_ref().map_or(&[][..], |line| line.bytes());
        let header = (bytes.len() as u64 <= HEADER_BYTES && bytes.last() == Some(&b'\n'))
            .then(|| serde_json::from_slice::<Header>(bytes).ok())
            .flatten()
            .filter(|h| {
                h.kind == "session_meta" && h.payload.id.as_ref().is_some_and(|id| !id.is_empty())
            });
        let Some(header) = header else {
            // Unrecognized headers cannot establish independence from deferred files.
            return Ok(files.clone());
        };
        let id = header.payload.id.unwrap();
        identities.entry(id.clone()).or_default().push(path.clone());
        if let Some(parent) = header.payload.forked_from_id {
            families
                .entry(id.clone())
                .or_default()
                .insert(parent.clone());
            families.entry(parent).or_default().insert(id.clone());
        }
        if hot {
            pending.push(id);
        }
    }
    let mut visited = BTreeSet::new();
    while let Some(id) = pending.pop() {
        if !visited.insert(id.clone()) {
            continue;
        }
        if let Some(paths) = identities.get(&id) {
            selected.extend(paths.iter().cloned());
        }
        if let Some(related) = families.get(&id) {
            pending.extend(related.iter().cloned());
        }
    }
    Ok(selected)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn log(root: &Path, name: &str, parent: Option<&str>, total: u64, old: bool) {
        let path = root.join("sessions").join(format!("{name}.jsonl"));
        let header = serde_json::json!({"type":"session_meta","payload":{"id":name,"forked_from_id":parent}});
        let usage = serde_json::json!({"type":"event_msg","timestamp":"2026-10-14T00:00:00Z","payload":{"type":"token_usage_record","response_id":name,"usage":{"input_tokens":total-10,"output_tokens":10,"total_tokens":total}}});
        fs::write(&path, format!("{header}\n{usage}\n")).unwrap();
        if old {
            File::options()
                .write(true)
                .open(path)
                .unwrap()
                .set_modified(UNIX_EPOCH + Duration::from_secs(1_000_000))
                .unwrap();
        }
    }
    #[test]
    fn recent_preparation_loads_complete_families_and_history_expansion_conserves_usage() {
        let root = tempfile::tempdir().unwrap();
        let storage = tempfile::tempdir().unwrap();
        fs::create_dir(root.path().join("sessions")).unwrap();
        log(root.path(), "parent", None, 110, true);
        log(root.path(), "child", Some("parent"), 220, false);
        log(root.path(), "unrelated", None, 440, true);
        let source = CodexAdapter
            .discover(&DiscoveryRequest {
                roots: vec![root.path().into()],
            })
            .sources
            .remove(0);
        let db = crate::live_index::open(&storage.path().join("index.sqlite")).unwrap();
        let mut cache = Cache::default();
        let first = sync_cached_recent(&db, &source, false, &mut cache)
            .unwrap()
            .unwrap()
            .collected;
        assert_eq!(first.sources[0].files_read, 2);
        assert_eq!(
            first
                .measurements
                .iter()
                .map(|m| m.tokens.total.unwrap())
                .sum::<u64>(),
            330
        );
        assert!(first.issues.iter().any(|i| i.code == "historyDeferred"));
        let complete = sync_cached(&db, &source, false, &mut cache)
            .unwrap()
            .unwrap()
            .collected;
        assert_eq!(
            complete
                .measurements
                .iter()
                .map(|m| m.tokens.total.unwrap())
                .sum::<u64>(),
            770
        );
        assert!(!complete.issues.iter().any(|i| i.code == "historyDeferred"));
        assert!(
            sync_cached_recent(&db, &source, false, &mut cache)
                .unwrap()
                .is_none()
        );
    }
    #[test]
    fn large_valid_headers_keep_unrelated_history_cold() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir(root.path().join("sessions")).unwrap();
        log(root.path(), "hot", None, 110, false);
        log(root.path(), "cold", None, 220, true);
        let header = serde_json::json!({"type":"session_meta","payload":{"id":"hot","instructions":"x".repeat(2 * 1024 * 1024)}});
        fs::write(
            root.path().join("sessions/hot.jsonl"),
            format!("{header}\n"),
        )
        .unwrap();
        let files = fs::read_dir(root.path().join("sessions"))
            .unwrap()
            .map(|p| p.unwrap().path())
            .collect();
        let selected = select(&files, &BTreeMap::new()).unwrap();
        assert_eq!(selected.len(), 1);
        assert!(selected.iter().next().unwrap().ends_with("hot.jsonl"));
    }
    #[test]
    fn unknown_headers_force_full_scan_and_names_never_define_recency() {
        let root = tempfile::tempdir().unwrap();
        fs::create_dir(root.path().join("sessions")).unwrap();
        log(root.path(), "1990-01-01", None, 110, false);
        log(root.path(), "2099-01-01", None, 220, true);
        let files = fs::read_dir(root.path().join("sessions"))
            .unwrap()
            .map(|p| p.unwrap().path())
            .collect();
        let selected = select(&files, &BTreeMap::new()).unwrap();
        assert_eq!(selected.len(), 1);
        assert!(
            selected
                .iter()
                .next()
                .unwrap()
                .ends_with("1990-01-01.jsonl")
        );
        fs::write(root.path().join("sessions/2099-01-01.jsonl"), "{}\n").unwrap();
        assert_eq!(select(&files, &BTreeMap::new()).unwrap(), files);
    }
}
