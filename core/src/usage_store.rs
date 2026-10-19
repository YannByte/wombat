//! Immutable generations: ledger and per-thread/turn shards, manifest committed last.
use crate::adapters::contract::*;
use crate::dto::operation_error;
use crate::pricing::{self, PriceResult, PricingContext};
use crate::usage_app_dto::SnapshotRef;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::{
    collections::BTreeMap,
    fs,
    io::{Read, Seek, SeekFrom},
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

const MAX_FILE_BYTES: u64 = 512 * 1024 * 1024;
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PricedMeasurement {
    pub fact: Arc<Measurement>,
    pub price: Arc<PriceResult>,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FileRef {
    pub file: String,
    pub sha256: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Slice {
    pub offset: u64,
    pub length: u64,
    pub sha256: String,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnEntry {
    pub turn: Option<Turn>,
    pub slice: Slice,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ThreadEntry {
    pub thread: Thread,
    pub file: FileRef,
    pub turns: BTreeMap<String, TurnEntry>,
    pub unassigned_uses: UnassignedUseRecords,
}
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub schema_version: u32,
    #[serde(flatten)]
    pub(crate) observation_versions: crate::observation_versions::SnapshotObservationVersions,
    pub title_observations: Vec<crate::session_events::title_observations::TitleObservation>,
    pub snapshot_ref: SnapshotRef,
    pub price_revision: String,
    pub price_catalog_hash: String,
    pub sources: Vec<SourceReport>,
    pub watermarks: Vec<SourceWatermark>,
    pub issues: Vec<Issue>,
    pub ledger: FileRef,
    pub events: EventIndex,
    pub threads: Vec<Arc<ThreadEntry>>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TurnData {
    pub measurements: Vec<PricedMeasurement>,
    pub operations: Vec<Operation>,
}
#[derive(Clone, Default)]
struct MemoryTurn {
    measurements: Vec<usize>,
    operations: Vec<Arc<Operation>>,
    cold_operations: Vec<crate::live_index::FactLocation>,
}
pub struct Snapshot {
    /// Statistics open independently of historical evidence. Details are loaded
    /// at most once, from this view's pinned SQLite transaction, on demand.
    lazy_details: Option<Mutex<Option<Arc<Snapshot>>>>,
    pub(crate) summary_index: Arc<crate::usage_app::SummaryIndex>,
    pub(crate) publication_change: Option<crate::usage_app_dto::PublicationChange>,
    pub(crate) project_loads: Vec<crate::live::ProjectLoad>,
    pub(crate) timing_cache: Mutex<crate::timing::cache::Cache>,
    pub(crate) query_cache: Mutex<crate::query_cache::QueryCache>,
    pub(crate) query_work: Arc<Mutex<()>>,
    live_rows: Option<Arc<Vec<Arc<PricedMeasurement>>>>,
    project_rows: Option<Arc<BTreeMap<Option<String>, Vec<usize>>>>,
    live_events: Option<events::EventBuckets>,
    cold_events: Option<Arc<cold_events::ColdEvents>>,
    memory_turns: Option<Arc<BTreeMap<(String, String), MemoryTurn>>>,
    pub manifest: Manifest,
    directory: PathBuf,
}

fn validate_title_observations<'a>(
    rows: &[crate::session_events::title_observations::TitleObservation],
    threads: impl IntoIterator<Item = &'a Thread>,
) -> Result<()> {
    let targets: BTreeMap<_, _> = threads.into_iter().map(|t| (t.id.as_str(), t)).collect();
    let mut seen = std::collections::BTreeSet::new();
    for observation in rows {
        observation.validate()?;
        anyhow::ensure!(
            seen.insert(&observation.thread_id),
            "duplicate title observation"
        );
        let thread = targets
            .get(observation.thread_id.as_str())
            .ok_or_else(|| anyhow::anyhow!("title target missing"))?;
        anyhow::ensure!(
            thread.source_instance_id == observation.source_instance_id
                && thread.title.as_deref() == Some(observation.title.as_str()),
            "title observation scope mismatch"
        );
    }
    Ok(())
}

mod cold_events;
pub(crate) mod ledger_projection;
pub(crate) use ledger_projection::{
    schema as ledger_projection_schema, update as update_ledger_projection,
};
mod events;
pub use events::{
    EventChunk, EventCursor, EventIndex, EventPage, EventPartition, EventReadBudget, EventTarget,
    MAX_EVENT_PAGE_ROWS, MAX_TARGET_EVENTS,
};
#[cfg(test)]
mod event_tests;
mod files;
mod memory;
mod native_boundary;
pub use native_boundary::NativeBoundaryIndex;
mod price_pool;
mod query;
#[cfg(test)]
mod tests;
pub mod timing_evidence;
mod timing_measurements;
mod use_metadata;
pub use use_metadata::UnassignedUseRecords;
#[cfg(test)]
mod watermark_tests;
pub(crate) use cold_events::{
    load_index as load_cold_event_index, save_index as save_cold_event_index,
};
pub use files::{RefreshLock, load, save};
use files::{bounded_read, corrupt, file_ref, product_home, safe_file, save_with_prices};
#[cfg(test)]
use files::{load_at, private_dir, save_at};
pub(crate) use memory::memory;
pub(crate) use memory::memory_cold_operations;

#[cfg(test)]
mod observation_tests;

pub(crate) use cold_events::build_from_sources as cold_event_index_from_sources;

pub(crate) mod cold_operations;
pub(crate) use cold_operations::ColdOperations;
mod read_header;
pub(crate) use read_header::{load as load_read_header, save as save_read_header};
