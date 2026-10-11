//! Incremental collection commits cursors and facts together, then publishes an immutable revision.
use super::*;
pub(super) fn sources(roots: &[String]) -> DiscoveryReport {
    adapters::codex::CodexAdapter.discover(&DiscoveryRequest {
        roots: roots.iter().map(PathBuf::from).collect(),
    })
}
pub(super) fn source_key(roots: &[String]) -> String {
    let mut ids: Vec<_> = sources(roots).sources.into_iter().map(|s| s.id).collect();
    ids.sort();
    crate::hash(ids.join(":"))
}
pub(super) fn load_collected(db: &rusqlite::Connection, key: &str) -> Result<Option<Collected>> {
    load_collected_inner(db, key, false)
}
fn load_collected_inner(
    db: &rusqlite::Connection,
    key: &str,
    skip_events: bool,
) -> Result<Option<Collected>> {
    validate_message_mapping(db, key)?;
    let mut value = Collected::default();
    let mut paths = EvidencePaths::default();
    let mut watermark_version = None;
    let mut strings = adapters::shared_strings::FactStrings::default();
    let consume = |field: &str, id: &str, payload: &str| {
        macro_rules! rows {
            ($name:ident) => {
                value.$name.push(serde_json::from_str(payload)?);
            };
        }
        match field {
            field
                if crate::observation_versions::ObservationHeaderSet::Projection
                    .kinds()
                    .iter()
                    .any(|kind| kind.field() == field) => {}
            "watermarkVersion" => {
                watermark_version = Some(serde_json::from_str::<u32>(payload)?);
            }
            "watermarks" => {
                rows!(watermarks);
                let row = value.watermarks.last().expect("just appended watermark");
                anyhow::ensure!(
                    row.file_id == id && row.source_instance_id == key,
                    "projection watermark identity mismatch"
                );
            }
            "sources" => {
                rows!(sources);
            }
            "threads" => {
                rows!(threads);
            }
            "turns" => {
                rows!(turns);
            }
            "measurements" => {
                rows!(measurements);
                if let Some(row) = value.measurements.last_mut() {
                    let row = Arc::make_mut(row);
                    paths.compact(&mut row.evidence);
                    strings.measurement(row);
                }
            }
            "operations" => {
                rows!(operations);
                if let Some(row) = value.operations.last_mut() {
                    let row = Arc::make_mut(row);
                    paths.compact(&mut row.evidence);
                    strings.operation(row);
                }
            }
            "title_observations" => {
                rows!(title_observations);
                let observation = value
                    .title_observations
                    .last()
                    .expect("just appended observation");
                observation.validate()?;
                anyhow::ensure!(
                    observation.thread_id == id && observation.source_instance_id == key,
                    "projection title scope mismatch"
                );
            }
            "events" => {
                rows!(events);
            }
            "issues" => {
                value.issues = serde_json::from_str(payload)?;
            }
            _ => anyhow::bail!("unsupported projection field: {field}"),
        }
        Ok(())
    };
    let scope = format!("projection:{key}");
    let found = if skip_events {
        crate::live_index::each_without_events(db, &scope, consume)?
    } else {
        crate::live_index::each(db, &scope, consume)?
    };
    if found {
        if watermark_version != Some(WATERMARK_FORMAT_VERSION) {
            return Err(operation_error(
                "UNSUPPORTED_VERSION",
                "不支持此投影水位格式",
            ));
        }
        validate_watermarks(&value.watermarks)?;
        let targets: BTreeMap<_, _> = value.threads.iter().map(|t| (t.id.as_str(), t)).collect();
        let mut seen = std::collections::BTreeSet::new();
        for observation in &value.title_observations {
            let thread = targets
                .get(observation.thread_id.as_str())
                .ok_or_else(|| anyhow::anyhow!("projection title target missing"))?;
            anyhow::ensure!(
                seen.insert(&observation.thread_id)
                    && thread.source_instance_id == observation.source_instance_id
                    && thread.title.as_deref() == Some(observation.title.as_str()),
                "projection title scope mismatch"
            );
        }
    }
    Ok(found.then_some(value))
}

