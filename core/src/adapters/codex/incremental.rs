//! Durable parser checkpoints contain allowlisted facts only, never event bodies.
use super::*;
use anyhow::Result;
use rusqlite::Connection;

use crate::file_metadata::FileMetadata;
use std::time::UNIX_EPOCH;
mod external;
mod recent;
#[cfg(test)]
mod reduction;
#[cfg(test)]
mod reduction_tests;
mod resume;

/// Mapping 3 adds bounded original-text locators; damaged-item boundaries remain.
pub(crate) const MESSAGE_OBSERVATION_VERSION: u32 = 3;
/// Canonical operation outcome mapping, including persistent conflict evidence.
pub(crate) const OPERATION_OBSERVATION_VERSION: u32 = 6;
/// Measurement context conflict markers require explicit source observation headers.
pub(crate) const MEASUREMENT_OBSERVATION_VERSION: u32 = 2;
pub(crate) use crate::adapters::contract::WORK_OBSERVATION_VERSION;

#[derive(Default, Serialize, Deserialize)]
pub(super) struct Checkpoint {
    pub offset: u64,
    pub line: u64,
    pub state: State,
    stamp: String,
    physical: String,
    prefix_sha256: String,
    issues: Vec<Issue>,
    watermark: Option<SourceWatermark>,
}

pub(super) fn physical_identity(meta: &FileMetadata) -> String {
    meta.physical.clone()
}

fn stamp(meta: &FileMetadata) -> String {
    let physical = physical_identity(meta);
    format!(
        "{physical}:{}:{}",
        meta.len(),
        meta.modified()
            .ok()
            .and_then(|v| v.duration_since(UNIX_EPOCH).ok())
            .map_or(0, |d| d.as_nanos())
    )
}
struct PrefixChecksums {
    committed: String,
    captured: String,
}

fn source_changed() -> anyhow::Error {
    crate::dto::operation_error("SOURCE_CHANGED", "日志读取中发生变化，保留已提交数据并重试")
}

/// Appends beyond the captured prefix belong to the next synchronization. The
/// captured prefix is hashed before and after parsing to detect edits within it.
pub(super) fn contains_capture(expected: &FileMetadata, current: &FileMetadata) -> bool {
    physical_identity(expected) == physical_identity(current)
        && current.len() >= expected.len()
        && (current.len() > expected.len() || stamp(expected) == stamp(current))
}

/// Hash only the captured prefix, even if the writer appends while hashing.
fn prefix_checksums(path: &Path, offset: u64, expected: &FileMetadata) -> Result<PrefixChecksums> {
    let mut file = File::open(path)?;
    if !contains_capture(expected, &FileMetadata::from_file(&file)?) {
        return Err(source_changed());
    }
    let result = prefix_checksums_from(path, offset, expected, &mut file)?;
    if !contains_capture(expected, &FileMetadata::from_file(&file)?) {
        return Err(source_changed());
    }
    Ok(result)
}

fn prefix_checksums_from(
    path: &Path,
    offset: u64,
    expected: &FileMetadata,
    reader: &mut impl Read,
) -> Result<PrefixChecksums> {
    if offset > expected.len() || !contains_capture(expected, &FileMetadata::read(path)?) {
        return Err(source_changed());
    }
    let mut digest = Sha256::new();
    let mut committed = (offset == 0).then(|| format!("{:x}", digest.clone().finalize()));
    let mut consumed = 0;
    let mut bytes = [0_u8; 64 * 1024];
    while consumed < expected.len() {
        let mut count = (expected.len() - consumed).min(bytes.len() as u64);
        if consumed < offset {
            count = count.min(offset - consumed);
        }
        reader.read_exact(&mut bytes[..count as usize])?;
        digest.update(&bytes[..count as usize]);
        consumed += count;
        if consumed == offset {
            committed = Some(format!("{:x}", digest.clone().finalize()));
        }
    }
    if !contains_capture(expected, &FileMetadata::read(path)?) {
        return Err(source_changed());
    }
    Ok(PrefixChecksums {
        committed: committed.expect("validated offset lies inside the captured file"),
        captured: format!("{:x}", digest.finalize()),
    })
}

