//! Source-neutral facts. No pricing or user-interface policy belongs in an adapter.
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

mod tokens;
pub use tokens::{
    TokenField, TokenFields, TokenUnavailableReason, merge_token_observations,
    validate_token_observations,
};
mod operation_match;
pub use operation_match::{
    MATCH_STRING_BYTES, MATCH_TARGET_LIMIT, MatchGap, OPERATION_MATCH_VERSION,
    OperationMatchObservation, ReadMatchTarget, SourcePathPlatform,
};
mod watermarks;
mod work;
pub(crate) use watermarks::validate_watermarks;
pub use watermarks::{
    OriginalSource, SourceWatermark, WATERMARK_FORMAT_VERSION, WatermarkIssue, WatermarkState,
};
pub(crate) use work::valid_work_path;
pub use work::{
    ChangeKind, CommandSource, FilePathChange, ParsedCommand, WORK_OBSERVATION_VERSION,
    WORK_PATH_BYTES, WORK_PATH_LIMIT, WorkData, WorkGap, WorkObservation, WorkStage,
};

pub const MAX_SAFE_INTEGER: u64 = 9_007_199_254_740_991;

#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub struct TokenUsage {
    pub input: Option<u64>,
    pub cache_read: Option<u64>,
    pub cache_create: Option<u64>,
    pub output: Option<u64>,
    /// Subset of output, never an additional billable category.
    pub reasoning: Option<u64>,
    pub total: Option<u64>,
    /// Source input including caches; retained for request-level price conditions.
    pub raw_input: Option<u64>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema, PartialEq, Eq, Hash)]
