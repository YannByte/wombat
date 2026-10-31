//! Fixed observation metadata; current configuration never establishes historical loading.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UseBasisStatus {
    Observed,
    /// Canonical observed counts remain usable; gaps prevent a complete count.
    Partial,
    Unavailable,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UseUnit {
    ObjectUse,
    RuleRead,
    RuleLoadOrRead,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UseTimeBasis {
    SourceOperationTime,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UseSourceCompleteness {
    Complete,
    Partial,
    Unknown,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum UseWindow {
    AllHistory,
    /// Inclusive local date since, exclusive local date until, in the recorded timezone.
    DateWindow {
        since: String,
        until: String,
        timezone: String,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UseScope {
    pub source_instance_ids: Vec<String>,
    pub project: Option<String>,
    pub thread_id: Option<String>,
    pub agent_kind: Option<String>,
    pub window: UseWindow,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UseCoverage {
    pub dispatch_gaps: Option<u64>,
    pub identity_gaps: Option<u64>,
    pub target_gaps: Option<u64>,
    pub time_gaps: Option<u64>,
    pub turn_gaps: Option<u64>,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UseBasis {
    pub method_version: u32,
    pub status: UseBasisStatus,
    pub unit: UseUnit,
    /// Observer cutoff, distinct from individual event times and source dispatch.
    pub captured_at: String,
    pub snapshot_id: Option<String>,
    pub scope: UseScope,
    pub time_basis: UseTimeBasis,
    pub coverage: UseCoverage,
    /// Completeness of selected source reports, not proof of all native use mechanisms.
    pub source_completeness: UseSourceCompleteness,
}
