//! On-demand local usage service. One writer commits cursors and derived facts together.
use crate::{
    adapters::{self, contract::*},
    dto::operation_error,
    usage_app_dto,
    usage_store::Snapshot,
};
use anyhow::Result;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{
    collections::{BTreeMap, VecDeque},
    fs,
    path::PathBuf,
    sync::{Arc, Condvar, Mutex, mpsc},
    time::{Duration, Instant},
};

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Request {
    pub query: usage_app_dto::Request,
    #[serde(default)]
    pub mode: Mode,
    #[serde(default)]
    pub verify: bool,
    /// One bounded wait owned by this caller; cancelling it leaves shared work running.
    #[serde(default)]
    #[schemars(range(min = 1, max = 600000))]
    pub wait_ms: Option<u64>,
    /// Explicitly rebuild derived storage in a new generation; user records stay separate.
    #[serde(default)]
    pub rebuild: bool,
    /// Include deferred historical files even for a recent query.
    #[serde(default)]
    pub include_history: bool,
    /// Additional queries share the selected immutable revision. No synchronization options.
    #[serde(default)]
    #[schemars(length(max = 8))]
    pub also: Vec<usage_app_dto::Request>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    #[default]
    Auto,
    Fresh,
    Cached,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Freshness {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub publication_change: Option<usage_app_dto::PublicationChange>,
    /// Projects restored from committed facts; pending projects are not zero usage.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub project_loads: Vec<ProjectLoad>,
    pub status: String,
    /// Ephemeral task headers; no completed ledger or coverage is available yet.
    #[serde(default)]
    pub initial_scan: bool,
    pub checked_at: Option<String>,
    pub revision: String,
    pub error: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub error_code: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ProjectLoad {
    pub project: Option<String>,
    pub state: ProjectLoadState,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ProjectLoadState {
    Pending,
    Loading,
    Ready,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Response {
    pub output_version: u32,
    pub result: usage_app_dto::Response,
    pub freshness: Freshness,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub additional: Vec<usage_app_dto::Response>,
}
struct Entry {
    paused: bool,
    changed: bool,
    last_cost: Duration,
    include_history: bool,
    restoring: bool,
    preferred_project: Option<String>,
    roots: Vec<String>,
    views: VecDeque<(Instant, Arc<Snapshot>)>,
    attempt: u64,
    checked: Option<String>,
    error: Option<String>,
    error_code: Option<&'static str>,
    touched: Instant,
    syncing: bool,
    last_sync: Instant,
    following: bool,
    requested: u64,
    completed: u64,
    pending: Option<SyncWork>,
    running: Option<SyncWork>,
}
impl Entry {
    fn new(roots: Vec<String>) -> Self {
        Self {
            paused: service::paused().unwrap_or(true),
            changed: false,
            last_cost: Duration::ZERO,
            include_history: false,
            restoring: false,
            preferred_project: None,
            roots,
            views: VecDeque::new(),
            attempt: 0,
            checked: None,
            error: None,
            error_code: None,
            touched: Instant::now(),
            syncing: false,
            last_sync: Instant::now(),
            following: false,
            requested: 0,
            completed: 0,
            pending: None,
            running: None,
        }
    }
}
type Shared = Arc<(Mutex<BTreeMap<String, Entry>>, Condvar)>;
struct Job {
    restore_only: bool,
    key: String,
}
#[derive(Clone, Copy)]
struct SyncWork {
    ticket: u64,
    verify: bool,
    rebuild: bool,
    include_history: bool,
}

fn directory() -> Result<PathBuf> {
    let path = crate::storage::data_home()?.join("live-v2");
    let mut builder = fs::DirBuilder::new();
    builder.recursive(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    builder.create(&path)?;
    Ok(path)
}
#[cfg(windows)]
fn socket_path() -> Result<PathBuf> {
    Ok(PathBuf::from(format!(
        r"\\.\pipe\wombat-{}-v2",
        &crate::hash(
            dunce::canonicalize(directory()?)?
                .to_string_lossy()
                .as_bytes()
        )[..24]
    )))
}
#[cfg(unix)]
fn socket_path() -> Result<PathBuf> {
    let root = directory()?;
    // Unix-domain paths are short even when the product data directory is deeply nested.
    let parent = std::path::Path::new("/tmp").join(format!(
        "wombat-{}",
        &crate::hash(dunce::canonicalize(&root)?.to_string_lossy().as_bytes())[..24]
    ));
    let mut builder = fs::DirBuilder::new();
    #[cfg(unix)]
    {
        use std::os::unix::fs::DirBuilderExt;
        builder.mode(0o700);
    }
    match builder.create(&parent) {
        Ok(()) => {}
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
        Err(e) => return Err(e.into()),
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt;
        let meta = fs::symlink_metadata(&parent)?;
        if !meta.is_dir() || meta.mode() & 0o077 != 0 || meta.uid() != fs::metadata(&root)?.uid() {
            return Err(operation_error("EACCES", "实时用量接口目录权限不正确"));
        }
    }
    Ok(parent.join("usage-v2.sock"))
}
pub fn endpoint() -> Result<Value> {
    let socket = socket_path()?;
    let registration = crate::hash(
        dunce::canonicalize(crate::storage::data_home()?)?
            .to_string_lossy()
            .as_bytes(),
    );
    let discovery = sources(&[]);
    let roots: Vec<_> = discovery.sources.into_iter().map(|s| s.root).collect();
    Ok(
        json!({"protocolVersion":2,"socket":socket,"registrationId":registration,"defaultRoots":roots,"discoveryError":!discovery.issues.is_empty(),"contextKey":crate::hash(serde_json::to_vec(&crate::host_environment::context()?)?)}),
    )
}
mod collection;
mod generation;
mod preview;
mod query;
mod resources;
mod scheduling;
mod selection;
mod service;
pub(crate) use service::offline as service_offline;
mod transport;
pub(crate) use collection::load_collected_except;
use collection::{restore_projects, source_key, sources, sync};
#[cfg(test)]
use query::query;
use query::{config_query, timing_query};
use selection::select_view;
pub use transport::serve;
pub use transport::serve_resident;
