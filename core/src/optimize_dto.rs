//! Narrow, object-bound review operations. No source writes or command execution.
pub const REQUEST_VERSION: u32 = 2;

use crate::config_dto::{Issue, Item};
use crate::usage_app_dto::Page;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
pub const OUTPUT_VERSION: u32 = 6;
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    #[default]
    List,
    Detail,
    Keep,
    NotApplicable,
    Recheck,
    Capabilities,
    Checks,
    Activity,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Request {
    #[serde(default)]
    pub action: Action,
    pub roots: Option<Vec<String>>,
    pub project_roots: Option<Vec<String>>,
    #[serde(default, skip_serializing)]
    #[schemars(skip)]
    pub read_view: Option<String>,
    pub baseline_id: Option<String>,
    pub baseline: Option<Box<Suggestion>>,
    pub decision_revision: Option<String>,
    pub project: Option<String>,
    pub source_instance_id: Option<String>,
    pub suggestion_id: Option<String>,
    pub item_id: Option<String>,
    pub decision_reason: Option<DecisionReason>,
    pub category: Option<Category>,
    pub offset: Option<usize>,
    pub limit: Option<usize>,
    pub rule_overrides: Option<RuleOverrides>,
    pub activity: Option<ActivitySelection>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuleOverrides {
    pub agents_bytes: Option<u64>,
    pub description_characters: Option<u64>,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RuleParameters {
    pub version: String,
    pub agents_bytes_default: u64,
    pub description_characters_default: u64,
    pub overrides: RuleOverrides,
    pub body_tokens: u64,
    pub description_standard_max: u64,
    pub applicability: String,
}
impl Default for RuleParameters {
    fn default() -> Self {
        Self {
            version: "static-config-v7".into(),
            agents_bytes_default: 16384,
            description_characters_default: 500,
            overrides: RuleOverrides::default(),
            body_tokens: 5000,
            description_standard_max: 1024,
            applicability: "authorizedCurrentConfigurationOnly".into(),
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Category {
    Repair,
    Trim,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Finding {
    pub identity: FindingIdentity,
    pub rule: String,
    pub status: String,
    pub observed: Option<u64>,
    pub threshold: Option<u64>,
    pub evidence_codes: Vec<String>,
    pub basis: Option<String>,
    /// Positions and relationships only; never retain source text or command arguments.
    pub evidence: Option<StaticEvidence>,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct StaticEvidence {
    pub method: String,
    pub applicability: String,
    pub declaration_hash: Option<String>,
    pub relation_id: Option<String>,
    pub direction: Option<String>,
    pub transform: Option<String>,
    pub versions: Vec<FileVersion>,
    pub positions: Vec<BlockPosition>,
    /// Scope-bound owner required to prove a declared relation.
    pub relation: Option<RelationIdentity>,
    pub references: Vec<ReferenceEvidence>,
    pub hook: Option<HookTargetEvidence>,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct HookTargetEvidence {
    pub project: String,
    pub native_key: String,
    pub registration_hash: String,
    pub host_version: String,
    pub trust: crate::config_dto::HookTrust,
    pub target: String,
    pub status: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReferenceEvidence {
    pub target: String,
    pub base_directory: String,
    pub expected_type: Option<String>,
    pub status: String,
    pub start_byte: usize,
    pub end_byte: usize,
    pub start_line: usize,
    pub end_line: usize,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RuleOutcome {
    Hit,
    Miss,
    Insufficient,
    Unsupported,
    Error,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RuleDefinition {
    pub rule: String,
    pub version: String,
    pub kinds: Vec<crate::config_dto::Kind>,
    pub basis: String,
}

/// Problem identity is independent of revisions, thresholds and check timestamps.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FindingIdentity {
    pub version: u32,
    pub finding_id: Option<String>,
    pub gap: Option<String>,
}
impl Default for FindingIdentity {
    fn default() -> Self {
        Self {
            version: 1,
            finding_id: None,
            gap: Some("identityNotAssessed".into()),
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct MethodVersion {
    pub method: String,
    pub version: u32,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AssessmentScope {
    pub source_instance_id: Option<String>,
    pub item_project: Option<String>,
    pub global: bool,
    pub project: Option<String>,
    pub source_instances: Vec<String>,
    pub authorized_projects: Vec<String>,
    pub roots: Vec<String>,
    pub project_roots: Vec<String>,
    pub source_roots: Vec<String>,
    pub complete: bool,
}
/// Values consumed by the rule, including successful measurements and thresholds.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum RuleMeasurement {
    Existence {
        configured_state: String,
        measurement_status: String,
        missing: bool,
    },
    Numeric {
        basis: String,
        observed: Option<u64>,
        threshold: u64,
        inclusive: bool,
        standard_max: Option<u64>,
        suppressed_by_standard: bool,
    },
    SkillMetadata {
        status: Option<String>,
        issues: Vec<String>,
    },
    Static {
        complete: Option<bool>,
        findings: usize,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AssessmentBasis {
    pub version: u32,
    pub dependency_revision: Option<String>,
    pub scope: AssessmentScope,
    pub cutoff: String,
    pub applicability: String,
    pub measurement: RuleMeasurement,
    /// Absent revisions never act as equality wildcards.
    pub gaps: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ComparisonStatus {
    NotRequested,
    Comparable,
    Incomparable,
    Unknown,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AssessmentComparison {
    pub status: ComparisonStatus,
    pub baseline_assessment_id: Option<String>,
    pub reason: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct RuleAssessment {
    pub assessment_id: Option<String>,
    pub identity_gap: Option<String>,
    pub rule_semantics_version: u32,
    pub method_versions: Vec<MethodVersion>,
    pub basis: AssessmentBasis,
    pub comparison: AssessmentComparison,
    pub rule: String,
    pub rule_version: String,
    pub item_id: String,
    pub content_version: String,
    pub checked_at: String,
    pub outcome: RuleOutcome,
    pub reason: Option<String>,
    pub findings: Vec<Finding>,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "camelCase")]
pub struct RelationIdentity {
    pub source_instance_id: String,
    pub project: String,
    pub declaration_path: String,
    pub declaration_hash: String,
    pub relation_id: String,
    pub kind: RelationKind,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq, PartialOrd, Ord)]
#[serde(rename_all = "snake_case")]
pub enum RelationKind {
    Chain,
    Copy,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FileVersion {
    pub item_id: String,
    pub path: String,
    pub content_hash: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct BlockPosition {
    pub item_id: String,
    pub start_byte: usize,
    pub end_byte: usize,
    pub start_line: usize,
    pub end_line: usize,
    pub block_hash: String,
}

/// Existing, user-maintained .wombat/analysis.json; it declares intent, never host loading.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AnalysisDeclaration {
    pub version: u32,
    #[serde(default)]
    pub chains: Vec<DeclaredChain>,
    #[serde(default)]
    pub copies: Vec<DeclaredCopy>,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeclaredChain {
    pub id: String,
    /// Project-relative paths that the user explicitly declares jointly applicable.
    pub files: Vec<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DeclaredCopy {
    pub id: String,
    pub source: String,
    pub copy: String,
    /// First supported version: identity-v1. Other transforms stay unknown.
    pub transform: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Suggestion {
    pub baseline_id: Option<String>,
    pub review_format_version: u32,
    pub scope_project: Option<String>,
    pub id: String,
    pub item: Item,
    pub category: Category,
    pub decision: Option<UserDecision>,
    /// Current rule facts; user decisions never stand in for check outcomes.
    pub checks: Vec<RuleAssessment>,
    pub findings: Vec<Finding>,
    pub checked_at: String,
    pub rule_version: String,
    pub rule_parameters: Option<RuleParameters>,
}
/// Captured before the first persisted observation; never replaced by later checks.
/// Missing problem location permits only the exact complete suggestion/content version.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DecisionIdentityBasis {
    StableProblems,
    ExactSuggestionVersion,
    Unavailable,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct DecisionBinding {
    pub identity_basis: DecisionIdentityBasis,
    pub version: u32,
    pub suggestion_id: String,
    pub finding_ids: Vec<String>,
    pub assessment_ids: Vec<String>,
    pub content_version: String,
    pub scope: AssessmentScope,
    /// Includes relevant dependencies, methods and parameters, but no check cutoff or log revision.
    pub applicability_id: Option<String>,
    pub gap: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DecisionKind {
    Keep,
    NotApplicable,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum DecisionReason {
    Necessary,
    ObjectChanged,
    IncorrectEvidence,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct UserDecision {
    pub binding: DecisionBinding,
    pub kind: DecisionKind,
    pub reason: DecisionReason,
    pub recorded_at: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Capabilities {
    pub static_checks: bool,
    pub manual_edit_review: bool,
    pub decisions: bool,
    pub exact_instruction_blocks: bool,
    pub declared_copy_drift: bool,
    pub hook_support: HookSupport,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct HookSupport {
    pub host: Option<String>,
    pub host_version: Option<String>,
    pub effective_registry: bool,
    pub status: HookSupportStatus,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum HookSupportStatus {
    #[default]
    NoVerifiedAdapter,
    RegistryObserved,
    RegistryPartial,
}
impl Default for Capabilities {
    fn default() -> Self {
        Self {
            static_checks: true,
            manual_edit_review: true,
            decisions: true,
            exact_instruction_blocks: true,
            declared_copy_drift: true,
            hook_support: HookSupport::default(),
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Response {
    #[schemars(range(min = 6, max = 6))]
    pub output_version: u32,
    pub action: Action,
    pub capabilities: Capabilities,
    #[serde(skip)]
    #[schemars(skip)]
    pub read_view: Option<String>,
    pub config_revision: String,
    pub usage_revision: Option<String>,
    pub decision_revision: String,
    pub checked_at: String,
    pub suggestions: Vec<Suggestion>,
    pub pending: usize,
    pub page: Page,
    pub issues: Vec<Issue>,
    pub result_status: String,
    pub rule_parameters: RuleParameters,
    pub rule_catalog: Vec<RuleDefinition>,
    pub checks: Vec<RuleAssessment>,
    pub activity: Option<ActivityResult>,
}

/// Fixed turn analysis, separate from configuration identities and durable handling decisions.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ActivitySelection {
    #[schemars(length(min = 1, max = 4096))]
    #[serde(default, skip_serializing)]
    #[schemars(skip)]
    pub snapshot_id: String,
    #[schemars(length(min = 1, max = 4096))]
    pub thread_id: String,
    #[schemars(length(min = 1, max = 4096))]
    pub turn_id: String,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ActivityRule {
    InspectCallsAfterFailure,
    InspectRepeatedReads,
    InspectRepeatedRequests,
    InspectFailureShare,
    InspectInputChange,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum ActivityReason {
    ActivityMeasureUnavailable,
    ActivityCoverageIncomplete,
    ActivityBasisUnsupported,
    ActivitySampleTooSmall,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct FailureSharePolicy {
    #[schemars(range(min = 5, max = 5))]
    pub minimum_determinate: u32,
    #[schemars(range(min = 2, max = 2))]
    pub minimum_failures: u32,
    #[schemars(range(min = 0.4, max = 0.4))]
    pub minimum_ratio: f64,
}
impl Default for FailureSharePolicy {
    fn default() -> Self {
        Self {
            minimum_determinate: 5,
            minimum_failures: 2,
            minimum_ratio: 0.4,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct InputChangePolicy {
    #[schemars(range(min = 16384, max = 16384))]
    pub minimum_increase: u32,
}
impl Default for InputChangePolicy {
    fn default() -> Self {
        Self {
            minimum_increase: 16384,
        }
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ActivityCheck {
    pub input_change: Option<crate::timing_dto::InputChange>,
    pub input_policy: Option<InputChangePolicy>,
    pub outcomes: Option<crate::timing_dto::OutcomeStatistics>,
    pub failure_policy: Option<FailureSharePolicy>,
    pub rule: ActivityRule,
    #[schemars(range(min = 1, max = 1))]
    pub version: u32,
    pub method: String,
    pub outcome: RuleOutcome,
    pub observed: crate::timing_dto::Count,
    pub partial: bool,
    pub reason: Option<ActivityReason>,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ActivityResult {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operation_diagnostics: Option<crate::timing_dto::OperationDiagnostics>,
    #[schemars(range(min = 3, max = 3))]
    pub format_version: u32,
    #[serde(rename = "basis")]
    pub read_view: crate::timing_dto::ReadView,
    pub scope: crate::timing_dto::LocalScope,
    pub analysis_method: String,
    pub freshness: crate::timing_dto::QueryFreshness,
    pub source_status: String,
    pub coverage: crate::timing_dto::RepeatCoverage,
    #[schemars(length(min = 5, max = 5))]
    pub checks: Vec<ActivityCheck>,
    /// Positive inspection signals; never fault, resolution, causal waste, or savings claims.
    #[schemars(length(max = 5))]
    pub advice: Vec<ActivityRule>,
}
