//! Agent CLI request envelope. Product request DTOs keep their existing owners.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::BTreeMap;
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(
    tag = "method",
    content = "params",
    rename_all = "snake_case",
    deny_unknown_fields
)]
pub enum Request {
    /// Discover activity, locate messages, search selected conversations or explicitly read bounded original text.
    Conversation(crate::conversation_dto::Request),
    /// Query current usage and synchronize sources as requested. Params wrap query, mode and verify.
    Usage(crate::live::Request),
    /// Query fixed or saved usage views; explicit refresh scans sources and saves a new snapshot.
    Snapshot(crate::usage_app_dto::Request),
    /// Inspect current configuration and associated evidence at a read view.
    Config(crate::config_dto::Request),
    /// List, inspect, handle or recheck recommendations; handling changes user decisions.
    Optimize(crate::optimize_dto::Request),
    /// Read bounded timing and evidence for exact task/turn identities, or capabilities.
    Timing(crate::timing_dto::Request),
    /// Observe runtime compatibility and native project integration without installing.
    Setup(crate::setup_dto::Request),
    /// Read or refresh native account observations, or read saved history.
    Account(crate::account_dto::Request),
    /// List directory grants or explicitly choose, authorize or revoke a grant.
    Directories(crate::directories::Request),
    /// Read or set local presentation preferences.
    Preferences(crate::preferences::Request),
    /// Read prices or explicitly request an update.
    Prices(crate::pricing_sync::Request),
    /// Read safe collection facts or explicitly configure, pause or resume receipt.
    Collection(crate::collection::Request),
    /// Review selected targets or explicitly send an authorized native Codex request.
    Handoff(crate::handoff_dto::Request),
    /// Save token budgets and review schedules, inspect notifications, or check selected plans at a recent usage view.
    Monitor(crate::monitor::Request),
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Description {
    pub output_version: u32,
    pub runtime_version: String,
    pub methods: Vec<String>,
    pub method_descriptions: BTreeMap<String, String>,
    pub selected_method: Option<String>,
    pub input_schema: Option<Value>,
    pub output_schema: Option<Value>,
    pub stdin_bytes: usize,
    pub stdin_timeout_ms: u64,
    pub stdout_bytes: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Recovery {
    ReadSchema,
    ReacquireView,
    NarrowQuery,
    RetrySameScope,
    CheckSetup,
    InspectState,
    None,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Error {
    pub code: String,
    pub message: String,
    pub recovery: Recovery,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ErrorOutput {
    pub output_version: u32,
    pub error: Error,
}