#[derive(Default)]
pub(crate) struct Cache {
    pub(crate) encoded_bytes: u64,
}
impl Cache {
    pub(crate) fn refresh_encoded_bytes(&mut self, _db: &Connection, _source: &str) -> Result<()> {
        // Recovery bodies live in verified blocks and a file-backed work relation.
        self.encoded_bytes = 0;
        Ok(())
    }
}

pub(crate) fn fact_scope(source: &SourceInstance) -> String {
    format!("parser:{}:{VERSION}:1:facts", source.id)
}

#[cfg(test)]
pub(crate) fn sync(
    db: &Connection,
    source: &SourceInstance,
    verify: bool,
) -> Result<Option<Collected>> {
    sync_cached(db, source, verify, &mut Cache::default())?
        .map(|value| expand_test_events(db, source, value).map(|v| v.collected))
        .transpose()
}

#[cfg(test)]
pub(crate) fn expand_test_events(
    db: &Connection,
    source: &SourceInstance,
    mut value: Synced,
) -> Result<Synced> {
    let mut events = BTreeMap::new();
    crate::live_index::each_event(db, &fact_scope(source), |e| {
        events.insert(e.shared_id(), e);
        Ok(())
    })?;
    value.collected.events = events.into_values().collect();
    Ok(value)
}

#[cfg(test)]
fn replay_events(
    events: BTreeMap<Arc<str>, Arc<crate::session_events::Event>>,
    report: &SourceReport,
    cancelled: &std::sync::atomic::AtomicBool,
) -> Result<Facts> {
    crate::operation_association::check(cancelled)?;
    let mut facts = Facts::default();
    // Parse diagnostics remain owned by file checkpoints; replay only restores facts.
    let mut replay_report = report.clone();
    replay_report.issues.clear();
    let mut ordered = Vec::new();
    for event in events.values() {
        crate::operation_association::check(cancelled)?;
        ordered.push(event);
    }
    crate::operation_association::check(cancelled)?;
    ordered.sort_by(|a, b| event_projection::order(a).cmp(&event_projection::order(b)));
    crate::operation_association::check(cancelled)?;
    for event in ordered {
        crate::operation_association::check(cancelled)?;
        event_projection::apply(&mut facts, event, &mut replay_report);
    }
    facts.events = events;
    // Canonical operations must exist before allocation reuse or derived emission.
    // The caller repeats this pass only after additional observations are collected.
    facts.resolve_operations(&mut replay_report, cancelled)?;
    Ok(facts)
}

