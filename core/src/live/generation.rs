//! Explicit rebuilds publish a new database only after its complete durable scan.
//! User decisions, grants, identities and handling records are outside this store.
use super::*;
use std::path::Path;

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Pointer {
    version: u32,
    directory: String,
}

pub(super) fn index_path(root: &Path) -> Result<PathBuf> {
    let raw = match fs::read(root.join("active-index.json")) {
        Ok(raw) => raw,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(root.join("index.sqlite")),
        Err(e) => return Err(e.into()),
    };
    let value: Value = serde_json::from_slice(&raw)?;
    if value.get("version").and_then(Value::as_u64) != Some(1) {
        return Err(operation_error(
            "UNSUPPORTED_VERSION",
            "Unsupported index generation pointer",
        ));
    }
    let pointer: Pointer = serde_json::from_value(value)?;
    if !pointer.directory.starts_with("index-generation-")
        || !pointer
            .directory
            .bytes()
            .all(|c| c.is_ascii_alphanumeric() || c == b'-')
    {
        return Err(operation_error(
            "INDEX_UNAVAILABLE",
            "Unsafe index generation",
        ));
    }
    let directory = root.join(pointer.directory);
    if fs::symlink_metadata(&directory)?.file_type().is_symlink() {
        return Err(operation_error(
            "INDEX_UNAVAILABLE",
            "Unsafe index generation",
        ));
    }
    Ok(directory.join("index.sqlite"))
}
const PENDING: &str = ".wombat-pending-index-v1";
const COMPLETE: &str = ".wombat-complete-index-v1";
const OWNERSHIP: &[u8] = b"wombat-owned-pending-index-v1\n";

