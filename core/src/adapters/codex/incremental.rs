//! Durable parser checkpoints contain allowlisted facts only, never event bodies.
use super::*;
use anyhow::Result;
use rusqlite::Connection;

use crate::file_metadata::FileMetadata;
use std::time::UNIX_EPOCH;
mod recent;
mod reduction;
#[cfg(test)]
mod reduction_tests;

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
    facts: Option<Facts>,
    projected: Option<Vec<Arc<Measurement>>>,
    seed: Option<ReusableFacts>,
}

struct ReusableFacts {
    measurements: Vec<Arc<Measurement>>,
    operations: Vec<Arc<Operation>>,
}

impl Cache {
    pub(crate) fn needs_seed(&self) -> bool {
        self.facts.is_none() && self.seed.is_none()
    }
    pub(crate) fn seed(
        &mut self,
        measurements: Vec<Arc<Measurement>>,
        mut operations: Vec<Arc<Operation>>,
    ) {
        operations.sort_unstable_by(|a, b| a.id.cmp(&b.id));
        self.seed = Some(ReusableFacts {
            measurements,
            operations,
        });
    }
    pub(crate) fn share_measurements<'a>(
        &mut self,
        rows: impl Iterator<Item = &'a Arc<Measurement>>,
    ) {
        // Candidates, projected facts and caller rows are ID-sorted. Merge
        // them without a tree lookup and binary search for every ledger row.
        let Some(facts) = &mut self.facts else {
            return;
        };
        let mut rows = rows.peekable();
        let projected = self.projected.as_deref_mut().unwrap_or_default();
        let mut index = 0;
        for (id, candidate) in &mut facts.measurements {
            while rows.peek().is_some_and(|row| row.id < *id) {
                rows.next();
            }
            let Some(row) = rows.peek().copied().filter(|row| row.id == *id) else {
                continue;
            };
            if !Arc::ptr_eq(&candidate.measurement, row) && candidate.measurement == *row {
                candidate.measurement = Arc::clone(row);
            }
            while projected.get(index).is_some_and(|value| value.id < *id) {
                index += 1;
            }
            if let Some(value) = projected.get_mut(index)
                && value.id == *id
                && !Arc::ptr_eq(value, row)
                && *value == *row
            {
                *value = Arc::clone(row);
            }
        }
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

fn load_facts(
    db: &Connection,
    scope: &str,
    reuse: Option<&ReusableFacts>,
    report: &SourceReport,
    cancelled: &std::sync::atomic::AtomicBool,
    retain_all: bool,
) -> Result<Facts> {
    let mut events = BTreeMap::new();
    let mut event_strings = crate::session_events::EventStrings::default();
    let mut title_observations = BTreeMap::new();
    crate::live_index::each_without_events(db, scope, |field, id, payload| {
        crate::operation_association::check(cancelled)?;
        if field == "title_observations" {
            let observation: crate::session_events::title_observations::TitleObservation =
                serde_json::from_str(payload)?;
            observation.validate()?;
            anyhow::ensure!(
                observation.thread_id == id && observation.source_instance_id == report.source.id,
                "stored title scope mismatch"
            );
            title_observations.insert(id.into(), observation);
        } else {
            anyhow::ensure!(
                matches!(
                    field,
                    "threads"
                        | "turns"
                        | "measurements"
                        | "operations"
                        | "aliases"
                        | "parents"
                        | "projects"
                        | "measurement_conflicts"
                ),
                "unsupported fact field: {field}"
            );
        }
        Ok(())
    })?;
    if retain_all {
        crate::live_index::each_event(db, scope, |mut event| {
            crate::operation_association::check(cancelled)?;
            event_strings.share(Arc::make_mut(&mut event));
            anyhow::ensure!(
                event.position().source_instance_id.as_ref() == report.source.id,
                "stored event source identity mismatch"
            );
            events.insert(event.shared_id(), event);
            Ok(())
        })?;
    }
    let mut facts = if retain_all {
        replay_events(events, report, cancelled)?
    } else {
        reduction::load_streamed(db, scope, report, cancelled)?
    };
    facts.title_observations = title_observations;
    for observation in facts.title_observations.values() {
        crate::operation_association::check(cancelled)?;
        anyhow::ensure!(
            facts
                .threads
                .get(&observation.thread_id)
                .is_some_and(|t| t.source_instance_id == observation.source_instance_id),
            "stored title target mismatch"
        );
    }
    titles::apply_titles(&mut facts);
    // Read projections may share allocations, but never establish parser facts.
    let mut paths = EvidencePaths::default();
    for (id, candidate) in &mut facts.measurements {
        crate::operation_association::check(cancelled)?;
        if let Some(rows) = reuse.map(|r| &r.measurements)
            && let Ok(index) = rows.binary_search_by(|r| r.id.as_str().cmp(id))
            && rows[index] == candidate.measurement
        {
            candidate.measurement = Arc::clone(&rows[index]);
        }
        if Arc::strong_count(&candidate.measurement) == 1 {
            let row = Arc::make_mut(&mut candidate.measurement);
            paths.compact(&mut row.evidence);
            facts.strings.measurement(row);
        }
    }
    for (id, operation) in &mut facts.operations {
        crate::operation_association::check(cancelled)?;
        if let Some(rows) = reuse.map(|r| &r.operations)
            && let Ok(index) = rows.binary_search_by(|r| r.id.as_str().cmp(id))
            && rows[index] == *operation
        {
            *operation = Arc::clone(&rows[index]);
        }
        if Arc::strong_count(operation) == 1 {
            let row = Arc::make_mut(operation);
            paths.compact(&mut row.evidence);
            facts.strings.operation(row);
        }
    }
    Ok(facts)
}

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

pub(crate) struct MeasurementDelta {
    pub upsert: Vec<Arc<Measurement>>,
    pub remove: Vec<String>,
}
pub(crate) struct Synced {
    pub collected: Collected,
    pub operations: Option<BTreeSet<String>>,
    pub measurements: Option<MeasurementDelta>,
}
// Both lists are ID-sorted; reconcile inserts, corrections and retractions in one pass.
fn projection_delta(old: &[Arc<Measurement>], new: &[Arc<Measurement>]) -> MeasurementDelta {
    let mut delta = MeasurementDelta {
        upsert: vec![],
        remove: vec![],
    };
    let (mut left, mut right) = (0, 0);
    while left < old.len() || right < new.len() {
        match (old.get(left), new.get(right)) {
            (Some(a), Some(b)) => match a.id.cmp(&b.id) {
                std::cmp::Ordering::Less => {
                    delta.remove.push(a.id.clone());
                    left += 1;
                }
                std::cmp::Ordering::Greater => {
                    delta.upsert.push(Arc::clone(b));
                    right += 1;
                }
                std::cmp::Ordering::Equal => {
                    if !Arc::ptr_eq(a, b) && a != b {
                        delta.upsert.push(Arc::clone(b));
                    }
                    left += 1;
                    right += 1;
                }
            },
            (Some(a), None) => {
                delta.remove.push(a.id.clone());
                left += 1;
            }
            (None, Some(b)) => {
                delta.upsert.push(Arc::clone(b));
                right += 1;
            }
            (None, None) => break,
        }
    }
    delta
}
pub(crate) fn sync_cached(
    db: &Connection,
    source: &SourceInstance,
    verify: bool,
    cache: &mut Cache,
) -> Result<Option<Synced>> {
    let context = RunContext::default();
    sync_cached_with_context(db, source, verify, cache, &context)
}
pub(crate) fn sync_cached_recent(
    db: &Connection,
    source: &SourceInstance,
    verify: bool,
    cache: &mut Cache,
) -> Result<Option<Synced>> {
    sync_with_policy(db, source, verify, cache, &RunContext::default(), true)
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
    let restoring = cache.facts.is_none();
    let mut facts = match cache.facts.take() {
        Some(facts) => facts,
        None => load_facts(
            db,
            &fact_scope,
            cache.seed.as_ref(),
            &report,
            &context.cancelled,
            rebuild,
        )?,
    };
    cache.seed = None;
    facts.dirty_events.clear();
    facts.dirty_operations.clear();
    facts.retired_operations.clear();
    facts.dirty_measurements.clear();
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
    if rebuild && facts.streaming {
        facts = load_facts(db, &fact_scope, None, &report, &context.cancelled, true)?;
    }
    if rebuild {
        // Cross-file ownership and late direct measurements require source-wide
        // reconciliation. Retain facts evidenced solely by now-missing files.
        let retained_collection_times = facts
            .events
            .iter()
            .map(|(id, e)| (id.clone(), e.collected_at().to_owned()))
            .collect();
        let title_observations = std::mem::take(&mut facts.title_observations);
        let missing_files: BTreeSet<_> =
            missing.iter().map(|p| crate::hash(p.as_bytes())).collect();
        facts
            .events
            .retain(|_, event| missing_files.contains(event.position().file_id.as_ref()));
        facts = replay_events(
            std::mem::take(&mut facts.events),
            &report,
            &context.cancelled,
        )?;
        facts.retained_collection_times = retained_collection_times;
        facts.title_observations = title_observations;
        checkpoints.retain(|p, _| missing.contains(p));
        dirty = files
            .iter()
            .map(|p| p.to_string_lossy().into_owned())
            .collect();
    }
    // Full writes already visit every event. Building a second ordered identity
    // set adds no information and needlessly grows with the entire archive.
    facts.full_event_write = rebuild || previous.is_empty() || restoring;
    if previous.is_empty() {
        facts.streaming = true;
        crate::live_index::save_events(db, &fact_scope, [], true, &context.cancelled)?;
    }
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
                    crate::live_index::save_events(
                        db,
                        &fact_scope,
                        &facts.pending_events,
                        false,
                        &context.cancelled,
                    )?;
                    facts.pending_events.clear();
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
    if rebuild && !facts.streaming {
        // Missing files were retained before existing files were parsed. Restore
        // their combined source order before publishing or saving projections.
        // Checkpoints already own diagnostics; replay must not append them again.
        let title_observations = std::mem::take(&mut facts.title_observations);
        facts = replay_events(
            std::mem::take(&mut facts.events),
            &report,
            &context.cancelled,
        )?;
        facts.title_observations = title_observations;
    }
    if facts.streaming {
        reduction::resolve_streamed(db, &fact_scope, &mut facts, &mut report, &context.cancelled)?;
    } else {
        facts.resolve_operations(&mut report, &context.cancelled)?;
    }
    read_titles(root, &mut facts, &mut report);
    titles::apply_titles(&mut facts);
    for cp in checkpoints.values() {
        report.issues.extend(cp.issues.iter().cloned());
    }
    if let Some(versions) = previous.get("sourceVersions") {
        for v in serde_json::from_value::<Vec<String>>(versions.clone())? {
            if !report.source_versions.contains(&v) {
                report.source_versions.push(v);
            }
        }
    }
    // Events restore parser truth; refresh canonical query facts after replay.
    let full = rebuild || previous.is_empty() || restoring;
    let replace_operations = full || !facts.retired_operations.is_empty();
    let changed_operations = if replace_operations || !facts.parents.is_empty() {
        None
    } else {
        Some(facts.dirty_operations.clone())
    };
    // Complete source-derived calculations before any parser writes. Errors or
    // cancellation then leave publication to the caller's source transaction.
    let mut result = Collected::default();
    for checkpoint in checkpoints.values() {
        crate::operation_association::check(&context.cancelled)?;
        if let Some(watermark) = &checkpoint.watermark {
            result.watermarks.push(watermark.clone());
        }
    }
    facts.watermarks.clear();
    let mut direct_only = facts.parents.is_empty();
    if direct_only {
        for candidate in facts.measurements.values() {
            crate::operation_association::check(&context.cancelled)?;
            if !candidate.direct {
                direct_only = false;
                break;
            }
        }
    }
    crate::operation_association::check(&context.cancelled)?;
    let derived = facts.fork_derived(!direct_only);
    let mut derived = finish_projection(derived, &mut report, &context.cancelled)?;
    titles::apply_titles(&mut derived);
    crate::operation_association::check(&context.cancelled)?;
    emit_facts(derived, &mut result, &context.cancelled)?;
    if !facts.streaming {
        for event in facts.events.values() {
            crate::operation_association::check(&context.cancelled)?;
            result.events.push(Arc::clone(event));
        }
    }
    if direct_only {
        for candidate in facts.measurements.values() {
            crate::operation_association::check(&context.cancelled)?;
            result.measurements.push(Arc::clone(&candidate.measurement));
        }
        for operation in facts.operations.values() {
            crate::operation_association::check(&context.cancelled)?;
            result.operations.push(Arc::clone(operation));
        }
    }
    crate::operation_association::check(&context.cancelled)?;
    if facts.streaming {
        crate::live_index::save_events(
            db,
            &fact_scope,
            &facts.pending_events,
            false,
            &context.cancelled,
        )?;
        facts.pending_events.clear();
    } else if full {
        crate::live_index::save_events(
            db,
            &fact_scope,
            facts.events.values(),
            true,
            &context.cancelled,
        )?;
    } else {
        crate::live_index::save_events(
            db,
            &fact_scope,
            facts.dirty_events.iter().map(|id| &facts.events[id]),
            false,
            &context.cancelled,
        )?;
    }
    facts.dirty_events.clear();
    crate::live_index::replace_field(
        db,
        &fact_scope,
        "title_observations",
        facts
            .title_observations
            .iter()
            .map(|(id, value)| (id.as_str(), value)),
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
    facts.dirty_measurements.clear();
    facts.dirty_operations.clear();
    facts.retired_operations.clear();
    crate::operation_association::check(&context.cancelled)?;
    // Verification may temporarily own all observations. Ordinary appends use
    // canonical components and affected archive scopes; evidence stays durable.
    facts.events.clear();
    if !facts.streaming {
        facts.association_groups = None;
    }
    facts.streaming = true;
    cache.facts = Some(facts);
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
    result.measurements.sort_unstable_by(|a, b| a.id.cmp(&b.id));
    let measurements = cache
        .projected
        .as_ref()
        .map(|old| projection_delta(old, &result.measurements));
    cache.projected = Some(result.measurements.clone());
    Ok(Some(Synced {
        collected: result,
        operations: changed_operations,
        measurements,
    }))
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
        let retained = cache.facts.as_ref().unwrap().events.clone();
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
            cache
                .facts
                .as_ref()
                .unwrap()
                .events
                .keys()
                .collect::<Vec<_>>(),
            retained.keys().collect::<Vec<_>>()
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

    fn row(id: &str, total: u64) -> Arc<Measurement> {
        Arc::new(serde_json::from_value(serde_json::json!({"id":id,"agentKind":"synthetic","sourceInstanceId":"s","grain":"response","timePrecision":"unknown","model":{},"tokens":{"total":total},"tokenUnavailableReasons":{"input":"missing","cacheRead":"missing","cacheCreate":"missing","output":"missing","reasoning":"missing","total":null,"rawInput":"missing"},"pricingContextConflict":false,"requestScoped":true,"sequence":0,"evidence":[]})).unwrap())
    }
    #[test]
    fn ordered_sharing_preserves_corrections_missing_and_foreign_facts() {
        let b = row("b", 1);
        let d = row("d", 2);
        let f = row("f", 1);
        let z = row("z", 1);
        let facts = Facts {
            measurements: [b.clone(), d.clone(), f.clone(), z.clone()]
                .into_iter()
                .map(|measurement| {
                    (
                        measurement.id.clone(),
                        Candidate {
                            measurement,
                            direct: true,
                            cumulative: None,
                            interval_start: None,
                            fingerprint: String::new(),
                        },
                    )
                })
                .collect(),
            ..Facts::default()
        };
        let orphan = row("c", 1);
        let mut cache = Cache {
            facts: Some(facts),
            projected: Some(vec![
                b.clone(),
                orphan.clone(),
                d.clone(),
                f.clone(),
                z.clone(),
            ]),
            seed: None,
        };
        let rows = [
            row("a", 1),
            row("b", 1),
            row("d", 3),
            row("e", 1),
            row("f", 1),
            row("g", 1),
        ];
        cache.share_measurements(rows.iter());
        let facts = &cache.facts.as_ref().unwrap().measurements;
        let projected = cache.projected.as_ref().unwrap();
        assert!(Arc::ptr_eq(&facts["b"].measurement, &rows[1]));
        assert!(Arc::ptr_eq(&projected[0], &rows[1]));
        assert!(Arc::ptr_eq(&facts["d"].measurement, &d));
        assert!(Arc::ptr_eq(&projected[2], &d));
        assert!(Arc::ptr_eq(&facts["f"].measurement, &rows[4]));
        assert!(Arc::ptr_eq(&projected[3], &rows[4]));
        assert!(Arc::ptr_eq(&facts["z"].measurement, &z));
        assert!(Arc::ptr_eq(&projected[4], &z));
        assert!(Arc::ptr_eq(&projected[1], &orphan));
        cache.share_measurements(rows.iter());
        assert_eq!(cache.facts.as_ref().unwrap().measurements.len(), 4);
        let mut empty = Cache::default();
        empty.share_measurements(rows.iter());
    }
}

#[cfg(test)]
mod streaming_tests;