pub(crate) struct Synced {
    pub collected: Collected,
}
pub(crate) fn sync_cached(
    db: &Connection,
    source: &SourceInstance,
    verify: bool,
    cache: &mut Cache,
) -> Result<Option<Synced>> {
    let context = RunContext::default();
    let result = sync_cached_with_context(db, source, verify, cache, &context)?;
    #[cfg(test)]
    let result = expand_test_projection(db, source, result)?;
    Ok(result)
}
#[cfg(test)]
fn expand_test_projection(
    db: &Connection,
    source: &SourceInstance,
    result: Option<Synced>,
) -> Result<Option<Synced>> {
    let result = result
        .map(|mut result| {
            for field in ["measurements", "operations"] {
                crate::live_index::each_located(
                    db,
                    &format!("projection:{}", source.id),
                    Some(field),
                    |_, _, payload, _| {
                        if field == "measurements" {
                            result
                                .collected
                                .measurements
                                .push(serde_json::from_str(payload)?);
                        } else {
                            result
                                .collected
                                .operations
                                .push(serde_json::from_str(payload)?);
                        }
                        Ok(())
                    },
                )?;
            }
            result
                .collected
                .measurements
                .sort_unstable_by(|a, b| a.id.cmp(&b.id));
            result
                .collected
                .operations
                .sort_unstable_by(|a, b| a.id.cmp(&b.id));
            Ok::<_, anyhow::Error>(result)
        })
        .transpose()?;
    Ok(result)
}
pub(crate) fn sync_cached_recent(
    db: &Connection,
    source: &SourceInstance,
    verify: bool,
    cache: &mut Cache,
) -> Result<Option<Synced>> {
    let result = sync_with_policy(db, source, verify, cache, &RunContext::default(), true)?;
    #[cfg(test)]
    let result = expand_test_projection(db, source, result)?;
    Ok(result)
}
fn sync_cached_with_context(
    db: &Connection,
    source: &SourceInstance,
    verify: bool,
    cache: &mut Cache,
    context: &RunContext,
) -> Result<Option<Synced>> {
    sync_with_policy(db, source, verify, cache, context, false)
}
fn sync_with_policy(
    db: &Connection,
    source: &SourceInstance,
    verify: bool,
    cache: &mut Cache,
    context: &RunContext,
    recent_only: bool,
) -> Result<Option<Synced>> {
    crate::operation_association::check(&context.cancelled)?;
    let mut report = SourceReport {
        source: source.clone(),
        adapter_version: VERSION.into(),
        source_versions: vec![],
        capabilities: CodexAdapter.descriptor().capabilities,
        status: "complete".into(),
        files_read: 0,
        bytes_read: 0,
        issues: vec![],
    };
    let root = Path::new(&source.root);
    let mut files = BTreeSet::new();
    let mut visited = BTreeSet::new();
    for directory in [root.join("sessions"), root.join("archived_sessions")] {
        list_files(
            &directory,
            0,
            &mut files,
            &mut visited,
            context,
            &mut report,
        );
    }
    let scope = format!("parser:{}:{VERSION}:1", source.id);
    if crate::live_index::has_scope(db, &scope)? {
        // Reject an unavailable mapping before materializing any future parser
        // payload, whose shape may be beyond this version's JSON decoding limits.
        crate::observation_versions::ObservationHeaderSet::Parser.validate_index(
            |field| crate::live_index::scalar(db, &scope, field),
            |kind| match kind {
                crate::observation_versions::ObservationKind::Operation => {
                    "不支持此操作结果观察映射"
                }
                crate::observation_versions::ObservationKind::Association => "不支持此操作关联方法",
                crate::observation_versions::ObservationKind::Event => "不支持此事件观察格式",
                crate::observation_versions::ObservationKind::Title => "不支持此标题观察格式",
                crate::observation_versions::ObservationKind::Work => "不支持此来源工作观察映射",
                crate::observation_versions::ObservationKind::Message => "不支持此来源消息观察映射",
                crate::observation_versions::ObservationKind::Measurement => {
                    "不支持此来源计量观察映射"
                }
            },
        )?;
        if crate::live_index::scalar(db, &scope, "watermarkVersion")?
            .and_then(|value| value.as_u64())
            != Some(u64::from(WATERMARK_FORMAT_VERSION))
        {
            return Err(crate::dto::operation_error(
                "UNSUPPORTED_VERSION",
                "不支持此来源水位格式",
            ));
        }
    }
    let previous = crate::live_index::load_map(db, &scope)?;
    resume::validate(db, source)?;
    let mut checkpoints: BTreeMap<String, Checkpoint> = previous
        .get("checkpoints")
        .map(|v| serde_json::from_value(v.clone()))
        .transpose()?
        .unwrap_or_default();
    for (path, cp) in &checkpoints {
        let watermark = cp
            .watermark
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("missing checkpoint watermark"))?;
        watermark.validate()?;
        anyhow::ensure!(
            watermark.source_instance_id == source.id
                && watermark.file_id == crate::hash(path.as_bytes())
                && watermark.committed_offset == cp.offset
                && watermark.generation == cp.state.event_generation,
            "checkpoint watermark identity or position mismatch"
        );
    }
    let observed_at = chrono::Utc::now().to_rfc3339();
    let eligible = if recent_only && !verify {
        recent::select(&files, &checkpoints)?
    } else {
        files.clone()
    };
    let deferred: Vec<_> = files
        .difference(&eligible)
        .map(|p| p.to_string_lossy().into_owned())
        .collect();
    if !deferred.is_empty() {
        issue(
            &mut report,
            "historyDeferred",
            "Historical files are not indexed yet; results have partial source coverage and can change when history is loaded",
            None,
        );
    }
    let title_stamp = FileMetadata::read(root.join("session_index.jsonl"))
        .ok()
        .map(|m| stamp(&m));
    let mut changed =
        previous.is_empty() || previous.get("titleStamp") != Some(&serde_json::json!(title_stamp));
    let mut rebuild = verify;
    let mut dirty = BTreeSet::new();
    let mut replaced = BTreeSet::new();
    let mut observations = BTreeMap::new();
    let mut unchanged = BTreeMap::new();
    for file in &files {
        if !eligible.contains(file) {
            continue;
        }
        let key = file.to_string_lossy().into_owned();
        let meta = FileMetadata::read(file)?;
        let cp = checkpoints.get(&key);
        if let Some(cp) = cp
            && cp.stamp == stamp(&meta)
            && !verify
            && cp
                .watermark
                .as_ref()
                .is_some_and(|w| w.state != WatermarkState::Missing)
        {
            unchanged.insert(key, (meta, cp.offset, cp.prefix_sha256.clone()));
            continue;
        }
        let checksums =
            prefix_checksums(file, cp.map_or(0, |cp| cp.offset.min(meta.len())), &meta)?;
        if let Some(cp) = cp
            && (physical_identity(&meta) != cp.physical
                || meta.len() < cp.offset
                || checksums.committed != cp.prefix_sha256)
        {
            rebuild = true;
            replaced.insert(key.clone());
        }
        observations.insert(key.clone(), (meta, checksums.captured));
        dirty.insert(key);
    }
    let missing: BTreeSet<_> = checkpoints
        .keys()
        .filter(|p| !files.contains(Path::new(p)))
        .cloned()
        .collect();
    // Removing a log is not evidence that its historical consumption was zero.
    for path in &missing {
        let cp = checkpoints
            .get_mut(path)
            .expect("missing checkpoint exists");
        if let Some(w) = &cp.watermark
            && w.state != WatermarkState::Missing
        {
            cp.watermark = Some(w.unavailable(
                WatermarkState::Missing,
                WatermarkIssue::SourceMissing,
                &observed_at,
            ));
        }
        issue(
            &mut report,
            "sourceMissing",
            "已索引的日志不可用，保留已观察用量",
            Some(EvidenceRef {
                file: path.as_str().into(),
                line: 0,
            }),
        );
    }
    changed |= !dirty.is_empty()
        || rebuild
        || previous.get("deferredFiles") != Some(&serde_json::json!(deferred))
        || previous.get("missing") != Some(&serde_json::json!(missing));
    if !changed {
        return Ok(None);
    }
    let fact_scope = format!("{scope}:facts");
    external::prepare(db)?;
    let mut facts = if rebuild || previous.is_empty() {
        Facts {
            streaming: true,
            external: true,
            title_observations: if previous.is_empty() {
                BTreeMap::new()
            } else {
                resume::load(db, source)?
                    .ok_or_else(|| anyhow::anyhow!("Parser recovery directory missing"))?
                    .title_observations
            },
            ..Facts::default()
        }
    } else {
        let facts = resume::load(db, source)?
            .ok_or_else(|| anyhow::anyhow!("Parser recovery directory missing"))?;
        external::restore(db, source, &context.cancelled)?;
        facts
    };
    let generations: BTreeMap<_, _> = checkpoints
        .iter()
        .filter_map(|(path, cp)| {
            cp.state
                .event_generation
                .clone()
                .map(|generation| (path.clone(), generation))
        })
        .collect();
    let pending_generations: BTreeMap<_, _> = checkpoints
        .iter()
        .filter_map(|(path, cp)| {
            cp.state
                .pending_event_generation
                .clone()
                .map(|generation| (path.clone(), generation))
        })
        .collect();
    if rebuild {
        external::rebuild_archive(db, &fact_scope, source, &replaced, &context.cancelled)?;
        checkpoints.retain(|p, _| missing.contains(p));
        dirty = files
            .iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect();
    } else if previous.is_empty() {
        crate::live_index::save_events(db, &fact_scope, [], true, &context.cancelled)?;
    }
    facts.full_event_write = rebuild;
    for path in dirty {
        let checkpoint = checkpoints.entry(path.clone()).or_default();
        if checkpoint.state.event_generation.is_none() {
            if replaced.contains(&path) {
                checkpoint.state.pending_event_generation = Some(uuid::Uuid::new_v4().to_string());
            } else {
                checkpoint.state.event_generation = generations.get(&path).cloned();
                checkpoint.state.pending_event_generation = pending_generations.get(&path).cloned();
            }
        }
        let (before, captured) = if let Some((observed, captured)) = observations.get(&path) {
            (observed.clone(), captured.clone())
        } else {
            let (before, offset, expected_prefix) =
                if let Some((observed, offset, prefix)) = unchanged.get(&path) {
                    (observed.clone(), *offset, Some(prefix))
                } else {
                    (FileMetadata::read(&path)?, 0, None)
                };
            let checksums = prefix_checksums(Path::new(&path), offset, &before)?;
            if expected_prefix.is_some_and(|prefix| *prefix != checksums.committed) {
                return Err(source_changed());
            }
            (before, checksums.captured)
        };
        checkpoint.issues.clear();
        let first = report.issues.len();
        read_file_flushing(
            Path::new(&path),
            source,
            context,
            &mut facts,
            &mut report,
            Some(checkpoint),
            Some(&before),
            |facts, file_complete| {
                if facts.streaming && (facts.pending_events.len() >= 1024 || file_complete) {
                    external::flush(db, &fact_scope, facts, &context.cancelled)?;
                }
                Ok(())
            },
        )?;
        let after = FileMetadata::read(&path)?;
        if !contains_capture(&before, &after)
            || report.issues[first..].iter().any(|i| {
                matches!(
                    i.code.as_str(),
                    "sourceChanged" | "sourceUnreadable" | "resourceLimit"
                )
            })
        {
            return Err(source_changed());
        }
        let checksums = prefix_checksums(Path::new(&path), checkpoint.offset, &before)?;
        if checksums.captured != captured {
            return Err(source_changed());
        }
        checkpoint.stamp = stamp(&before);
        checkpoint.physical = physical_identity(&before);
        checkpoint.prefix_sha256 = checksums.committed;
        checkpoint.issues = report.issues.split_off(first);
        let watermark = facts
            .watermarks
            .get(&crate::hash(path.as_bytes()))
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("missing read watermark"))?;
        anyhow::ensure!(
            watermark.committed_offset == checkpoint.offset,
            "cursor and watermark differ"
        );
        watermark.validate()?;
        checkpoint.watermark = Some(watermark);
    }
    external::refresh(
        db,
        &fact_scope,
        &mut facts,
        &mut report,
        rebuild || previous.is_empty(),
        rebuild,
        &context.cancelled,
    )?;
    external::resolve_operations(db, &mut report, &context.cancelled)?;
    read_titles(root, &mut facts, &mut report);
    titles::apply_titles(&mut facts);
    if let Some(versions) = previous.get("sourceVersions") {
        for v in serde_json::from_value::<Vec<String>>(versions.clone())? {
            if !report.source_versions.contains(&v) {
                report.source_versions.push(v);
            }
        }
    }
    facts.watermarks.clear();
    resume::save(db, source, &facts)?;
    // Operation association issues precede checkpoint diagnostics, as in source
    // replay. Reconciliation then streams scalar ownership and coverage keys.
    let mut measurement_issues = external::measurement_issues(db)?;
    for (path, cp) in &checkpoints {
        let mut diagnostics = cp.issues.clone();
        diagnostics.extend(measurement_issues.remove(path).unwrap_or_default());
        diagnostics.sort_by_key(|i| i.evidence.as_ref().map_or(0, |e| e.line));
        report.issues.extend(diagnostics);
    }
    report
        .issues
        .extend(measurement_issues.into_values().flatten());
    external::project(db, &mut facts, &mut report, &context.cancelled)?;
    external::save(db, source)?;
    let mut result = Collected::default();
    for checkpoint in checkpoints.values() {
        crate::operation_association::check(&context.cancelled)?;
        if let Some(watermark) = &checkpoint.watermark {
            result.watermarks.push(watermark.clone());
        }
    }
    facts.watermarks.clear();
    titles::apply_titles(&mut facts);
    let derived = projection::Projection {
        metadata: facts,
        measurements: vec![],
        operations: vec![],
        events: vec![],
    };
    emit_facts(derived, &mut result, &context.cancelled)?;
    crate::live_index::replace_field(
        db,
        &fact_scope,
        "title_observations",
        result
            .title_observations
            .iter()
            .map(|value| (value.thread_id.as_str(), value)),
    )?;
    // Parser facts are reconstructed from authoritative observations. Query facts
    // are stored once in their canonical projection, never copied into this scope.
    let mut metadata = serde_json::json!({
        "watermarkVersion": WATERMARK_FORMAT_VERSION,
        "checkpoints": checkpoints,
        "missing": missing,
        "titleStamp": title_stamp,
        "sourceVersions": report.source_versions
    });
    metadata["deferredFiles"] = serde_json::json!(deferred);
    crate::observation_versions::ObservationHeaderSet::Parser
        .write_json(metadata.as_object_mut().expect("metadata is an object"));
    crate::live_index::save_map(db, &scope, metadata.as_object().unwrap())?;
    external::clear(db)?;
    cache.encoded_bytes = 0;
    if !root.exists() && checkpoints.is_empty() {
        report.status = "notFound".into();
    } else if !report.issues.is_empty() {
        report.status = if report.files_read == 0
            && facts_empty_marker(&result)
            && report
                .issues
                .iter()
                .any(|i| matches!(i.code.as_str(), "invalidRecord" | "sourceUnreadable"))
        {
            "failed"
        } else {
            "partial"
        }
        .into();
    }
    result.issues.extend(report.issues.iter().cloned());
    result.sources.push(report);
    Ok(Some(Synced { collected: result }))
}