#[serde(rename_all = "camelCase")]
pub struct ModelRef {
    pub raw: Option<Arc<str>>,
    pub provider: Option<Arc<str>>,
    pub api_provider: Option<Arc<str>>,
    pub pricing_model: Option<Arc<str>>,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct EvidenceRef {
    pub file: Arc<str>,
    pub line: u64,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct Capabilities {
    pub usage: bool,
    pub threads: bool,
    pub turns: bool,
    pub operations: bool,
    pub reasoning_effort: bool,
    pub response_identity: bool,
    pub measurement_grain: Vec<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AdapterDescriptor {
    pub agent_kind: String,
    pub adapter_version: String,
    pub capabilities: Capabilities,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SourceInstance {
    pub id: String,
    pub agent_kind: String,
    pub root: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Issue {
    pub code: String,
    pub message: String,
    pub source_instance_id: Option<String>,
    pub evidence: Option<EvidenceRef>,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SourceReport {
    pub source: SourceInstance,
    pub adapter_version: String,
    #[serde(default)]
    pub source_versions: Vec<String>,
    pub capabilities: Capabilities,
    /// complete, partial, failed, notFound, cancelled
    pub status: String,
    pub files_read: u64,
    pub bytes_read: u64,
    pub issues: Vec<Issue>,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(rename_all = "camelCase")]
#[schemars(rename = "Thread")]
pub struct Thread<T = String> {
    pub id: T,
    pub agent_kind: T,
    pub source_instance_id: T,
    pub upstream_id: T,
    pub title: Option<T>,
    pub project: Option<T>,
    pub started_at: Option<T>,
    pub last_activity_at: Option<T>,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(rename_all = "camelCase")]
#[schemars(rename = "Turn")]
pub struct Turn<T = String> {
    pub id: T,
    pub thread_id: T,
    pub upstream_id: T,
    pub ordinal: u64,
    pub started_at: Option<T>,
    pub ended_at: Option<T>,
    /// Last observed event; never inferred from turn ordinal.
    pub last_activity_at: Option<T>,
    /// running, completed, interrupted, failed, unknown
    pub status: T,
}

// Source events use shared immutable text; public canonical facts keep owned text.
impl Thread {
    pub(crate) fn into_shared(self) -> Thread<Arc<str>> {
        Thread {
            id: self.id.into(),
            agent_kind: self.agent_kind.into(),
            source_instance_id: self.source_instance_id.into(),
            upstream_id: self.upstream_id.into(),
            title: self.title.map(Into::into),
            project: self.project.map(Into::into),
            started_at: self.started_at.map(Into::into),
            last_activity_at: self.last_activity_at.map(Into::into),
        }
    }
}
impl Turn {
    pub(crate) fn into_shared(self) -> Turn<Arc<str>> {
        Turn {
            id: self.id.into(),
            thread_id: self.thread_id.into(),
            upstream_id: self.upstream_id.into(),
            ordinal: self.ordinal,
            started_at: self.started_at.map(Into::into),
            ended_at: self.ended_at.map(Into::into),
            last_activity_at: self.last_activity_at.map(Into::into),
            status: self.status.into(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Measurement {
    pub id: String,
    pub agent_kind: Arc<str>,
    pub source_instance_id: Arc<str>,
    pub thread_id: Option<Arc<str>>,
    pub turn_id: Option<Arc<str>>,
    pub response_id: Option<String>,
    pub timestamp: Option<String>,
    pub interval_end: Option<String>,
    pub grain: Arc<str>,
    pub time_precision: Arc<str>,
    pub model: ModelRef,
    pub reasoning_effort: Option<Arc<str>>,
    pub tokens: TokenUsage,
    /// One explicit reason for each unavailable token field; known values have none.
    pub token_unavailable_reasons: TokenFields<Option<TokenUnavailableReason>>,
    /// Explicit model/provider contradictions cannot be treated as absent price context.
    pub pricing_context_conflict: bool,
    pub request_scoped: bool,
    pub reported_cost: Option<String>,
    pub service_tier: Option<String>,
    pub sequence: u64,
    pub evidence: Vec<EvidenceRef>,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct Operation {
    /// Hash of an exact native text result, never inferred from current filesystem content.
    #[serde(default)]
    pub text_result: Option<TextResultObservation>,
    pub id: String,
    pub thread_id: Arc<str>,
    pub turn_id: Option<Arc<str>>,
    pub item_id: Option<String>,
    pub call_id: Option<String>,
    pub response_id: Option<String>,
    pub kind: Arc<str>,
    pub name: Arc<str>,
    pub sequence: u64,
    pub timestamp: Option<String>,
    pub time_precision: Arc<str>,
    pub status: Arc<str>,
    pub exit_code: Option<i64>,
    /// Sticky disagreement among reliable native results; no single exit code is asserted.
    pub outcome_conflict: bool,
    pub duration_ms: Option<u64>,
    pub path: Option<String>,
    /// Safe source metadata; absent observations cannot establish zero work.
    pub work: Option<WorkObservation>,
    /// Shared safe request/read observations; raw arguments never persist.
    #[serde(default)]
    pub matching: Option<OperationMatchObservation>,
    pub server: Option<Arc<str>>,
    pub tool: Option<Arc<str>>,
    pub evidence: Vec<EvidenceRef>,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TextResultObservation {
    pub method_version: u32,
    pub hash: Option<String>,
    pub observed_at: Option<String>,
    pub conflicting: bool,
}

// Internal collection, not a public query DTO or generated transport schema.
#[derive(Clone, Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Collected {
    pub watermarks: Vec<SourceWatermark>,
    pub sources: Vec<SourceReport>,
    pub threads: Vec<Thread>,
    pub turns: Vec<Turn>,
    pub measurements: Vec<Arc<Measurement>>,
    pub operations: Vec<Arc<Operation>>,
    pub events: Vec<Arc<crate::session_events::Event>>,
    pub title_observations: Vec<crate::session_events::title_observations::TitleObservation>,
    pub issues: Vec<Issue>,
}

#[derive(Clone, Debug, Default)]
pub struct DiscoveryRequest {
    pub roots: Vec<PathBuf>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DiscoveryReport {
    pub sources: Vec<SourceInstance>,
    pub issues: Vec<Issue>,
}
#[derive(Clone, Debug, Default)]
pub struct ReadPlan;

#[derive(Clone, Debug)]
pub struct RunContext {
    pub cancelled: Arc<AtomicBool>,
    pub max_files: usize,
    pub max_bytes: u64,
}
impl Default for RunContext {
    fn default() -> Self {
        Self {
            cancelled: Arc::new(AtomicBool::new(false)),
            max_files: 1_000_000,
            max_bytes: 1_000_000_000_000,
        }
    }
}
impl RunContext {
    pub fn is_cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
    }
}

pub enum Fact {
    Watermark(SourceWatermark),
    Thread(Thread),
    Turn(Turn),
    Measurement(Arc<Measurement>),
    Operation(Arc<Operation>),
    Event(Arc<crate::session_events::Event>),
    TitleObservation(crate::session_events::title_observations::TitleObservation),
}
pub trait FactSink {
    fn push(&mut self, fact: Fact);
}
impl FactSink for Collected {
    fn push(&mut self, fact: Fact) {
        match fact {
            Fact::Watermark(value) => self.watermarks.push(value),
            Fact::Thread(value) => self.threads.push(value),
            Fact::Turn(value) => self.turns.push(value),
            Fact::Measurement(value) => self.measurements.push(value),
            Fact::Operation(value) => self.operations.push(value),
            Fact::Event(value) => self.events.push(value),
            Fact::TitleObservation(value) => self.title_observations.push(value),
        }
    }
}
pub trait AgentAdapter: Send + Sync {
    fn descriptor(&self) -> AdapterDescriptor;
    fn discover(&self, request: &DiscoveryRequest) -> DiscoveryReport;
    fn collect(
        &self,
        source: &SourceInstance,
        plan: &ReadPlan,
        context: &RunContext,
        sink: &mut dyn FactSink,
    ) -> SourceReport;
}

/// Source paths repeat across every record; keep one allocation per observed path.
#[derive(Default)]
pub(crate) struct EvidencePaths(std::collections::BTreeSet<Arc<str>>);
impl EvidencePaths {
    pub(crate) fn compact(&mut self, evidence: &mut [EvidenceRef]) {
        for item in evidence {
            if let Some(path) = self.0.get(item.file.as_ref()) {
                item.file = Arc::clone(path);
            } else {
                self.0.insert(Arc::clone(&item.file));
            }
        }
    }
}
