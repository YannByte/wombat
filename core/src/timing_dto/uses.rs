//! Local object and record navigation; the share whitelist contains numeric totals only.
use super::*;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceSet {
    #[default]
    TurnEvents,
    UseObjects,
    UseRecords,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EventPageKind {
    TurnEvents,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum UseObjectPageKind {
    UseObjects,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum UseRecordPageKind {
    UseRecords,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum UseSourceCoverage {
    Complete,
    Partial,
    Unknown,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum UseObjectKind {
    Skill,
    Mcp,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum UseState {
    Used,
    Candidate,
    Unclassified,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum UseKind {
    SkillRead,
    McpTool,
    McpResource,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum UseTimeBasis {
    SourceOperationTime,
    Unknown,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum UseOutcome {
    Running,
    Completed,
    Failed,
    Cancelled,
    Interrupted,
    Declined,
    Unknown,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UseCoverage {
    pub dispatch_gaps: Count,
    pub identity_gaps: Count,
    pub target_gaps: Count,
    pub time_gaps: Count,
    /// Gaps among associated records; unassigned membership is separately reported below.
    pub associated_turn_gaps: Count,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UseTotals {
    pub method_version: u32,
    pub source_coverage: UseSourceCoverage,
    pub object_count: Count,
    /// Canonical rows, including replay and candidate evidence; not a dispatch count.
    pub record_count: Count,
    pub unbound_target_records: Count,
    pub unassigned_skill_records: Count,
    pub unassigned_mcp_records: Count,
    pub coverage: UseCoverage,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UseObject {
    pub object_ref: String,
    pub kind: UseObjectKind,
    pub state: UseState,
    /// Resolved historical local target; never sent by share-v1.
    pub path: Option<String>,
    pub server: Option<String>,
    pub project: Option<String>,
    /// Exact count in the positively associated canonical set; coverage explains local gaps.
    pub associated_use_count: Count,
    /// Complete turn count is unavailable when association or unassigned-membership evidence has gaps.
    pub use_count: Count,
    pub record_count: Count,
    pub unassigned_turn_records: Count,
    pub coverage: UseCoverage,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LocalUses {
    pub totals: UseTotals,
    pub detail: Capability,
    #[schemars(range(min = 50, max = 50))]
    pub limit: usize,
    #[schemars(length(max = 50))]
    pub objects: Vec<UseObject>,
    pub next_cursor: Option<Cursor>,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UseRecord {
    pub reference: String,
    pub object_ref: Option<String>,
    pub kind: Option<UseKind>,
    pub state: UseState,
    pub outcome: UseOutcome,
    pub timestamp_ms: Option<i64>,
    pub time_basis: UseTimeBasis,
    pub native_duration_ms: Option<u64>,
    pub tool: Option<String>,
    pub exit_code: Option<i64>,
    pub identity_known: bool,
    pub replay_of: Option<String>,
    pub target_conflict: bool,
    pub gap_codes: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UseObjectsResponse {
    #[schemars(range(min = 7, max = 7))]
    pub output_version: u32,
    pub action: EvidenceAction,
    pub collection: UseObjectPageKind,
    pub method_version: String,
    pub profile: LocalProfile,
    pub snapshot_id: String,
    pub scope: LocalScope,
    pub totals: UseTotals,
    pub total: Count,
    #[schemars(length(max = 200))]
    pub rows: Vec<UseObject>,
    pub next_cursor: Option<Cursor>,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UseRecordsResponse {
    #[schemars(range(min = 7, max = 7))]
    pub output_version: u32,
    pub action: EvidenceAction,
    pub collection: UseRecordPageKind,
    pub method_version: String,
    pub profile: LocalProfile,
    pub snapshot_id: String,
    pub scope: LocalScope,
    pub object_ref: Option<String>,
    pub totals: UseTotals,
    /// Full selected record count; the whole-turn totals do not shrink with this object page.
    pub total: Count,
    #[schemars(length(max = 200))]
    pub rows: Vec<UseRecord>,
    pub next_cursor: Option<Cursor>,
}
