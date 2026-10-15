//! Read-only configuration analysis. Counts are evidence, never exclusive cost attribution.
use crate::usage_app_dto::{Page, UsageSummary};
mod use_basis;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
pub use use_basis::*;

#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    #[default]
    List,
    Detail,
    Evidence,
    RelatedScopes,
    Capabilities,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    Rule,
    Skill,
    Mcp,
    Hook,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Observation {
    Used,
    LoadedOnly,
    Unknown,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Scope {
    pub all_time: Option<bool>,
    pub since: Option<String>,
    pub until: Option<String>,
    pub timezone: Option<String>,
    pub project: Option<String>,
    pub agent_kind: Option<String>,
    pub source_instance_id: Option<String>,
    pub thread_id: Option<String>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Sort {
    #[default]
    Tokens,
    Activity,
    Size,
    Name,
    ContentTokens,
    Characters,
    Recent,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Request {
    #[serde(default)]
    pub action: Action,
    pub roots: Option<Vec<String>>,
    pub project_roots: Option<Vec<String>>,
    pub snapshot_id: Option<String>,
    pub read_view: Option<String>,
    /// Check current configuration against committed usage without scheduling collection.
    #[serde(default)]
    pub cached_usage: bool,
    #[serde(default)]
    pub scope: Scope,
    pub kind: Option<Kind>,
    pub kinds: Option<Vec<Kind>>,
    pub observation: Option<Observation>,
    pub search: Option<String>,
    #[serde(default)]
    pub sort: Sort,
    pub item_id: Option<String>,
    pub offset: Option<usize>,
    pub limit: Option<usize>,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Capabilities {
    pub historical_content_hashes: bool,
    pub kinds: Vec<Kind>,
    pub evidence_types: Vec<String>,
    pub token_estimates: bool,
    pub historical_content: bool,
    pub writes: bool,
    pub project_registry: bool,
}
impl Default for Capabilities {
    fn default() -> Self {
        Self {
            historical_content_hashes: true,
            kinds: vec![Kind::Rule, Kind::Skill, Kind::Mcp, Kind::Hook],
            evidence_types: vec![
                "file_read".into(),
                "skill_available".into(),
                "skill_use".into(),
                "tool_call".into(),
                "resource_read".into(),
            ],
            token_estimates: true,
            historical_content: false,
            writes: false,
            project_registry: false,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Item {
    pub id: String,
    pub name: String,
    pub kind: Kind,
    pub source_instance_id: String,
    pub path: String,
    pub project: Option<String>,
    pub native_key: Option<String>,
    /// Authorized inventory memberships, not proof of joint host loading.
    pub authorized_projects: Vec<String>,
    /// Current physical object keeps each source inventory identity and observation.
    pub source_contexts: Vec<SourceContext>,
    pub configured_state: String,
    pub content_hash: String,
    pub observed_at: String,
    pub current: bool,
    pub stale: bool,
    pub bytes: Option<u64>,
    pub content_tokens: Option<u64>,
    pub estimate_status: String,
    pub characters: Option<u64>,
    pub measurement_status: String,
    pub bytes_source: Option<String>,
    pub estimate: Option<ContentEstimate>,
    pub skill_metadata: Option<SkillMetadata>,
    pub body_token_estimate: Option<ContentEstimate>,
    pub body_estimate_status: String,
    pub usage_count: Option<u64>,
    pub use_basis: Option<UseBasis>,
    pub last_record_at: Option<String>,
    pub observation: Observation,
    pub counts: Counts,
    pub related_turns: usize,
    pub related_tasks: usize,
    pub usage: Option<UsageSummary>,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SourceContext {
    pub inventory_id: String,
    pub global: bool,
    pub source_instance_id: String,
    pub content_hash: String,
    pub configured_state: String,
    pub counts: Counts,
    pub observation: Observation,
    pub last_record_at: Option<String>,
}
impl Item {
    pub(crate) fn source_ids(&self) -> impl Iterator<Item = &str> {
        std::iter::once(self.source_instance_id.as_str()).chain(
            self.source_contexts
                .iter()
                .map(|c| c.source_instance_id.as_str())
                .filter(|id| *id != self.source_instance_id),
        )
    }
    pub(crate) fn in_source(&self, source: &str) -> bool {
        self.source_ids().any(|id| id == source)
    }
    pub(crate) fn applies(&self, source: Option<&str>, project: Option<&str>) -> bool {
        let membership = project.is_none_or(|p| {
            self.authorized_projects.iter().any(|v| v == p) || self.project.as_deref() == Some(p)
        });
        if self.source_contexts.is_empty() {
            return source.is_none_or(|s| s == self.source_instance_id)
                && (self.project.is_none() || membership);
        }
        self.source_contexts
            .iter()
            .any(|c| source.is_none_or(|s| s == c.source_instance_id) && (c.global || membership))
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ContentEstimate {
    pub tokens: u64,
    pub encoding: String,
    pub method: String,
    pub payload: String,
    pub content_hash: String,
    pub applicability: String,
    #[serde(default)]
    pub tokenizer_version: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SkillMetadata {
    pub status: String,
    pub description_characters: Option<u64>,
    pub issues: Vec<String>,
    /// Bounded current-file diagnostics for directly reviewable static fields.
    pub diagnostics: Vec<SkillDiagnostic>,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SkillDiagnostic {
    pub code: String,
    pub field: Option<String>,
    pub line: Option<usize>,
    pub column: Option<usize>,
    pub current: Option<String>,
    pub expected: Option<String>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Counts {
    pub file_reads: u64,
    pub tool_calls: u64,
    pub resource_reads: u64,
    pub succeeded: u64,
    pub failed: u64,
    pub outcome_unknown: u64,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Evidence {
    pub id: String,
    #[serde(default)]
    pub source_instance_id: Option<String>,
    pub item_id: String,
    pub thread_id: String,
    pub turn_id: Option<String>,
    pub title: Option<String>,
    pub project: Option<String>,
    pub timestamp: Option<String>,
    pub event_type: String,
    pub outcome: String,
    pub association: String,
    pub usage: Option<UsageSummary>,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Issue {
    pub code: String,
    pub path: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Coverage {
    pub status: String,
    pub history_status: String,
    pub issues: Vec<Issue>,
    pub supported_evidence: Vec<String>,
    pub absence_observable: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Summary {
    pub current_items: usize,
    pub historical_items: usize,
    pub observed_items: usize,
    pub usage: Option<UsageSummary>,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RelatedScope {
    pub project: Option<String>,
    pub evidence_count: usize,
    pub usage: Option<UsageSummary>,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Response {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub extension_activity: Option<ExtensionActivityStatistics>,
    pub output_version: u32,
    pub action: Action,
    pub capabilities: Capabilities,
    pub read_view: Option<String>,
    pub usage_revision: Option<String>,
    pub config_revision: String,
    pub checked_at: String,
    pub scope: Scope,
    pub authorized_projects: Vec<String>,
    /// Startup read authorization, not proof that logs exist or are readable.
    #[serde(default)]
    pub authorized_source_roots: Vec<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub host_restart_command: Option<String>,
    pub summary: Summary,
    pub items: Vec<Item>,
    pub evidence: Vec<Evidence>,
    pub related_scopes: Vec<RelatedScope>,
    pub page: Page,
    pub coverage: Coverage,
    pub hook_registry: HookRegistry,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionActivity {
    pub item_id: String,
    pub observed_records: Option<u64>,
    /// Whole elapsed 24-hour periods without a recorded use in the selected window.
    /// Does not establish continuous enablement, loading, or complete observation.
    pub no_observed_use_days: Option<u64>,
    pub last_record_at: Option<String>,
    pub absence_observable: bool,
    pub use_basis: Option<UseBasis>,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionActivityStatistics {
    #[schemars(range(min = 1, max = 1))]
    pub method_version: u32,
    pub observed_use: usize,
    pub no_observed_use: usize,
    pub unavailable: usize,
    pub scope: Scope,
    pub checked_at: String,
    /// Same page as inventory items; summary counts are computed before pagination.
    pub items: Vec<ExtensionActivity>,
}

/// A current native registry observation; it never proves that a Hook ran.
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct HookRegistry {
    pub native_version: Option<String>,
    pub checked_at: Option<String>,
    pub status: HookRegistryStatus,
    pub contexts: Vec<HookContext>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HookRegistryStatus {
    #[default]
    Unavailable,
    Partial,
    Observed,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct HookContext {
    pub project: String,
    pub complete: bool,
    pub registrations: Vec<HookRegistration>,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct HookRegistration {
    pub item_id: String,
    pub native_key: String,
    pub content_hash: String,
    pub registration_hash: String,
    pub enabled: bool,
    pub trust: HookTrust,
    pub handler: HookHandler,
    pub source: String,
    pub plugin_id: Option<String>,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HookTrust {
    Managed,
    Untrusted,
    Trusted,
    Modified,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum HookHandler {
    Command,
    McpTool,
    Prompt,
    Agent,
}