/// Called only while the foreground writer lock is held. Unknown directories and
/// complete generations remain untouched; no source or user-record path is visited.
pub(super) fn cleanup_pending(root: &Path, request_id: Option<&str>) -> Result<usize> {
    let paths = pending_paths(root, request_id)?;
    for path in &paths {
        fs::remove_dir_all(path)?;
    }
    Ok(paths.len())
}
pub(super) fn has_pending(root: &Path, request_id: &str) -> Result<bool> {
    Ok(!pending_paths(root, Some(request_id))?.is_empty())
}
fn pending_paths(root: &Path, request_id: Option<&str>) -> Result<Vec<PathBuf>> {
    if let Some(id) = request_id {
        validate_request_id(id)?;
    }
    let mut paths = Vec::new();
    for entry in fs::read_dir(root)? {
        let entry = entry?;
        let name = entry.file_name();
        let Some(name) = name.to_str() else { continue };
        let Some(rest) = name.strip_prefix("index-generation-") else {
            continue;
        };
        let Some(id) = rest.get(..36) else { continue };
        if validate_request_id(id).is_err()
            || !rest[36..].starts_with('-')
            || request_id.is_some_and(|expected| expected != id)
        {
            continue;
        }
        let path = entry.path();
        let meta = fs::symlink_metadata(&path)?;
        if !meta.is_dir() || meta.file_type().is_symlink() || path.join(COMPLETE).try_exists()? {
            continue;
        }
        let marker = path.join(PENDING);
        let Ok(meta) = fs::symlink_metadata(&marker) else {
            continue;
        };
        if !meta.is_file()
            || meta.file_type().is_symlink()
            || meta.len() != OWNERSHIP.len() as u64
            || fs::read(marker)? != OWNERSHIP
        {
            continue;
        }
        paths.push(path);
    }
    Ok(paths)
}
fn validate_request_id(id: &str) -> Result<()> {
    if uuid::Uuid::parse_str(id)
        .ok()
        .is_none_or(|value| value.to_string() != id)
    {
        return Err(operation_error(
            "INVALID_ARGUMENT",
            "Invalid foreground request identity",
        ));
    }
    Ok(())
}
pub(super) fn rebuild(
    root: &Path,
    key: &str,
    roots: &[String],
) -> Result<(rusqlite::Connection, Arc<Snapshot>)> {
    let request_id =
        std::env::var("WOMBAT_REQUEST_ID").unwrap_or_else(|_| uuid::Uuid::new_v4().to_string());
    validate_request_id(&request_id)?;
    let stage = tempfile::Builder::new()
        .prefix(&format!("index-generation-{request_id}-"))
        .tempdir_in(root)?;
    fs::write(stage.path().join(PENDING), OWNERSHIP)?;
    let path = stage.path().join("index.sqlite");
    let mut db = crate::live_index::open(&path)?;
    let view = sync(&mut db, key, roots, false, None, &mut BTreeMap::new())?.ok_or_else(|| {
        operation_error("INDEX_UNAVAILABLE", "Rebuild produced no committed view")
    })?;
    // The read view pins this transaction. SQLite's FULL checkpoint syncs the
    // committed WAL; publication never relies on renaming an open database.
    db.execute_batch("PRAGMA wal_checkpoint(FULL)")?;
    // Windows requires write access to flush an existing file. Opening without
    // create/truncate keeps this completed generation intact before publication.
    fs::OpenOptions::new().write(true).open(&path)?.sync_all()?;
    let name = stage
        .path()
        .file_name()
        .and_then(|n| n.to_str())
        .ok_or_else(|| operation_error("INDEX_UNAVAILABLE", "Invalid generation directory"))?
        .to_owned();
    // A complete generation is recovery material even if the caller stops just
    // before pointer publication. Pending cleanup must never remove it.
    fs::write(stage.path().join(COMPLETE), b"complete\n")?;
    crate::storage::atomic_write(
        &root.join("active-index.json"),
        &serde_json::to_vec(&Pointer {
            version: 1,
            directory: name,
        })?,
    )?;
    let _published = stage.keep();
    Ok((db, view))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rebuild_failure_preserves_original_and_success_switches_only_derived_storage() {
        let storage = tempfile::tempdir().unwrap();
        let root = storage.path().join("live-v2");
        fs::create_dir(&root).unwrap();
        let source = tempfile::tempdir().unwrap();
        fs::create_dir(source.path().join("sessions")).unwrap();
        fs::write(
            source.path().join("sessions/a.jsonl"),
            "{\"type\":\"session_meta\",\"payload\":{\"id\":\"a\"}}\n",
        )
        .unwrap();
        fs::write(root.join("index.sqlite"), "unsupported-original").unwrap();
        let decisions = storage.path().join("user-v1");
        fs::create_dir(&decisions).unwrap();
        fs::write(decisions.join("decisions.json"), "user decisions").unwrap();
        let roots = vec![source.path().join("missing").to_string_lossy().into_owned()];
        assert!(rebuild(&root, &source_key(&roots), &roots).is_err());
        assert!(!root.join("active-index.json").exists());
        assert_eq!(
            fs::read_to_string(root.join("index.sqlite")).unwrap(),
            "unsupported-original"
        );
        assert_eq!(fs::read_dir(&root).unwrap().count(), 1);
        let roots = vec![source.path().to_string_lossy().into_owned()];
        let key = source_key(&roots);
        let (db, view) = rebuild(&root, &key, &roots).unwrap();
        assert_ne!(index_path(&root).unwrap(), root.join("index.sqlite"));
        assert_eq!(
            dunce::canonicalize(db.path().unwrap()).unwrap(),
            dunce::canonicalize(index_path(&root).unwrap()).unwrap()
        );
        assert!(
            !view
                .manifest
                .sources
                .iter()
                .any(|s| s.issues.iter().any(|i| i.code == "historyDeferred"))
        );
        assert_eq!(
            fs::read_to_string(decisions.join("decisions.json")).unwrap(),
            "user decisions"
        );
        assert_eq!(
            fs::read_to_string(root.join("index.sqlite")).unwrap(),
            "unsupported-original"
        );
    }
    #[test]
    fn unknown_or_escaping_generation_pointers_are_rejected() {
        let storage = tempfile::tempdir().unwrap();
        for value in [
            json!({"version":99,"directory":"index-generation-future"}),
            json!({"version":1,"directory":"index-generation-../../user-v1"}),
        ] {
            fs::write(storage.path().join("active-index.json"), value.to_string()).unwrap();
            assert!(index_path(storage.path()).is_err());
        }
    }
}

#[cfg(test)]
mod cleanup_tests {
    use super::*;
    #[test]
    fn cleanup_only_removes_owned_unpublished_stages_and_rejects_foreign_ids() {
        let root = tempfile::tempdir().unwrap();
        let one = uuid::Uuid::new_v4().to_string();
        let two = uuid::Uuid::new_v4().to_string();
        let stage = root.path().join(format!("index-generation-{one}-pending"));
        fs::create_dir(&stage).unwrap();
        fs::write(stage.join(PENDING), OWNERSHIP).unwrap();
        fs::write(stage.join("index.sqlite"), "partial").unwrap();
        let complete = root.path().join(format!("index-generation-{one}-complete"));
        fs::create_dir(&complete).unwrap();
        fs::write(complete.join(PENDING), OWNERSHIP).unwrap();
        fs::write(complete.join(COMPLETE), "complete").unwrap();
        let unknown = root.path().join(format!("index-generation-{one}-foreign"));
        fs::create_dir(&unknown).unwrap();
        fs::write(unknown.join(PENDING), "unknown format").unwrap();
        assert_eq!(cleanup_pending(root.path(), Some(&two)).unwrap(), 0);
        assert!(stage.exists());
        assert!(cleanup_pending(root.path(), Some("../../sources")).is_err());
        assert_eq!(cleanup_pending(root.path(), Some(&one)).unwrap(), 1);
        assert!(!stage.exists());
        assert!(complete.exists());
        assert!(unknown.exists());
    }
}