/// Guard before reading facts and before sync's unchanged/cached fast path.
fn validate_message_mapping(db: &rusqlite::Connection, key: &str) -> Result<()> {
    let scope = format!("projection:{key}");
    if crate::live_index::has_scope(db, &scope)? {
        crate::observation_versions::ObservationHeaderSet::Projection.validate_index(
            |field| crate::live_index::scalar(db, &scope, field),
            |_| "不支持此投影来源观察映射",
        )?;
    }
    Ok(())
}

pub(super) fn sync(
    db: &mut rusqlite::Connection,
    key: &str,
    roots: &[String],
    verify: bool,
    prior_view: Option<&Snapshot>,
    caches: &mut BTreeMap<String, adapters::codex::incremental::Cache>,
) -> Result<Option<Arc<Snapshot>>> {
    let discovered = sources(roots);
    if !discovered.issues.is_empty()
        && !discovered
            .sources
            .iter()
            .any(|s| std::path::Path::new(&s.root).is_dir())
    {
        return Err(operation_error(
            "SOURCE_UNREADABLE",
            "指定的日志来源不可读取，保留已提交数据",
        ));
    }
    let mut tx = db.transaction()?;
    let mut failures = BTreeMap::<String, SourceReport>::new();
    let mut failure_watermarks = BTreeMap::<String, Vec<SourceWatermark>>::new();
    let mut changed = false;
    let mut epochs = BTreeMap::new();
    let mut updated = BTreeMap::new();
    for source in &discovered.sources {
        validate_message_mapping(&tx, &source.id)?;
    }
    for source in &discovered.sources {
        let cache = caches.entry(source.id.clone()).or_default();
        if cache.needs_seed()
            && let Some(prior) = prior_view
        {
            cache.seed(
                prior.measurement_facts().cloned().collect(),
                prior.operation_facts().cloned().collect(),
            );
            cache.seed_events(prior.events()?);
        }

        let mut source_tx = tx.savepoint()?;
        let outcome = adapters::codex::incremental::sync_cached(&source_tx, source, verify, cache);
        let synced = match outcome {
            Ok(value) => {
                source_tx.commit()?;
                value
            }
            Err(error) => {
                source_tx.rollback()?;
                drop(source_tx);
                caches.remove(&source.id);
                // Preserve the actual failure for a single-source scan instead
                // of misreporting an append/edit conflict as an unreadable root.
                if discovered.sources.len() == 1 {
                    caches.clear();
                    return Err(error);
                }
                if error
                    .downcast_ref::<crate::dto::OperationError>()
                    .is_some_and(|error| error.code == "UNSUPPORTED_VERSION")
                {
                    // Earlier sources may have updated in-memory facts inside this
                    // transaction; discard them along with the durable rollback.
                    caches.clear();
                    return Err(error);
                }
                let mut report = prior_view
                    .and_then(|v| v.manifest.sources.iter().find(|s| s.source.id == source.id))
                    .cloned()
                    .unwrap_or_else(|| SourceReport {
                        source: source.clone(),
                        adapter_version: adapters::codex::CodexAdapter.descriptor().adapter_version,
                        source_versions: vec![],
                        capabilities: adapters::codex::CodexAdapter.descriptor().capabilities,
                        status: String::new(),
                        files_read: 0,
                        bytes_read: 0,
                        issues: vec![],
                    });
                report.status = "failed".into();
                report.files_read = 0;
                report.bytes_read = 0;
                report.issues.retain(|i| i.code != "sourceSyncFailed");
                report.issues.push(Issue {
                    code: "sourceSyncFailed".into(),
                    message: format!("来源同步失败，保留上次成功数据：{error}"),
                    source_instance_id: Some(source.id.clone()),
                    evidence: None,
                });
                // Source savepoint was rolled back: only previous committed file
                // positions may be retained. This attempt has no captured length.
                let at = chrono::Utc::now().to_rfc3339();
                let watermarks = load_collected(&tx, &source.id)?
                    .map(|v| v.watermarks)
                    .or_else(|| {
                        prior_view.map(|v| {
                            v.manifest
                                .watermarks
                                .iter()
                                .filter(|w| w.source_instance_id == source.id)
                                .cloned()
                                .collect()
                        })
                    })
                    .unwrap_or_default()
                    .iter()
                    .map(|w| {
                        w.unavailable(
                            WatermarkState::Failed,
                            WatermarkIssue::SourceSyncFailed,
                            &at,
                        )
                    })
                    .collect();
                failure_watermarks.insert(source.id.clone(), watermarks);
                failures.insert(source.id.clone(), report);
                None
            }
        };
        if let Some(synced) = synced {
            let value = synced.collected;
            let scope = format!("projection:{}", source.id);
            crate::live_index::replace_field(
                &tx,
                &scope,
                "sources",
                value.sources.iter().map(|s| (s.source.id.as_str(), s)),
            )?;
            crate::live_index::put(
                &tx,
                &scope,
                "watermarkVersion",
                "",
                &WATERMARK_FORMAT_VERSION,
            )?;
            for &kind in crate::observation_versions::ObservationHeaderSet::Projection.kinds() {
                let version = kind.current();
                crate::live_index::put(&tx, &scope, kind.field(), "", &version)?;
            }
            crate::live_index::replace_field(
                &tx,
                &scope,
                "watermarks",
                value.watermarks.iter().map(|w| (w.file_id.as_str(), w)),
            )?;
            macro_rules! save {
                ($field:ident) => {
                    crate::live_index::replace_field(
                        &tx,
                        &scope,
                        stringify!($field),
                        value.$field.iter().map(|v| (v.id.as_str(), v)),
                    )?;
                };
            }
            crate::live_index::replace_field(
                &tx,
                &scope,
                "title_observations",
                value
                    .title_observations
                    .iter()
                    .map(|v| (v.thread_id.as_str(), v)),
            )?;
            save!(threads);
            save!(turns);
            if let Some(delta) = synced.measurements {
                crate::live_index::put_packed(
                    &tx,
                    &scope,
                    "measurements",
                    delta.upsert.iter().map(|r| (r.id.as_str(), r)),
                )?;
                for id in delta.remove {
                    crate::live_index::remove(&tx, &scope, "measurements", &id)?;
                }
            } else {
                crate::live_index::put_packed(
                    &tx,
                    &scope,
                    "measurements",
                    value.measurements.iter().map(|r| (r.id.as_str(), r)),
                )?;
                crate::live_index::retain_packed(
                    &tx,
                    &scope,
                    "measurements",
                    value.measurements.iter().map(|r| r.id.as_str()),
                )?;
            }
            crate::live_index::put(&tx, &scope, "issues", "", &value.issues)?;
            crate::live_index::put_packed(
                &tx,
                &scope,
                "operations",
                value
                    .operations
                    .iter()
                    .filter(|r| {
                        synced
                            .operations
                            .as_ref()
                            .is_none_or(|dirty| dirty.contains(&r.id))
                    })
                    .map(|r| (r.id.as_str(), r)),
            )?;
            if synced.operations.is_none() {
                crate::live_index::retain_packed(
                    &tx,
                    &scope,
                    "operations",
                    value.operations.iter().map(|r| r.id.as_str()),
                )?;
            }
            crate::live_index::share_events(
                &tx,
                &scope,
                &adapters::codex::incremental::fact_scope(source),
            )?;
            updated.insert(source.id.clone(), value);
            crate::live_index::save_map(
                &tx,
                &format!("epoch:{}", source.id),
                json!({"id":uuid::Uuid::new_v4().to_string()})
                    .as_object()
                    .unwrap(),
            )?;
            changed = true;
        }
        epochs.insert(
            source.id.clone(),
            crate::live_index::load_map(&tx, &format!("epoch:{}", source.id))?,
        );
    }
    if !failures.is_empty() && failures.len() == discovered.sources.len() {
        return Err(operation_error(
            "SOURCE_UNREADABLE",
            "没有可读取的来源，保留已提交数据",
        ));
    }
    let prices = crate::pricing_sync::current()?;
    let prior = crate::live_index::load_map(&tx, &format!("view:{key}"))?;
    changed |= prior.get("failures").unwrap_or(&json!({})) != &serde_json::to_value(&failures)?;
    changed |= prior.get("epochs") != Some(&serde_json::to_value(&epochs)?);
    changed |= prior.get("prices").and_then(Value::as_str) != Some(&prices.catalog_hash);
    if !changed {
        tx.commit()?;
        return Ok(None);
    }
    let mut collected = Collected {
        issues: discovered.issues,
        ..Collected::default()
    };
    for source in &discovered.sources {
        let mut value = match updated.remove(&source.id) {
            Some(value) => value,
            None => load_collected(&tx, &source.id)?.unwrap_or_default(),
        };
        if let Some(failure) = failures.get(&source.id) {
            value.sources = vec![failure.clone()];
            value.issues = failure.issues.clone();
            value.watermarks = failure_watermarks
                .get(&source.id)
                .cloned()
                .unwrap_or_default();
        }
        collected.watermarks.extend(value.watermarks);
        collected.sources.extend(value.sources);
        collected.issues.extend(value.issues);
        collected.threads.extend(value.threads);
        collected.turns.extend(value.turns);
        collected.measurements.extend(value.measurements);
        collected.operations.extend(value.operations);
        collected.events.extend(value.events);
        collected
            .title_observations
            .extend(value.title_observations);
    }
    if collected.sources.iter().any(|s| s.status == "failed")
        && !collected
            .sources
            .iter()
            .any(|s| matches!(s.status.as_str(), "complete" | "partial"))
    {
        return Err(operation_error(
            "SOURCE_UNREADABLE",
            "没有可读取的来源，保留已提交数据",
        ));
    }
    let id = format!("live:{key}:{}", uuid::Uuid::new_v4());
    let mut snapshot =
        crate::usage_store::memory(collected, id.clone(), prices.clone(), prior_view)?;
    if let Some(prior) =
        prior_view.filter(|prior| !super::preview::is_initial(prior) && !restoring_projects(prior))
    {
        snapshot.publication_change = Some(crate::usage_app::publication_change(prior, &snapshot)?);
    }
    let snapshot = Arc::new(snapshot);
    for source in &discovered.sources {
        if let Some(cache) = caches.get_mut(&source.id) {
            cache.share_measurements(snapshot.measurement_facts());
        }
    }
    crate::live_index::save_map(
        &tx,
        &format!("view:{key}"),
        json!({"id": id,"prices":prices.catalog_hash,"epochs":epochs,"failures":failures,"failureWatermarks":failure_watermarks,"createdAt":snapshot.manifest.snapshot_ref.created_at,"publicationChange":snapshot.publication_change})
            .as_object()
            .unwrap(),
    )?;
    tx.commit()?;
    Ok(Some(snapshot))
}
#[cfg(test)]
pub(super) fn restore(
    db: &rusqlite::Connection,
    key: &str,
    roots: &[String],
) -> Result<Option<Arc<Snapshot>>> {
    restore_projects::restore_projects(db, key, roots, || None, |_| {})
}
mod restore_projects;
pub(super) use restore_projects::incomplete as restoring_projects;
pub(super) use restore_projects::restore_projects;

#[cfg(test)]
mod observation_tests;
#[cfg(test)]
mod watermark_tests;
