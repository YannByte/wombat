//! Foreground incremental queries. Each caller owns its core process and read transaction.
pub const REQUEST_VERSION: u32 = 2;

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
use std::{collections::BTreeMap, fs, path::PathBuf, sync::Arc};
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Request {
    pub query: usage_app_dto::Request,
    #[serde(default)]
    pub mode: Mode,
    #[serde(default)]
    pub verify: bool,
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
mod collection;
mod generation;
mod project;
mod query;
mod resources;
pub(crate) use collection::load_collected_except;
use collection::{source_key, sync};
pub(crate) use query::cleanup_rebuild;
pub use query::dispatch;
