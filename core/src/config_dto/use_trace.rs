//! Bounded, body-free explanations from the same projection that owns use counts.
use super::{Kind, Scope};
use crate::{adapters::contract::EvidenceRef, usage_app_dto::Page};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(
    Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize, JsonSchema,
)]
#[serde(rename_all = "snake_case")]
pub enum UseReason {
    Counted,
    IdentityMissing,
    TimeMissing,
    DispatchUnknown,
    TargetMissing,
    TargetNotFound,
    TargetAmbiguous,
    IdentityConflict,
    Duplicate,
    OutsideWindow,
    OutsideProject,
    CatalogOnly,
    InstructionLoad,
    NotUse,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UseTraceRecord {
    pub operation_id: String,
    pub source_instance_id: String,
    pub thread_id: String,
    pub turn_id: Option<String>,
    pub item_id: Option<String>,
    pub timestamp: Option<String>,
    pub event_type: String,
    pub outcome: String,
    pub reason: UseReason,
    /// None means the evidence cannot establish a numeric contribution.
    pub count_delta: Option<u64>,
    pub evidence: Option<EvidenceRef>,
    pub source_observation_count: usize,
    /// Up to three further source observations; phases are not independent uses.
    pub additional_evidence: Vec<EvidenceRef>,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UseReasonCount {
    pub reason: UseReason,
    pub records: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UseReplayBaseline {
    #[schemars(range(min = 1, max = 1))]
    pub format_version: u32,
    pub method_version: u32,
    pub scope: Scope,
    pub roots: Vec<String>,
    pub project_roots: Vec<String>,
    pub kind: Option<Kind>,
    pub item_id: Option<String>,
    pub unassigned: bool,
    /// Normalized input facts, not raw source bodies or a guarantee of native coverage.
    pub input_revision: String,
    pub configuration_revision: String,
    pub result_revision: String,
    pub counted: u64,
    pub total_records: usize,
    pub reasons: Vec<UseReasonCount>,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UseComparisonStatus {
    Unchanged,
    Changed,
    InputChanged,
    ScopeChanged,
    ConfigurationChanged,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UseComparison {
    pub status: UseComparisonStatus,
    pub before: u64,
    pub after: u64,
    /// Available only when scope, normalized evidence and configuration are identical.
    pub delta: Option<i64>,
    pub changes: Vec<UseRecordChange>,
    pub changes_complete: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UseRecordChange {
    pub operation_id: String,
    pub thread_id: String,
    pub item_id: Option<String>,
    pub before: Option<UseReason>,
    pub after: Option<UseReason>,
    pub before_delta: Option<u64>,
    pub after_delta: Option<u64>,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UseExplanation {
    pub baseline: UseReplayBaseline,
    /// Counts and digests cover the whole selection; only this page is retained.
    pub records: Vec<UseTraceRecord>,
    pub page: Page,
    pub comparison: Option<UseComparison>,
    /// Source-level diagnostics can extend beyond the selected dates. No raw messages.
    pub source_issues: Vec<UseSourceIssue>,
    pub source_issues_total: usize,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UseSourceIssue {
    pub code: String,
    pub source_instance_id: Option<String>,
    pub evidence: Option<EvidenceRef>,
}
