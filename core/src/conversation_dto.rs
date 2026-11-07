//! Public conversation access contract. Source bodies require explicit read/search.
pub const REQUEST_VERSION: u32 = 7;
pub const OUTPUT_VERSION: u32 = 7;
pub(crate) const MAX_READ_TARGETS: usize = 200;
pub(crate) const MAX_SEARCH_TERMS: usize = 16;
pub(crate) const MAX_TERM_BYTES: usize = 4096;

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Selection {
    /// One canonical operation inside an explicitly selected thread and turn.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operation_id: Option<String>,
    /// Read/search selected tool results on demand; bodies are never indexed.
    #[serde(default)]
    pub include_tool_output: bool,
    pub since: Option<String>,
    pub until: Option<String>,
    pub timezone: Option<String>,
    #[serde(default)]
    pub include_undated: bool,
    #[serde(default)]
    pub sources: Vec<String>,
    #[serde(default)]
    pub projects: Vec<String>,
    #[serde(default)]
    pub threads: Vec<String>,
    #[serde(default)]
    pub turns: Vec<String>,
    #[serde(default)]
    pub origins: Vec<crate::session_events::MessageOrigin>,
    #[serde(default)]
    pub phases: Vec<crate::session_events::ContentPhase>,
    #[serde(default)]
    pub record_kinds: Vec<crate::session_events::MessageRecordKind>,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    /// One source-backed activity per conversation, including work without measurements.
    Records,
    /// Resolve an exact canonical or upstream identity without scanning activity or text.
    Resolve,
    Activity,
    Locate,
    Read,
    Search,
    /// Search visible Codex source text and return excerpts without an index.
    SourceSearch,
    #[schemars(skip)]
    Changes,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Evidence {
    pub event_id: String,
    pub source_instance_id: String,
    pub thread_id: Option<String>,
    pub turn_id: Option<String>,
    /// Exact position in the fixed target partition, not a raw file offset.
    pub event_offset: usize,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReadTarget {
    pub evidence: Evidence,
    pub cursor: Option<String>,
    pub max_bytes: Option<usize>,
    #[serde(default)]
    pub before: usize,
    #[serde(default)]
    pub after: usize,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Request {
    pub action: Action,
    #[serde(default, skip_serializing)]
    #[schemars(skip)]
    pub snapshot_id: String,
    #[serde(default)]
    pub roots: Vec<String>,
    #[serde(default)]
    pub selection: Selection,
    pub limit: Option<usize>,
    /// Skip matching conversations in Records; other actions use bound cursors.
    pub offset: Option<usize>,
    pub cursor: Option<String>,
    pub query: Option<String>,
    /// Search only. Match any of 1–16 literal terms. Supply this or query.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub terms: Option<Vec<String>>,
    #[serde(default, skip_serializing)]
    #[schemars(skip)]
    pub baseline_snapshot_id: Option<String>,
    #[serde(default)]
    pub targets: Vec<ReadTarget>,
    pub max_scan_bytes: Option<u64>,
    /// Use the extended foreground scan budget for search or location.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub complete: bool,
    /// Read this many visible messages on either side of each read target or direct source hit.
    pub context_messages: Option<usize>,
    /// Maximum UTF-8 bytes per direct source hit.
    pub excerpt_bytes: Option<usize>,
    /// Pin an evidence scan to its first observed source scope.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub fixed_scope: bool,
    /// Use the scope returned by the initial scan when reading export evidence.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_scope: Option<String>,
    pub anchor: Option<Evidence>,
    pub radius: Option<usize>,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Activity {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub record: Option<RecordSummary>,
    /// Read-target cursor at a source search match; bound to snapshot, selection and evidence.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub read_cursor: Option<String>,
    /// The literal at the returned read cursor.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub matched_term: Option<String>,
    pub evidence: Evidence,
    pub timestamp: Option<String>,
    pub collected_at: String,
    pub project: Option<String>,
    pub title: Option<String>,
    pub kind: String,
    pub origin: Option<crate::session_events::MessageOrigin>,
    pub phase: Option<crate::session_events::ContentPhase>,
    pub native_message_id: Option<String>,
    pub record_kind: Option<crate::session_events::MessageRecordKind>,
    pub name: Option<String>,
    pub operation_id: Option<String>,
    pub status: Option<String>,
    pub has_text_locator: bool,
    pub content_partial: bool,
    pub change: Option<Change>,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RecordSummary {
    pub upstream_id: String,
    pub started_at: Option<String>,
    pub last_activity_at: Option<String>,
}
#[derive(Clone, Copy, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Change {
    NewSourceFile,
    Appended,
    GenerationChanged,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ContentLocation {
    pub file: String,
    pub byte_start: u64,
    pub byte_end: u64,
    pub part: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Fragment {
    /// Zero-based request target indices served by this occurrence.
    pub target_indices: Vec<usize>,
    pub evidence: Evidence,
    pub message: Activity,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub match_range: Option<MatchRange>,
    pub next_cursor: Option<String>,
    pub locations: Vec<ContentLocation>,
    pub context: Vec<Activity>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visible_context: Option<VisibleContext>,
    pub partial: bool,
    /// Other read targets whose returned text is exactly equal to this text.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub also_at: Vec<FragmentOccurrence>,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FragmentOccurrence {
    pub target_indices: Vec<usize>,
    pub evidence: Evidence,
    pub message: Activity,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub match_range: Option<MatchRange>,
    pub next_cursor: Option<String>,
    pub locations: Vec<ContentLocation>,
    pub context: Vec<Activity>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub visible_context: Option<VisibleContext>,
    pub partial: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct VisibleContext {
    /// Event IDs into the response's shared neighbor bodies, in source order.
    pub before: Vec<String>,
    pub after: Vec<String>,
    pub partial: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Neighbor {
    pub message: Activity,
    pub text: String,
    pub locations: Vec<ContentLocation>,
    pub partial: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceSearchFile {
    pub source_instance_id: String,
    pub thread_id: String,
    pub upstream_id: String,
    pub project: Option<String>,
    pub file: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceHit {
    /// Zero-based index into this response's source files.
    pub file_index: usize,
    pub timestamp: Option<String>,
    pub origin: crate::session_events::MessageOrigin,
    pub byte_start: u64,
    pub byte_end: u64,
    /// Digest of the complete original JSONL record.
    pub sha256: String,
    pub matched_term: String,
    pub match_range: MatchRange,
    pub text: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub context: Option<SourceContext>,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceContext {
    /// Indices into the response's shared source neighbors, in file order.
    pub before: Vec<usize>,
    pub after: Vec<usize>,
    pub partial: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct SourceNeighbor {
    pub file_index: usize,
    pub timestamp: Option<String>,
    pub origin: crate::session_events::MessageOrigin,
    pub byte_start: u64,
    pub byte_end: u64,
    pub sha256: String,
    pub text: String,
    pub partial: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MatchRange {
    /// Start and exclusive end in UTF-8 bytes within the returned text.
    pub start: usize,
    pub end: usize,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ReadIssue {
    pub target_indices: Vec<usize>,
    pub event_id: String,
    pub code: String,
    /// Retry these original targets in a separate batch.
    pub deferred: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ResolvedRecord {
    pub id: String,
    pub upstream_id: String,
    pub source_instance_id: String,
    pub project: Option<String>,
    pub title: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Response {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub record: Option<ResolvedRecord>,
    #[schemars(extend("const" = OUTPUT_VERSION))]
    pub output_version: u32,
    pub action: Action,
    pub snapshot_id: String,
    #[serde(default, skip_serializing)]
    #[schemars(skip)]
    pub baseline_snapshot_id: Option<String>,
    pub selection: Selection,
    /// Explicit roots used for this query, for a search-to-read handoff.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub roots: Vec<String>,
    pub items: Vec<Activity>,
    pub fragments: Vec<Fragment>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub neighbors: Vec<Neighbor>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_hits: Vec<SourceHit>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_files: Vec<SourceSearchFile>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub source_neighbors: Vec<SourceNeighbor>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub next_search: Option<Request>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub read_issues: Vec<ReadIssue>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_scope: Option<String>,
    pub next_cursor: Option<String>,
    /// All matching indexed positions were visited; source completeness is separate.
    pub scan_complete: bool,
    pub partial: bool,
    pub limits: Vec<String>,
    pub scanned_events: usize,
    pub source_bytes_read: u64,
    pub freshness: Option<crate::live::Freshness>,
    /// When present, completeness applies only to this requested source window.
    pub window: Option<ContextWindow>,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ContextWindow {
    pub anchor: Evidence,
    pub radius: usize,
}
