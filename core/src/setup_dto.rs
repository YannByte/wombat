//! Host setup observations. Discovery, registration and collection remain distinct.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Request {
    pub project: Option<String>,
    pub roots: Option<Vec<String>>,
    #[serde(default)]
    pub include_hooks: bool,
}
#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Response {
    pub output_version: u32,
    pub checked_at: String,
    pub project: Option<String>,
    pub native_version: Option<String>,
    pub discovery: crate::skill_dto::Discovery,
    pub hooks: Option<crate::config_dto::HookRegistry>,
    pub runtime_capabilities: Vec<String>,
    pub marketplace_path: Option<String>,
    pub error_codes: Vec<String>,
    /// Installed CLI bundle identity, separate from the native Codex version.
    #[serde(default)]
    pub runtime_version: Option<String>,
    #[serde(default)]
    pub runtime_checks: Vec<RuntimeCheck>,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeStatus {
    Compatible,
    Incompatible,
    Modified,
    Unavailable,
    Unmanaged,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeCheck {
    pub path: String,
    pub status: RuntimeStatus,
    pub plugin_version: Option<String>,
    pub skill_content_hash: Option<String>,
    pub required_capabilities: Vec<String>,
    pub missing_capabilities: Vec<String>,
    pub error_code: Option<String>,
}
