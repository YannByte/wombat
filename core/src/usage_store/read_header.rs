//! Small committed query headers contain no turn, operation or event directory.
//! The canonical archive remains authoritative for details and parser recovery.
use super::*;
use rusqlite::Connection;

const VERSION: u32 = 1;
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct Header {
    version: u32,
    manifest: Manifest,
}
pub(crate) fn save(db: &Connection, key: &str, snapshot: &Snapshot) -> Result<()> {
    let m = &snapshot.manifest;
    let manifest = Manifest {
        schema_version: m.schema_version,
        observation_versions: m.observation_versions,
        snapshot_ref: m.snapshot_ref.clone(),
        price_revision: m.price_revision.clone(),
        price_catalog_hash: m.price_catalog_hash.clone(),
        sources: m.sources.clone(),
        watermarks: m.watermarks.clone(),
        issues: m.issues.clone(),
        ledger: m.ledger.clone(),
        title_observations: m.title_observations.clone(),
        events: EventIndex {
            version: super::events::EVENT_INDEX_VERSION,
            partitions: vec![],
        },
        threads: m
            .threads
            .iter()
            .map(|entry| {
                Arc::new(ThreadEntry {
                    thread: entry.thread.clone(),
                    file: entry.file.clone(),
                    turns: BTreeMap::new(),
                    unassigned_uses: entry.unassigned_uses.clone(),
                })
            })
            .collect(),
    };
    crate::live_index::put(
        db,
        &format!("view:{key}"),
        "readHeader",
        "",
        &Header {
            version: VERSION,
            manifest,
        },
    )
}
pub(crate) fn load(
    db: &Connection,
    key: &str,
    prior: &serde_json::Map<String, serde_json::Value>,
    prices: crate::pricing_sync::Response,
) -> Result<Option<Snapshot>> {
    let Some(raw) = prior.get("readHeader") else {
        return Ok(None);
    };
    if raw.get("version").and_then(serde_json::Value::as_u64) != Some(u64::from(VERSION)) {
        return Err(operation_error(
            "UNSUPPORTED_VERSION",
            "Unsupported query header version",
        ));
    }
    if raw["manifest"]["schemaVersion"] != 4 {
        return Err(operation_error(
            "UNSUPPORTED_VERSION",
            "Unsupported query manifest version",
        ));
    }
    crate::observation_versions::ObservationHeaderSet::Snapshot.validate_json(
        &raw["manifest"],
        |_| "Unsupported query observation version",
    )?;
    let header: Header = serde_json::from_value(raw.clone())?;
    anyhow::ensure!(
        header.manifest.snapshot_ref.snapshot_id == prior["id"].as_str().unwrap_or_default()
            && header.manifest.price_catalog_hash == prices.catalog_hash
            && header.manifest.events.partitions.is_empty()
            && header.manifest.threads.iter().all(|t| t.turns.is_empty()),
        "query header identity mismatch"
    );
    let mut snapshot = memory::memory(
        Collected::default(),
        header.manifest.snapshot_ref.snapshot_id.clone(),
        prices.clone(),
        None,
    )?;
    snapshot.manifest = header.manifest;
    snapshot.live_events = None;
    let mut titles = Vec::new();
    for source in &snapshot.manifest.sources {
        crate::live_index::each_metadata_field(
            db,
            &format!("projection:{}", source.source.id),
            "title_observations",
            |id, payload| {
                let value: crate::session_events::title_observations::TitleObservation =
                    serde_json::from_str(payload)?;
                anyhow::ensure!(
                    value.thread_id == id && value.source_instance_id == source.source.id,
                    "query title scope mismatch"
                );
                titles.push(value);
                Ok(())
            },
        )?;
    }
    validate_title_observations(&titles, snapshot.manifest.threads.iter().map(|t| &t.thread))?;
    let expected: BTreeMap<_, _> = snapshot
        .manifest
        .title_observations
        .iter()
        .map(|title| (title.thread_id.as_str(), title))
        .collect();
    let actual: BTreeMap<_, _> = titles
        .iter()
        .map(|title| (title.thread_id.as_str(), title))
        .collect();
    anyhow::ensure!(
        actual == expected,
        "query title metadata changed without publication"
    );
    validate_title_observations(
        &snapshot.manifest.title_observations,
        snapshot.manifest.threads.iter().map(|t| &t.thread),
    )?;
    for watermark in &snapshot.manifest.watermarks {
        watermark.validate()?;
    }
    snapshot.cold_events = Some(cold_events::ColdEvents::capture(
        db,
        key,
        &snapshot.manifest.snapshot_ref.snapshot_id,
        &snapshot.manifest.sources,
        prices,
    )?);
    snapshot.defer_details();
    Ok(Some(snapshot))
}
impl Snapshot {
    pub(crate) fn expired_detail_lease(&self) -> bool {
        self.is_summary_view() && self.cold_events.is_none()
    }
    pub(crate) fn freeze_cached_answers(&mut self) -> bool {
        self.release_detail_cache();
        if !self.exclusive_cold_lease() {
            return false;
        }
        self.cold_events = None;
        *self.timing_cache.lock().unwrap() = Default::default();
        true
    }
    pub(crate) fn release_detail_cache(&self) {
        if let Some(cache) = &self.lazy_details
            && let Ok(mut value) = cache.try_lock()
        {
            *value = None;
        }
    }
    #[cfg(test)]
    pub(crate) fn lazy_details_empty_for_test(&self) -> bool {
        self.lazy_details
            .as_ref()
            .is_some_and(|cache| cache.lock().unwrap().is_none())
    }
    pub(crate) fn defer_details(&mut self) {
        self.lazy_details = Some(Mutex::new(None));
    }
    pub(crate) fn is_summary_view(&self) -> bool {
        self.lazy_details.is_some()
    }
    pub(crate) fn detailed_view(&self) -> Result<Option<Arc<Snapshot>>> {
        let Some(cache) = &self.lazy_details else {
            return Ok(None);
        };
        let mut cached = cache
            .lock()
            .map_err(|_| corrupt("detail loading unavailable"))?;
        if let Some(view) = &*cached {
            return Ok(Some(Arc::clone(view)));
        }
        let reader = self.cold_events.as_ref().ok_or_else(|| {
            operation_error(
                "VIEW_EXPIRED",
                "Live detail lease expired; reacquire the view",
            )
        })?;
        let view = Arc::new(reader.restore_details(self)?);
        *cached = Some(Arc::clone(&view));
        Ok(Some(view))
    }
}
