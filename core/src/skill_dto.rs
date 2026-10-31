//! User Skill installation and native discovery states; the Node host owns files and RPC.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Instance {
    pub name: String,
    pub path: String,
    pub enabled: bool,
}
#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum DiscoveryStatus {
    Available,
    Ambiguous,
    Disabled,
    Missing,
    #[default]
    Unavailable,
    SelectionChanged,
}
#[derive(Clone, Debug, Default, Deserialize, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Discovery {
    pub status: DiscoveryStatus,
    pub instances: Vec<Instance>,
    pub error_code: Option<String>,
}
