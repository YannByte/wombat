//! Private immutable generations, validated shard reads and refresh locking.
use super::*;
pub(super) fn corrupt(message: impl Into<String>) -> anyhow::Error {
    operation_error("SNAPSHOT_CORRUPT", message)
}
pub(super) fn product_home() -> Result<PathBuf> {
    Ok(crate::storage::data_home()?.join("usage-v4"))
}
pub(super) fn private_dir(path: &Path) -> Result<()> {
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(path)?;
    Ok(())
}
pub struct RefreshLock {
    _file: fs::File,
}
impl RefreshLock {
    pub fn acquire() -> Result<Self> {
        Self::at(&product_home()?)
    }
    pub(super) fn at(root: &Path) -> Result<Self> {
        private_dir(root)?;
        let mut options = fs::OpenOptions::new();
        options.read(true).write(true).create(true).truncate(false);
        #[cfg(unix)]
        {
            use std::os::unix::fs::OpenOptionsExt;
            options.mode(0o600);
        }
        let file = options.open(root.join("refresh.lock"))?;
        file.try_lock()
            .map_err(|_| operation_error("UPDATE_BUSY", "用量正在更新"))?;
        Ok(Self { _file: file })
    }
}
pub(super) fn file_ref(file: &str, bytes: &[u8]) -> FileRef {
    FileRef {
        file: file.into(),
        sha256: crate::hash(bytes),
    }
}
#[cfg(test)]
pub(super) fn save_json<T: Serialize>(directory: &Path, name: &str, value: &T) -> Result<FileRef> {
    let bytes = serde_json::to_vec(value)?;
    if bytes.len() as u64 > MAX_FILE_BYTES {
        return Err(operation_error("RESOURCE_LIMIT", "快照分片超过512 MiB"));
    }
    crate::storage::atomic_write(&directory.join(name), &bytes)?;
    Ok(file_ref(name, &bytes))
}
fn save_pending_json<T: Serialize>(directory: &Path, name: &str, value: &T) -> Result<FileRef> {
    let bytes = serde_json::to_vec(value)?;
    if bytes.len() as u64 > MAX_FILE_BYTES {
        return Err(operation_error("RESOURCE_LIMIT", "快照分片超过512 MiB"));
    }
    crate::storage::write_unpublished(&directory.join(name), &bytes)?;
    Ok(file_ref(name, &bytes))
}
pub fn save(collected: Collected) -> Result<Snapshot> {
    save_with_prices(&product_home()?, collected, crate::pricing_sync::current()?)
}
#[cfg(test)]
pub(super) fn save_at(root: &Path, collected: Collected) -> Result<Snapshot> {
    save_with_prices(
        root,
        collected,
        crate::pricing_sync::current_at(&root.join("prices"))?,
    )
}
pub(super) fn save_with_prices(
    root: &Path,
    collected: Collected,
    prices: crate::pricing_sync::Response,
) -> Result<Snapshot> {
    validate_watermarks(&collected.watermarks)?;
    validate_title_observations(&collected.title_observations, &collected.threads)?;
    private_dir(root)?;
    let id = uuid::Uuid::new_v4().to_string();
    let generation = root.join("generations").join(&id);
    private_dir(&generation)?;
    let pending = tempfile::Builder::new()
        .prefix(".pending-")
        .tempdir_in(&generation)?;
    let directory = pending.path();
    let mut pool = super::price_pool::PricePool::new(&prices);
    let records: Vec<_> = collected
        .measurements
        .into_iter()
        .map(|fact| {
            let price = pool.price(&fact);
            PricedMeasurement { fact, price }
        })
        .collect();
    drop(pool);
    // Reject unusable totals before publishing any new latest pointer.
    crate::usage_app::summarize(&records.iter().collect::<Vec<_>>())?;
    let ledger = save_pending_json(directory, "ledger.json", &records)?;
    let events = super::events::save_events(directory, collected.events)?;
    let mut by_thread: BTreeMap<String, BTreeMap<String, TurnData>> = BTreeMap::new();
    for row in &records {
        if let Some(thread) = &row.fact.thread_id {
            by_thread
                .entry(thread.to_string())
                .or_default()
                .entry(
                    row.fact
                        .turn_id
                        .as_deref()
                        .unwrap_or("unassigned")
                        .to_owned(),
                )
                .or_default()
                .measurements
                .push(row.clone());
        }
    }
    let mut unassigned_uses = BTreeMap::<String, UnassignedUseRecords>::new();
    for op in collected.operations {
        if op.turn_id.as_deref().is_none_or(|id| id.is_empty()) {
            unassigned_uses
                .entry(op.thread_id.to_string())
                .or_default()
                .observe(&op)?;
        }
        by_thread
            .entry(op.thread_id.to_string())
            .or_default()
            .entry(op.turn_id.as_deref().unwrap_or("unassigned").to_owned())
            .or_default()
            .operations
            .push(op.as_ref().clone());
    }
    let turns: BTreeMap<_, _> = collected
        .turns
        .into_iter()
        .map(|t| ((t.thread_id.clone(), t.id.clone()), t))
        .collect();
    for (thread, turn) in turns.keys() {
        by_thread
            .entry(thread.to_string())
            .or_default()
            .entry(turn.clone())
            .or_default();
    }
    let mut threads = Vec::new();
    for thread in collected.threads {
        let filename = format!("thread-{}.jsonl", crate::hash(&thread.id));
        let mut bytes = Vec::new();
        let mut index = BTreeMap::new();
        for (turn_id, data) in by_thread.remove(&thread.id).unwrap_or_default() {
            let mut segment = serde_json::to_vec(&data)?;
            segment.push(b'\n');
            if (bytes.len() + segment.len()) as u64 > MAX_FILE_BYTES {
                return Err(operation_error("RESOURCE_LIMIT", "对话分片超过512 MiB"));
            }
            let slice = Slice {
                offset: bytes.len() as u64,
                length: segment.len() as u64,
                sha256: crate::hash(&segment),
            };
            bytes.extend(segment);
            index.insert(
                turn_id.clone(),
                TurnEntry {
                    turn: turns.get(&(thread.id.clone(), turn_id)).cloned(),
                    slice,
                },
            );
        }
        crate::storage::write_unpublished(&directory.join(&filename), &bytes)?;
        threads.push(Arc::new(ThreadEntry {
            unassigned_uses: unassigned_uses.remove(&thread.id).unwrap_or_default(),
            thread,
            file: file_ref(&filename, &bytes),
            turns: index,
        }));
    }
    if !by_thread.is_empty() {
        return Err(operation_error("INVALID_FACTS", "存在没有对话元数据的记录"));
    }
    let observation_versions = crate::observation_versions::SnapshotObservationVersions::current();
    let manifest = Manifest {
        schema_version: 4,
        observation_versions,
        title_observations: collected.title_observations,
        snapshot_ref: SnapshotRef {
            snapshot_id: id,
            created_at: chrono::Utc::now().to_rfc3339(),
        },
        price_revision: prices.catalog.revision,
        price_catalog_hash: prices.catalog_hash,
        sources: collected.sources,
        watermarks: collected.watermarks,
        issues: collected.issues,
        ledger,
        events,
        threads,
    };
    save_pending_json(directory, "manifest.json", &manifest)?;
    // Moving one completed directory publishes all files together. latest is independent of previous formats.
    let final_dir = generation.join("committed");
    #[cfg(unix)]
    fs::File::open(directory)?.sync_all()?;
    fs::rename(directory, &final_dir)?;
    #[cfg(unix)]
    fs::File::open(&generation)?.sync_all()?;
    crate::storage::atomic_write(
        &root.join("latest.json"),
        &serde_json::to_vec(&manifest.snapshot_ref)?,
    )?;
    Ok(Snapshot {
        lazy_details: None,
        summary_index: Default::default(),
        publication_change: None,
        project_loads: vec![],
        project_rows: None,
        timing_cache: Mutex::default(),
        query_cache: Mutex::default(),
        query_work: Arc::default(),
        manifest,
        directory: final_dir,
        memory_turns: None,
        live_rows: None,
        live_events: None,
        cold_events: None,
    })
}
pub(super) fn bounded_read(path: &Path) -> Result<Vec<u8>> {
    let file = fs::File::open(path).map_err(|e| corrupt(format!("快照文件无法读取：{e}")))?;
    if file.metadata()?.len() > MAX_FILE_BYTES {
        return Err(operation_error("RESOURCE_LIMIT", "快照文件超过512 MiB"));
    }
    let mut bytes = Vec::new();
    file.take(MAX_FILE_BYTES + 1).read_to_end(&mut bytes)?;
    if bytes.len() as u64 > MAX_FILE_BYTES {
        return Err(operation_error("RESOURCE_LIMIT", "快照文件超过512 MiB"));
    }
    Ok(bytes)
}
pub(super) fn safe_file(directory: &Path, name: &str) -> Result<PathBuf> {
    if name.is_empty() || Path::new(name).components().count() != 1 || name == "." || name == ".." {
        return Err(corrupt("非法快照分片路径"));
    }
    let path = directory.join(name);
    if fs::symlink_metadata(&path)?.file_type().is_symlink() {
        return Err(corrupt("快照分片不能为符号链接"));
    }
    Ok(path)
}
pub fn load(id: Option<&str>) -> Result<Snapshot> {
    load_at(&product_home()?, id)
}
pub(super) fn load_at(root: &Path, id: Option<&str>) -> Result<Snapshot> {
    let id = match id {
        Some(id) => id.to_owned(),
        None => {
            if !root.join("latest.json").is_file() {
                return Err(operation_error("NO_SNAPSHOT", "尚无用量数据，请先更新用量"));
            }
            let reference: SnapshotRef =
                serde_json::from_slice(&bounded_read(&root.join("latest.json"))?)
                    .map_err(|_| corrupt("最近快照指针损坏"))?;
            reference.snapshot_id
        }
    };
    if id.is_empty() || id.len() > 128 || !id.chars().all(|c| c.is_ascii_alphanumeric() || c == '-')
    {
        return Err(operation_error("INVALID_ARGUMENT", "非法快照身份"));
    }
    let directory = root.join("generations").join(&id).join("committed");
    if !directory.join("manifest.json").is_file() {
        if root.file_name().is_some_and(|name| name == "usage-v4")
            && let Some(home) = root.parent()
            && ["usage-v1", "usage-v2", "usage-v3"].iter().any(|version| {
                home.join(version)
                    .join("generations")
                    .join(&id)
                    .join("committed/manifest.json")
                    .is_file()
            })
        {
            return Err(operation_error("UNSUPPORTED_VERSION", "不支持此快照版本"));
        }
        return Err(operation_error("NO_SNAPSHOT", "未找到已提交快照"));
    }
    let raw: serde_json::Value =
        serde_json::from_slice(&bounded_read(&directory.join("manifest.json"))?)
            .map_err(|_| corrupt("快照索引损坏"))?;
    if raw["schemaVersion"] != 4 {
        return Err(operation_error("UNSUPPORTED_VERSION", "不支持此快照版本"));
    }
    crate::observation_versions::ObservationHeaderSet::Snapshot.validate_json(&raw, |kind| {
        use crate::observation_versions::ObservationKind;
        match kind {
            ObservationKind::Event | ObservationKind::Title => "不支持此快照来源观察格式",
            ObservationKind::Message => "不支持此消息观察映射",
            ObservationKind::Operation => "不支持此操作结果观察映射",
            ObservationKind::Association => "不支持此操作关联方法",
            ObservationKind::Measurement => "不支持此计量观察映射",
            ObservationKind::Work => "不支持此快照来源观察格式",
        }
    })?;
    crate::session_events::title_observations::check_rows(&raw)?;
    super::events::check_index_version(raw["events"]["version"].as_u64())?;
    super::native_boundary::check_versions(&raw["events"])?;
    super::use_metadata::check_headers(&raw)?;
    if raw
        .get("watermarks")
        .and_then(serde_json::Value::as_array)
        .is_some_and(|rows| {
            rows.iter().any(|row| {
                row.get("formatVersion")
                    .and_then(serde_json::Value::as_u64)
                    .is_some_and(|v| v != u64::from(WATERMARK_FORMAT_VERSION))
            })
        })
    {
        return Err(operation_error(
            "UNSUPPORTED_VERSION",
            "不支持此来源水位版本",
        ));
    }
    let manifest: Manifest =
        serde_json::from_value(raw).map_err(|e| corrupt(format!("快照索引无效：{e}")))?;
    validate_watermarks(&manifest.watermarks).map_err(|e| corrupt(format!("快照水位无效：{e}")))?;
    validate_title_observations(
        &manifest.title_observations,
        manifest.threads.iter().map(|t| &t.thread),
    )
    .map_err(|e| corrupt(format!("标题观察无效：{e}")))?;
    super::events::validate_index(&manifest.events)?;
    if manifest.snapshot_ref.snapshot_id != id {
        return Err(corrupt("快照身份不匹配"));
    }
    Ok(Snapshot {
        lazy_details: None,
        summary_index: Default::default(),
        publication_change: None,
        project_loads: vec![],
        project_rows: None,
        timing_cache: Mutex::default(),
        query_cache: Mutex::default(),
        query_work: Arc::default(),
        manifest,
        directory,
        memory_turns: None,
        live_rows: None,
        live_events: None,
        cold_events: None,
    })
}
