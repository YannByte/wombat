//! Bounded inventory for proving reuse of the same accounting input.
use super::*;
use crate::file_metadata::FileMetadata;

pub(crate) const MAX_FILES: usize = 256;
pub(crate) const MAX_BYTES: u64 = 512 * 1024 * 1024;

pub(crate) struct Inventory {
    pub fingerprint: String,
    pub bytes: u64,
}

pub(crate) fn capture(roots: &[String]) -> anyhow::Result<Option<Inventory>> {
    let adapter = CodexAdapter;
    let discovered = adapter.discover(&DiscoveryRequest {
        roots: roots.iter().map(PathBuf::from).collect(),
    });
    // The measured policy is for one local source. Larger or ambiguous
    // inventories remain regular foreground reads.
    if !discovered.issues.is_empty() || discovered.sources.len() != 1 {
        return Ok(None);
    }
    let source = discovered.sources.into_iter().next().unwrap();
    let root = Path::new(&source.root);
    if !root.is_dir() {
        return Ok(None);
    }
    let mut report = SourceReport {
        source,
        adapter_version: VERSION.into(),
        source_versions: vec![],
        capabilities: adapter.descriptor().capabilities,
        status: "complete".into(),
        files_read: 0,
        bytes_read: 0,
        issues: vec![],
    };
    let context = RunContext {
        max_files: MAX_FILES + 1,
        ..RunContext::default()
    };
    let root = PathBuf::from(&report.source.root);
    let mut files = BTreeSet::new();
    let mut visited = BTreeSet::new();
    for subdir in ["sessions", "archived_sessions"] {
        list_files(
            &root.join(subdir),
            0,
            &mut files,
            &mut visited,
            &context,
            &mut report,
        );
    }
    if !report.issues.is_empty() || files.len() > MAX_FILES {
        return Ok(None);
    }
    let mut hash = Sha256::new();
    hash.update(serde_json::to_vec(&(VERSION, &report.source.id, &root))?);
    let mut bytes = 0_u64;
    let title = root.join("session_index.jsonl");
    if title.try_exists()? {
        files.insert(title);
    }
    for path in files {
        let (meta, change) = FileMetadata::read_for_reuse(&path)?;
        if !meta.is_file() {
            return Ok(None);
        }
        bytes = bytes.saturating_add(meta.len());
        if bytes > MAX_BYTES {
            return Ok(None);
        }
        let modified = meta.modified()?;
        hash.update(serde_json::to_vec(&(
            path,
            &meta.physical,
            meta.len(),
            modified,
            change,
        ))?);
    }
    Ok(Some(Inventory {
        fingerprint: format!("{:x}", hash.finalize()),
        bytes,
    }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn observed_inventory_changes_on_append_and_title_changes() {
        let root = tempfile::tempdir().unwrap();
        let source = root.path().join("sessions");
        fs::create_dir(&source).unwrap();
        let log = source.join("one.jsonl");
        fs::write(&log, b"first\n").unwrap();
        let roots = vec![root.path().to_str().unwrap().to_owned()];
        let before = capture(&roots).unwrap().unwrap();
        fs::write(&log, b"first\nsecond\n").unwrap();
        let appended = capture(&roots).unwrap().unwrap();
        assert_ne!(before.fingerprint, appended.fingerprint);
        assert!(appended.bytes > before.bytes);
        fs::write(root.path().join("session_index.jsonl"), b"metadata\n").unwrap();
        assert_ne!(
            appended.fingerprint,
            capture(&roots).unwrap().unwrap().fingerprint
        );
    }
}
