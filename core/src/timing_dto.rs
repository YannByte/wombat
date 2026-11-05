//! Narrow timing protocol. Sharing has an independent whitelist; no source payloads.
pub const REQUEST_VERSION: u32 = 2;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
mod input_change;
mod repeats;
pub use input_change::*;
mod uses;
pub use repeats::*;
pub use uses::*;

#[derive(Clone, Copy, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum SummaryAction {
    Summary,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum EvidenceAction {
    Evidence,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CapabilitiesAction {
    Capabilities,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum LocalProfile {
    Local,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ShareProfile {
    ShareV1,
}
pub const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;
pub const OUTPUT_VERSION: u32 = 8;
pub const METHOD_VERSION: &str = "safe_event_turn_v7";
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum PrivacyProfile {
    #[default]
    Local,
    ShareV1,
}
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Mode {
    #[default]
    Auto,
    Fresh,
    Cached,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Scope {
    pub source_instance_id: Option<String>,
    pub agent_kind: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct Cursor {
    #[schemars(length(max = 8192))]
    pub token: String,
}
fn page_limit() -> usize {
    50
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(
    tag = "action",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum Request {
    Summary {
        thread_id: String,
        turn_id: String,
        #[serde(default, skip_serializing)]
        #[schemars(skip)]
        snapshot_id: Option<String>,
        #[serde(default)]
        roots: Vec<String>,
        scope: Option<Scope>,
        #[serde(default)]
        mode: Mode,
        #[serde(default)]
        privacy_profile: PrivacyProfile,
    },
    Evidence {
        thread_id: String,
        turn_id: String,
        #[serde(default, skip_serializing)]
        #[schemars(skip)]
        snapshot_id: String,
        #[serde(default)]
        roots: Vec<String>,
        scope: Option<Scope>,
        cursor: Option<Cursor>,
        #[serde(default = "page_limit")]
        #[schemars(range(min = 1, max = 200))]
        limit: usize,
        #[serde(default)]
        collection: EvidenceSet,
        object_ref: Option<String>,
        #[serde(default)]
        privacy_profile: PrivacyProfile,
    },
    Capabilities {
        #[serde(default)]
        privacy_profile: PrivacyProfile,
    },
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum MetricStatus {
    Observed,
    Derived,
    Proxy,
    Unavailable,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Basis {
    NativeRecord,
    ExplicitBoundary,
    LifecycleUnion,
    LifecycleSum,
    IntervalMask,
    OperationUnion,
    OperationResidual,
    RepeatAfterFailure,
    SuccessfulReadRepeat,
    KnownOperationDuration,
    FailureRecoverySpan,
    SameRequestObservation,
    RequestInput,
    HistoricalWindow,
    Type7,
    SafeMessageRecord,
    SafeMessageDelay,
    SafeEventCount,
    ResponseGapV1,
    NotRecorded,
    AdapterNotMapped,
    UnsupportedMethod,
    MissingIdentity,
    MissingTime,
    RunningTurn,
    ExactEventPage,
    BoundaryConflict,
    SourcePartial,
    ResourceLimit,
    NumericRange,
    NoCandidates,
    MissingBatchCycle,
    MissingRepositoryBaseline,
    UnknownMessageOrigin,
    CanonicalOperationIdentity,
    DeterminateTerminalOutcomes,
    ReportedFilePaths,
    CanonicalUseIdentity,
    CanonicalUseRecords,
    UnassignedUseIndex,
    DispatchNotProven,
    MissingTarget,
    TargetConflict,
    OutcomeConflict,
    MissingTurn,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
#[schemars(bound = "T: JsonSchema", rename = "TimingMetric_{T}")]
pub struct Metric<T> {
    #[schemars(schema_with = "numeric_value_schema::<T>")]
    pub value: Option<T>,
    pub status: MetricStatus,
    pub basis: Basis,
    pub evidence_refs: Vec<String>,
}
pub type Count = Metric<u64>;
pub type Number = Metric<f64>;
pub type Signed = Metric<i64>;
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Support {
    Supported,
    Partial,
    Unavailable,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Capability {
    pub support: Support,
    pub reason: Basis,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Capabilities {
    pub wall_clock: Capability,
    pub native_ttft: Capability,
    pub first_content_record_delay: Capability,
    pub lifecycle_intervals: Capability,
    pub operation_intervals: Capability,
    pub context_pressure: Capability,
    pub strict_response_gap: Capability,
    pub exploratory_gap: Capability,
    pub command_labels: Capability,
    pub file_changes: Capability,
    pub message_records: Capability,
    pub object_uses: Capability,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TurnState {
    Running,
    Completed,
    Failed,
    Cancelled,
    Unknown,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Anchors {
    pub start_ms: Signed,
    pub end_ms: Signed,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Category {
    pub candidates: Count,
    pub closed: Count,
    pub union_ms: Count,
    pub sum_ms: Count,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TimelinePresentation {
    Timeline,
    List,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum TrackCategory {
    Command,
    Compaction,
    Reasoning,
    Mcp,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FragmentEvidence {
    EventRecords,
    TurnCollection,
    Unavailable,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TimelineTrack {
    pub interval_alias: String,
    pub category: TrackCategory,
    #[schemars(range(min = 0, max = 9007199254740991u64))]
    pub start_ms: u64,
    #[schemars(range(min = 0, max = 9007199254740991u64))]
    pub end_ms: u64,
    pub clipped: bool,
    pub evidence_scope: FragmentEvidence,
    #[schemars(length(max = 3))]
    pub evidence_refs: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TimelineGap {
    #[schemars(range(min = 0, max = 9007199254740991u64))]
    pub start_ms: u64,
    #[schemars(range(min = 0, max = 9007199254740991u64))]
    pub end_ms: u64,
    pub evidence_scope: FragmentEvidence,
    pub evidence_refs: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Timeline {
    pub presentation: TimelinePresentation,
    pub detail: Capability,
    pub entry_count: Count,
    pub track_count: Count,
    /// Majority/list denominator: unique identified lifecycle intervals, including
    /// excluded conflicts; records without identity are counted only in coverage.
    pub identified_interval_count: Count,
    pub unclassified_gap_count: Count,
    /// Unique identified intervals that cannot be placed; missing-identity records
    /// remain in coverage and are not guessed to be separate intervals.
    pub unlocated_interval_count: Count,
    pub outside_window_interval_count: Count,
    #[schemars(range(min = 200, max = 200))]
    pub detail_limit: usize,
    #[schemars(length(max = 200))]
    pub tracks: Vec<TimelineTrack>,
    #[schemars(length(max = 200))]
    pub unclassified_gaps: Vec<TimelineGap>,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum OperationCoverageReason {
    NoPairedOperations,
    MissingWindow,
    UnlocatedOperations,
    IdentityGaps,
    ConflictingOperations,
    SourcePartial,
    ResourceLimit,
    NumericRange,
    DetailLimit,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OperationResidualRange {
    pub start_ms: u64,
    pub end_ms: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OperationCoverage {
    #[schemars(range(min = 1, max = 1))]
    pub method_version: u32,
    #[schemars(range(min = 1, max = 1))]
    pub endpoint_method_version: u32,
    pub candidate_operations: Count,
    pub paired_operations: Count,
    pub identity_gap_records: Count,
    pub conflicting_operations: Count,
    pub covered_ms: Count,
    pub residual_ms: Count,
    pub residual_range_count: Count,
    pub partial: bool,
    pub reason_codes: Vec<OperationCoverageReason>,
    pub detail: Capability,
    #[schemars(range(min = 200, max = 200))]
    pub detail_limit: usize,
    #[schemars(length(max = 200))]
    pub residual_ranges: Vec<OperationResidualRange>,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Time {
    pub repeated_behavior: RepeatedBehavior,
    pub operation_coverage: OperationCoverage,
    pub timeline: Timeline,
    pub state: TurnState,
    pub native_wall_clock_ms: Count,
    pub derived_wall_clock_ms: Count,
    pub native_ttft_ms: Count,
    pub first_content_record_delay_ms: Count,
    pub boundary_discrepancy_ms: Signed,
    pub observed_window_ms: Count,
    pub command: Category,
    pub compaction: Category,
    pub reasoning: Category,
    pub mcp: Category,
    /// Masks 0..15: command bit 1, compaction bit 2, reasoning bit 4, MCP bit 8.
    #[schemars(length(min = 16, max = 16))]
    pub intersection_masks_ms: Vec<Count>,
    pub covered_ms: Count,
    pub unclassified_ms: Count,
    pub coverage_ratio: Number,
    pub waiting_proxy_ms: Count,
    pub strict_response_gap_ms: Count,
    pub exploratory_gap_ms: Count,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Distribution {
    pub samples: Count,
    pub median: Number,
    pub p90: Number,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Segment {
    pub id: String,
    pub candidates: Count,
    pub non_request_scoped: Count,
    pub missing_raw_input: Count,
    pub missing_window: Count,
    pub invalid_window: Count,
    pub same_record_windows: Count,
    pub continued_windows: Count,
    pub above_window: Count,
    pub input: Distribution,
    pub ratio: Distribution,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Neighbor {
    pub measurement_ref: String,
    pub raw_input: Count,
    pub ratio: Number,
    pub distance_ms: Count,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CompactionNeighbors {
    pub evidence_ref: String,
    pub segment_id: String,
    pub before: Option<Neighbor>,
    pub after: Option<Neighbor>,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Context {
    pub input_change: InputChange,
    pub active_context_occupancy: Number,
    pub compaction_records: Count,
    pub compaction_time_ms: Count,
    pub method: String,
    pub quantile_method: String,
    pub candidates: Count,
    pub conflicting_measurements: Count,
    pub conflicting_window_records: Count,
    pub input: Distribution,
    pub ratio: Distribution,
    pub segment_count: Count,
    pub segments: Vec<Segment>,
    pub compaction_neighbors: Vec<CompactionNeighbors>,
    pub detail: Capability,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum OutcomeMethod {
    TerminalSuccessFailureSubsetV1,
}
/// Counts in the captured subset, rather than inferred complete-turn totals.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OutcomeStatistics {
    pub method: OutcomeMethod,
    pub determinate_operations: Count,
    pub succeeded: Count,
    pub failed: Count,
    pub interrupted: Count,
    pub rejected: Count,
    pub nonterminal: Count,
    pub indeterminate: Count,
    pub conflicting: Count,
    /// Missing-identity observations use physical-record units, unlike canonical group counts.
    pub identity_gap_records: Count,
    pub unclassified: Count,
    pub failure_ratio: Number,
    pub partial: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Work {
    pub outcomes: OutcomeStatistics,
    pub operation_candidates: Count,
    pub closed_operations: Count,
    pub failed_operations: Count,
    pub labelled_command_ms: Count,
    pub file_change_records: Count,
    pub changed_files: Count,
    pub added_lines: Count,
    pub removed_lines: Count,
    pub message_record_candidates: Count,
    pub nonempty_visible_content_records: Count,
    pub unknown_content_records: Count,
    pub missing_content_time_records: Count,
    pub user_boundary_records: Count,
    pub injected_context_records: Count,
    pub reasoning_message_records: Count,
    pub compaction_records: Count,
    pub repository_baseline: Capability,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum FindingKind {
    Fact,
    Proxy,
    UserAnnotation,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Finding {
    pub code: String,
    pub kind: FindingKind,
    pub metric_refs: Vec<String>,
    pub evidence_refs: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Coverage {
    pub facts: Count,
    pub bytes: Count,
    pub metadata: Count,
    pub event_blocks: Count,
    pub scoped_events: Count,
    pub scoped_measurements: Count,
    pub boundary_candidates: Count,
    #[schemars(length(min = 4, max = 4))]
    pub lifecycle_candidates: Vec<Count>,
    #[schemars(length(min = 4, max = 4))]
    pub linked_lifecycles: Vec<Count>,
    pub conflicting_lifecycles: Count,
    pub missing_identity_lifecycles: Count,
    pub content_candidates: Count,
    pub domain_count: Count,
    pub missing_watermarks: Count,
    pub generation_mismatches: Count,
    pub incomplete_domains: Count,
    pub snapshot_unassigned_total: Count,
    pub thread_unassigned_total: Count,
    pub source_status: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Quality {
    pub partial: bool,
    pub running: bool,
    pub censored: bool,
    pub reason_codes: Vec<Basis>,
    pub fact_limit: usize,
    pub summary_limit_bytes: usize,
}
/// Internal selector observation; not a request option or source capability.
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct QueryFreshness {
    pub status: String,
    pub checked_at: Option<String>,
    pub revision: Option<u64>,
    pub error_code: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ShareFreshness {
    pub status: String,
    pub error_code: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReadView {
    pub snapshot_id: String,
    pub snapshot_schema: u32,
    pub created_at: String,
    pub adapter_versions: Vec<String>,
    pub projection_version: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LocalScope {
    pub source_instance_id: String,
    pub thread_id: String,
    pub turn_id: String,
    pub agent_kind: String,
    pub whole_turn: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ShareScope {
    pub task_alias: String,
    pub turn_alias: String,
    pub whole_turn: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Privacy {
    pub profile: PrivacyProfile,
    pub omitted_fields: Vec<String>,
    pub aliases: String,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum CollectionKind {
    TurnEvents,
    CanonicalMeasurements,
    CanonicalOperations,
    SourceControls,
    NativeBoundaryIndex,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EvidenceCollection {
    pub reference: String,
    pub kind: CollectionKind,
    pub snapshot_id: String,
    pub scope: LocalScope,
    pub count: Count,
    pub method: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ShareCollection {
    pub reference: String,
    pub kind: CollectionKind,
    pub task_alias: String,
    pub turn_alias: String,
    pub count: Count,
    pub method: String,
}
/// Local navigation only: the sharing DTO has no evidence index or cursor type.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct IntervalPage {
    pub interval_alias: String,
    #[schemars(length(max = 3))]
    pub pages: Vec<FragmentPage>,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FragmentPage {
    pub cursor: Option<Cursor>,
    #[schemars(range(min = 200, max = 200))]
    pub limit: usize,
    #[schemars(length(max = 3))]
    pub evidence_refs: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct IntervalPages {
    pub detail: Capability,
    pub candidate_interval_count: Count,
    pub located_interval_count: Count,
    pub missing_event_ref_count: Count,
    /// Sum of page locators over intervals, after merging each interval's same-page refs.
    pub page_count: Count,
    #[schemars(range(min = 65536, max = 65536))]
    pub limit_bytes: usize,
    #[schemars(length(max = 200))]
    pub entries: Vec<IntervalPage>,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EvidenceIndex {
    pub repeat_pages: RepeatPages,
    pub interval_pages: IntervalPages,
    pub collections: Vec<EvidenceCollection>,
    pub available: bool,
    pub limit: usize,
    pub snapshot_id: String,
    pub refs: Vec<String>,
    pub method: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct LocalResponse {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operation_diagnostics: Option<OperationDiagnostics>,
    pub uses: LocalUses,
    #[schemars(extend("const" = OUTPUT_VERSION))]
    pub output_version: u32,
    pub action: SummaryAction,
    pub method_version: String,
    pub profile: LocalProfile,
    pub privacy: Privacy,
    #[serde(rename = "basis")]
    pub read_view: ReadView,
    pub scope: LocalScope,
    pub capabilities: Capabilities,
    pub anchors: Anchors,
    pub time: Time,
    pub context: Context,
    pub work: Work,
    pub findings: Vec<Finding>,
    pub coverage: Coverage,
    pub quality: Quality,
    pub freshness: QueryFreshness,
    pub evidence: EvidenceIndex,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum OperationDiagnosticReason {
    IdentityConflict,
    TimeConflict,
    KindConflict,
    TargetConflict,
    OutcomeConflict,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OperationDiagnostic {
    pub operation_id: String,
    pub reasons: Vec<OperationDiagnosticReason>,
    pub observation_count: usize,
    #[schemars(length(max = 4))]
    pub event_refs: Vec<String>,
    #[schemars(length(max = 4))]
    pub evidence: Vec<crate::adapters::contract::EvidenceRef>,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OperationDiagnostics {
    pub total: usize,
    pub complete: bool,
    #[schemars(length(max = 8))]
    pub records: Vec<OperationDiagnostic>,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ShareResponse {
    pub uses: UseTotals,
    pub basis_collections: Vec<ShareCollection>,
    #[schemars(extend("const" = OUTPUT_VERSION))]
    pub output_version: u32,
    pub action: SummaryAction,
    pub method_version: String,
    pub profile: ShareProfile,
    pub privacy: Privacy,
    pub scope: ShareScope,
    pub capabilities: Capabilities,
    pub relative_anchors: Anchors,
    pub time: Time,
    pub context: Context,
    pub work: Work,
    pub findings: Vec<Finding>,
    pub coverage: Coverage,
    pub quality: Quality,
    pub freshness: ShareFreshness,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EvidenceRow {
    pub reference: String,
    pub record_kind: String,
    pub timestamp_ms: Option<i64>,
    pub phase: Option<String>,
    pub presence: Option<String>,
    pub origin: Option<String>,
    pub duration_ms: Option<u64>,
    pub first_token_ms: Option<u64>,
    pub gap_codes: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct EvidenceResponse {
    pub collection: EventPageKind,
    #[schemars(extend("const" = OUTPUT_VERSION))]
    pub output_version: u32,
    pub action: EvidenceAction,
    pub method_version: String,
    pub profile: LocalProfile,
    pub snapshot_id: String,
    pub scope: LocalScope,
    pub total: Count,
    pub rows: Vec<EvidenceRow>,
    pub next_cursor: Option<Cursor>,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CapabilitiesResponse {
    #[schemars(extend("const" = OUTPUT_VERSION))]
    pub output_version: u32,
    pub action: CapabilitiesAction,
    pub method_version: String,
    pub profile: PrivacyProfile,
    pub capabilities: Capabilities,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(untagged)]
pub enum Response {
    Local(Box<LocalResponse>),
    Share(Box<ShareResponse>),
    Evidence(EvidenceResponse),
    UseObjects(UseObjectsResponse),
    UseRecords(UseRecordsResponse),
    Capabilities(CapabilitiesResponse),
}

/// CLI error output; core/socket transports retain their standard failure envelope.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TimingErrorOutput {
    #[schemars(range(min = 1, max = 1))]
    pub output_version: u32,
    pub error: TimingError,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct TimingError {
    pub code: String,
    pub message: String,
}

fn numeric_value_schema<T: JsonSchema>(
    generator: &mut schemars::SchemaGenerator,
) -> schemars::Schema {
    let mut schema = generator.subschema_for::<Option<T>>();
    // Ratios/Type7 use JSON numbers. Integer measures require the safe JS range.
    if std::any::type_name::<T>() == "u64" || std::any::type_name::<T>() == "i64" {
        schema
            .ensure_object()
            .insert("maximum".into(), serde_json::json!(MAX_SAFE_INTEGER));
        let minimum = if std::any::type_name::<T>() == "i64" {
            -(MAX_SAFE_INTEGER as i64)
        } else {
            0
        };
        schema
            .ensure_object()
            .insert("minimum".into(), serde_json::json!(minimum));
    }
    schema
}
