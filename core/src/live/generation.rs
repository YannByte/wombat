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
pub(super) fn rebuild(
    root: &Path,
    key: &str,
    roots: &[String],
) -> Result<(rusqlite::Connection, Arc<Snapshot>)> {
    let stage = tempfile::Builder::new()
        .prefix("index-generation-")
        .tempdir_in(root)?;
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
