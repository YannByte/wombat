//! Local service controls. Registration is a host observation, not core readiness.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    Status,
    Start,
    Pause,
    Resume,
    Stop,
    Enable,
    Disable,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Request {
    pub action: Action,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeIdentity {
    pub binary: String,
    pub bytes: u64,
    pub modified_ns: String,
    pub context_key: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Response {
    pub output_version: u32,
    pub action: Action,
    pub running: bool,
    pub resident: bool,
    pub paused: bool,
    pub stopping: bool,
    pub instance_id: Option<String>,
    pub runtime: Option<RuntimeIdentity>,
    pub auto_start: bool,
    pub controls_available: bool,
    pub preparing: Option<bool>,
    pub ready: Option<bool>,
    pub checked_at: Option<String>,
    pub revision: Option<String>,
    pub error_code: Option<String>,
    pub watcher: String,
    /// Encoded parser-state cache budget; this is not a hard process RSS limit.
    pub parser_cache_bytes: u64,
    pub parser_cache_budget_bytes: u64,
}
