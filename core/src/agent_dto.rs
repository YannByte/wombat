//! Agent CLI request envelope. Product request DTOs keep their existing owners.
pub const REQUEST_VERSION: u32 = 2;

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
    /// Assemble current project checks, configuration, bounded work history and read-only artifact candidates without editing project files.
    Project(crate::project_dto::Request),
    /// Inspect artifacts, recheck original selections or explicitly remove selected Codex candidates; no session indexing.
    Artifacts(crate::artifacts_dto::Request),
    /// Discover activity, locate messages, search selected conversations or explicitly read bounded original text.
    Conversation(crate::conversation_dto::Request),
    /// Query current usage and synchronize sources as requested. Params wrap query, mode and verify.
    Usage(crate::live::Request),
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
    /// List directory grants or explicitly authorize or revoke a selected path.
    Directories(crate::directories::Request),
    /// Read or set local presentation preferences.
    Preferences(crate::preferences::Request),
    /// Read prices or explicitly request an update.
    Prices(crate::pricing_sync::Request),
    /// Read safe collection facts or explicitly configure, pause or resume receipt.
    Collection(crate::collection::Request),
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
    NarrowQuery,
    RetrySameScope,
    CheckSetup,
    InspectState,
    None,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Error {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub details: Option<InputErrorDetails>,
    pub code: String,
    pub message: String,
    pub recovery: Recovery,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InputErrorDetails {
    #[schemars(length(max = 4))]
    pub fields: Vec<InputErrorField>,
    pub schema_command: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InputErrorField {
    #[schemars(length(max = 256))]
    pub path: String,
    #[schemars(length(max = 160))]
    pub reason: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ErrorOutput {
    pub output_version: u32,
    pub error: Error,
}

/// Task-oriented local command discovery. No runtime or source preparation.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Help {
    #[schemars(range(min = 1, max = 1))]
    pub output_version: u32,
    pub command: String,
    pub commands: Vec<String>,
    pub help: String,
    pub examples: Vec<HelpExample>,
    pub next: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HelpExample {
    pub task: String,
    pub command: String,
}

/// A bounded original-text read and an independent continuation for message locations.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct HistoryRead {
    #[schemars(range(min = 3, max = 3))]
    pub output_version: u32,
    pub result: crate::conversation_dto::Response,
    pub next_locations: Option<crate::conversation_dto::Request>,
    /// Original read targets deferred by the response budget.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub retry_read: Option<crate::conversation_dto::Request>,
}