fn facts_empty_marker(result: &Collected) -> bool {
    result.measurements.is_empty() && result.threads.is_empty()
}

#[cfg(test)]
mod sharing_tests {
    use super::*;
    use std::io::Write;
    #[test]
    fn cancelled_incremental_context_does_not_write_or_replace_the_cached_view() {
        let root = tempfile::tempdir().unwrap();
        let index = tempfile::tempdir().unwrap();
        let dir = root.path().join("sessions");
        fs::create_dir(&dir).unwrap();
        let path = dir.join("a.jsonl");
        fs::write(
            &path,
            "{\"type\":\"session_meta\",\"payload\":{\"id\":\"t\"}}\n",
        )
        .unwrap();
        let source = CodexAdapter
            .discover(&DiscoveryRequest {
                roots: vec![root.path().into()],
            })
            .sources
            .remove(0);
        let db = crate::live_index::open(&index.path().join("index.sqlite")).unwrap();
        let mut cache = Cache::default();
        sync_cached(&db, &source, false, &mut cache)
            .unwrap()
            .unwrap();
        let scope = format!("parser:{}:{VERSION}:1", source.id);
        let before = crate::live_index::load_map(&db, &scope).unwrap();
        let retained = crate::live_index::load_map(&db, &fact_scope(&source)).unwrap();
        fs::OpenOptions::new()
            .append(true)
            .open(path)
            .unwrap()
            .write_all(b"{\"type\":\"turn_context\",\"payload\":{\"turn_id\":\"u\"}}\n")
            .unwrap();
        let context = RunContext::default();
        context
            .cancelled
            .store(true, std::sync::atomic::Ordering::Relaxed);
        let error = sync_cached_with_context(&db, &source, false, &mut cache, &context)
            .err()
            .unwrap();
        assert_eq!(
            error
                .downcast_ref::<crate::dto::OperationError>()
                .unwrap()
                .code,
            "CANCELLED"
        );
        assert_eq!(crate::live_index::load_map(&db, &scope).unwrap(), before);
        assert_eq!(
            crate::live_index::load_map(&db, &fact_scope(&source)).unwrap(),
            retained
        );
    }
    #[test]
    fn prefix_checksum_accepts_append_during_streaming_read() {
        use std::io::Write;

        struct ChangingReader {
            file: File,
            path: PathBuf,
            changed: bool,
        }
        impl Read for ChangingReader {
            fn read(&mut self, bytes: &mut [u8]) -> std::io::Result<usize> {
                let count = self.file.read(bytes)?;
                if !self.changed && count > 0 {
                    fs::OpenOptions::new()
                        .append(true)
                        .open(&self.path)?
                        .write_all(b"\n")?;
                    self.changed = true;
                }
                Ok(count)
            }
        }
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("changing.jsonl");
        let record = serde_json::json!({"type":"session_meta","payload":{"id":"synthetic","padding":"x".repeat(128 * 1024)}}).to_string() + "\n";
        fs::write(&path, &record).unwrap();
        let expected = FileMetadata::read(&path).unwrap();
        let mut reader = ChangingReader {
            file: File::open(&path).unwrap(),
            path: path.clone(),
            changed: false,
        };
        let result = prefix_checksums_from(&path, expected.len(), &expected, &mut reader);
        assert!(
            reader.changed,
            "the source changed after checksum reading began"
        );
        let result = result.unwrap();
        let expected_hash = crate::hash(record.as_bytes());
        assert_eq!(result.captured, expected_hash);
        assert_eq!(result.committed, expected_hash);
    }

