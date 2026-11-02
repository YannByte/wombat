//! Foreground project evidence. Component facts retain their own versions and accounting.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Request {
    pub project_root: String,
    #[serde(default)]
    pub roots: Vec<String>,
    pub since: String,
    pub until: String,
    pub timezone: String,
    pub source_instance_id: Option<String>,
    pub rule_overrides: Option<crate::optimize_dto::RuleOverrides>,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum State {
    Complete,
    Partial,
    Unavailable,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Failure {
    pub code: String,
    pub message: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Section<T> {
    pub state: State,
    pub result: Option<T>,
    pub error: Option<Failure>,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Response {
    #[schemars(range(min = 2, max = 2))]
    pub output_version: u32,
    pub scope: Request,
    pub partial: bool,
    pub configuration: Section<crate::config_dto::Response>,
    pub checks: Section<crate::optimize_dto::Response>,
    pub history: Section<crate::live::Response>,
    pub records: Section<crate::conversation_dto::Response>,
    pub artifacts: Section<crate::artifacts_dto::Response>,
}
