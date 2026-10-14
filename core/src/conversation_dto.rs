//! Public conversation access contract. Source bodies require explicit read/search.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Selection {
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
    Activity,
    Locate,
    Read,
    Search,
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
    pub snapshot_id: String,
    #[serde(default)]
    pub roots: Vec<String>,
    #[serde(default)]
    pub selection: Selection,
    pub limit: Option<usize>,
    pub cursor: Option<String>,
    pub query: Option<String>,
    pub baseline_snapshot_id: Option<String>,
    #[serde(default)]
    pub targets: Vec<ReadTarget>,
    pub max_scan_bytes: Option<u64>,
    pub anchor: Option<Evidence>,
    pub radius: Option<usize>,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Activity {
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
    pub evidence: Evidence,
    pub text: String,
    pub next_cursor: Option<String>,
    pub locations: Vec<ContentLocation>,
    pub context: Vec<Activity>,
    pub partial: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Response {
    #[schemars(range(min = 1, max = 1))]
    pub output_version: u32,
    pub action: Action,
    pub snapshot_id: String,
    pub baseline_snapshot_id: Option<String>,
    pub selection: Selection,
    pub items: Vec<Activity>,
    pub fragments: Vec<Fragment>,
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
