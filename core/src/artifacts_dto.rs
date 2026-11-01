//! Artifact observations and explicitly selected Codex cleanup. Candidates are not authorization.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(
    tag = "action",
    rename_all = "snake_case",
    rename_all_fields = "camelCase",
    deny_unknown_fields
)]
pub enum Request {
    Inspect {
        project_root: String,
        #[serde(default)]
        paths: Vec<String>,
        #[serde(default)]
        keep_paths: Vec<String>,
        #[serde(default)]
        product_temporary: bool,
        #[serde(default)]
        codex: bool,
    },
    Apply {
        read_view: String,
        baseline_id: String,
        #[schemars(length(max = 100))]
        selection: Vec<String>,
    },
    Recheck {
        read_view: String,
        baseline_id: String,
        #[serde(default)]
        selection: Vec<String>,
    },
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Assessment {
    Candidate,
    Review,
    Protected,
    Unknown,
    Absent,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Kind {
    BuildOutput,
    SelectedTemporary,
    ProductTemporary,
    CodexDesktopCache,
    CodexUpdateStage,
    CodexMarketplaceStage,
    CodexDaemonRelease,
    CodexRuntime,
    CodexCrashReport,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum UseState {
    NoObservedOpenFiles,
    InUse,
    Unknown,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Regeneration {
    CargoCache,
    TypeScriptOutput,
    CodexCache,
    SupersededPackage,
    DiscardedStage,
    Diagnostic,
    Unknown,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Reason {
    DeclaredBuildOutput,
    CacheTag,
    TaskOwnershipRequired,
    ProductStage,
    UserKept,
    UserAsset,
    TrackedContent,
    UntrackedContent,
    NestedRepository,
    Symlink,
    SpecialFile,
    SharedOutput,
    NoRepository,
    GitUnavailable,
    RegenerationUnknown,
    InUse,
    UseUnknown,
    ReadUnavailable,
    ScanLimit,
    ChangedDuringScan,
    NotFound,
    OwnerChanged,
    ReferencedProductData,
    KnownCodexCache,
    SupersededVersion,
    ActiveVersion,
    PendingUpdate,
    Recent,
    IncompleteRuntime,
    ActiveRuntime,
    ProcessUnknown,
    InstallationBusy,
    InstallationUnknown,
    UnsupportedPlatform,
    ConditionsChanged,
    RemovalFailed,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RecheckState {
    Unchanged,
    Changed,
    Gone,
    Unavailable,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Observation {
    pub physical_identity: String,
    pub fingerprint: String,
    pub logical_bytes: u64,
    pub allocated_bytes: Option<u64>,
    pub files: usize,
    pub complete: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CodexEvidence {
    pub parent_identity: String,
    pub install_lock_identity: Option<String>,
    pub minimum_age_days: Option<u32>,
    pub current_release: Option<String>,
    pub pending_release: Option<String>,
    pub installed_build: Option<u64>,
    pub staged_build: Option<u64>,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum Removal {
    Removed,
    AlreadyAbsent,
    Skipped,
    Failed,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Item {
    pub id: String,
    pub path: String,
    pub owner: Option<String>,
    pub kind: Kind,
    pub assessment: Assessment,
    pub regeneration: Regeneration,
    pub regeneration_version: Option<String>,
    pub use_state: UseState,
    pub observation: Option<Observation>,
    pub reasons: Vec<Reason>,
    pub recheck: Option<RecheckState>,
    pub codex_evidence: Option<CodexEvidence>,
    pub removal: Option<Removal>,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Scope {
    pub project_root: String,
    pub paths: Vec<String>,
    pub keep_paths: Vec<String>,
    pub product_temporary: bool,
    pub codex: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Summary {
    pub candidates: usize,
    pub review: usize,
    pub protected: usize,
    pub unknown: usize,
    pub absent: usize,
    /// Deduplicated observed logical file bytes, never a reclaim estimate.
    pub observed_logical_bytes: u64,
    pub complete: bool,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Response {
    #[schemars(range(min = 2, max = 2))]
    pub output_version: u32,
    #[schemars(range(min = 2, max = 2))]
    pub policy_version: u32,
    pub action: String,
    pub read_view: String,
    pub baseline_id: String,
    pub checked_at: String,
    pub expires_at: String,
    pub scope: Scope,
    pub items: Vec<Item>,
    pub summary: Summary,
    pub partial: bool,
}