    #[test]
    fn captured_reader_defers_appended_records_and_consumes_them_once() {
        let root = tempfile::tempdir().unwrap();
        let dir = root.path().join("sessions");
        fs::create_dir(&dir).unwrap();
        let path = dir.join("live.jsonl");
        let header = "{\"type\":\"session_meta\",\"payload\":{\"id\":\"live\"}}\n";
        let record = |id: &str| {
            serde_json::json!({"type":"event_msg","payload":{"type":"token_usage_record","thread_id":"live","turn_id":"turn","response_id":id,"usage":{"input_tokens":100,"output_tokens":10,"total_tokens":110}}}).to_string() + "\n"
        };
        let initial = format!("{header}{}", record("one"));
        fs::write(&path, &initial).unwrap();
        let capture = FileMetadata::read(&path).unwrap();
        let captured_hash = prefix_checksums(&path, 0, &capture).unwrap().captured;
        fs::OpenOptions::new()
            .append(true)
            .open(&path)
            .unwrap()
            .write_all(record("two").as_bytes())
            .unwrap();
        let source = CodexAdapter
            .discover(&DiscoveryRequest {
                roots: vec![root.path().into()],
            })
            .sources
            .remove(0);
        let descriptor = CodexAdapter.descriptor();
        let mut report = SourceReport {
            source: source.clone(),
            adapter_version: descriptor.adapter_version,
            source_versions: vec![],
            capabilities: descriptor.capabilities,
            status: "complete".into(),
            files_read: 0,
            bytes_read: 0,
            issues: vec![],
        };
        let mut facts = Facts::default();
        let mut checkpoint = Checkpoint::default();
        read_file_from(
            &path,
            &source,
            &RunContext::default(),
            &mut facts,
            &mut report,
            Some(&mut checkpoint),
            Some(&capture),
        );
        assert!(report.issues.is_empty(), "{:?}", report.issues);
        assert_eq!(checkpoint.offset, initial.len() as u64);
        assert_eq!(facts.measurements.len(), 1);
        let watermark = facts.watermarks.values().next().unwrap();
        assert_eq!(watermark.observed_bytes, Some(capture.len()));
        assert_eq!(watermark.committed_offset, capture.len());
        assert_eq!(
            prefix_checksums(&path, checkpoint.offset, &capture)
                .unwrap()
                .captured,
            captured_hash
        );
        read_file_from(
            &path,
            &source,
            &RunContext::default(),
            &mut facts,
            &mut report,
            Some(&mut checkpoint),
            None,
        );
        assert!(report.issues.is_empty());
        assert_eq!(facts.measurements.len(), 2);
        assert_eq!(
            facts
                .measurements
                .values()
                .map(|v| v.measurement.tokens.total.unwrap())
                .sum::<u64>(),
            220
        );
        read_file_from(
            &path,
            &source,
            &RunContext::default(),
            &mut facts,
            &mut report,
            Some(&mut checkpoint),
            None,
        );
        assert_eq!(facts.measurements.len(), 2);
    }

    #[test]
    fn capture_rejects_truncation_replacement_and_detects_rewrite_with_append() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("capture.jsonl");
        fs::write(&path, b"original\n").unwrap();
        let capture = FileMetadata::read(&path).unwrap();
        let hash = prefix_checksums(&path, 0, &capture).unwrap().captured;
        // Growing the file must not hide an edit to its captured prefix.
        fs::write(&path, b"modified\nappended\n").unwrap();
        assert_ne!(prefix_checksums(&path, 0, &capture).unwrap().captured, hash);
        fs::write(&path, b"short").unwrap();
        assert!(prefix_checksums(&path, 0, &capture).is_err());
        let original = File::open(&path).unwrap();
        fs::remove_file(&path).unwrap();
        fs::write(&path, b"original\n").unwrap();
        assert!(prefix_checksums(&path, 0, &capture).is_err());
        drop(original);
    }
}

#[cfg(test)]
mod streaming_tests;
