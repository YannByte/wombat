//! Public usage query contract. All calculations remain in the Rust core.
pub const REQUEST_VERSION: u32 = 4;

use crate::adapters::contract::{Issue, SourceReport, TokenField, TokenFields, TokenUsage};
use crate::pricing::PriceResult;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
mod comparison;
pub use comparison::*;
mod inspection;
pub use inspection::*;
mod opportunities;
pub use opportunities::*;
mod statistics;
pub use statistics::*;

#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Scope {
    pub all_time: Option<bool>,
    pub timezone: Option<String>,
    pub since: Option<String>,
    pub until: Option<String>,
    pub agent_kind: Option<String>,
    pub source_instance_id: Option<String>,
    pub model: Option<String>,
    pub model_unknown: Option<bool>,
    pub effort_unknown: Option<bool>,
    pub undated: Option<bool>,
    pub reasoning_effort: Option<String>,
    pub project: Option<String>,
    pub project_unknown: Option<bool>,
    pub thread_id: Option<String>,
    /// Exact turn inspection, always bound to a selected thread.
    #[serde(default)]
    pub turn_id: Option<String>,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Action {
    #[schemars(skip)]
    Refresh,
    #[default]
    Usage,
    Threads,
    Turns,
    Steps,
    Compare,
    Investigate,
    Trajectory,
    Resources,
    Review,
    Context,
    Statistics,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Group {
    #[default]
    Day,
    Week,
    Month,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Sort {
    Tokens,
    Cost,
    Recent,
    Time,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Presentation {
    Distribution,
    Details,
    Projects,
    Models,
}
#[derive(Clone, Debug, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Request {
    pub comparison: Option<ComparisonRequest>,
    pub action: Action,
    #[serde(default, skip_serializing)]
    #[schemars(skip)]
    pub snapshot_id: Option<String>,
    pub roots: Option<Vec<String>>,
    #[serde(default)]
    pub scope: Scope,
    pub group: Option<Group>,
    pub sort: Option<Sort>,
    pub presentation: Option<Presentation>,
    pub thread_id: Option<String>,
    pub turn_id: Option<String>,
    pub search: Option<String>,
    pub offset: Option<usize>,
    pub limit: Option<usize>,
    pub locate_thread_id: Option<String>,
    pub locate_turn_id: Option<String>,
    pub locate_operation_id: Option<String>,
    pub matched_only: Option<bool>,
    /// Keep totals and the requested page, with bounded quality examples and no facets.
    #[serde(default)]
    pub compact: Option<bool>,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct UsageSummary {
    /// All input, including cache reads and writes. Existing tokens.input stays uncached.
    #[serde(default)]
    pub input_total: Option<u64>,
    #[serde(default)]
    pub cache_hit_rate: Option<f64>,
    #[serde(default)]
    pub unpriced_tokens: Option<u64>,
    pub tokens: TokenUsage,
    pub token_analysis: TokenAnalysis,
    pub price: PriceResult,
    pub measurement_count: usize,
}

/// Token subtotals are scoped to the canonical measurements selected by this query.
/// Existing `tokens` fields remain complete totals; partial observations live here.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct TokenAnalysis {
    #[schemars(range(min = 1, max = 1))]
    pub method_version: u32,
    pub scope: TokenAnalysisScope,
    pub fields: TokenFields<ObservedTokenSubtotal>,
    /// Independent analysis; immutable historical review items may contain only
    /// the native observations captured when the user made the decision.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub total_analysis: Option<AnalyzedTokenTotal>,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct AnalyzedTokenTotal {
    #[schemars(range(min = 1, max = 1))]
    pub method_version: u32,
    pub subtotal: Option<u64>,
    pub covered_records: u64,
    pub recorded_records: u64,
    /// Unavailable native totals use recorded input (including caches) plus output
    /// only for an identified response grain. Reasoning is already in output.
    pub calculated_records: u64,
    pub unavailable_records: u64,
    /// Alternative records excluded by individual or combined safe-integer limits.
    pub overflow_records: u64,
}

impl UsageSummary {
    pub fn available_token_subtotal(&self) -> Option<u64> {
        match &self.token_analysis.total_analysis {
            Some(total) => total.subtotal,
            None => self.token_analysis.fields.total.observed_subtotal,
        }
    }
    /// Fractions require all selected canonical measurements to have a usable
    /// value under this analysis method; partial subtotals are not denominators.
    pub fn complete_token_total(&self) -> Option<u64> {
        match &self.token_analysis.total_analysis {
            Some(total) if total.unavailable_records == 0 => total
                .subtotal
                .or((self.measurement_count == 0).then_some(0)),
            Some(_) => None,
            None => self.tokens.total,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TokenAnalysisScope {
    SelectedCanonicalMeasurements,
}

/// An observed subtotal never implies that unavailable records contributed zero.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ObservedTokenSubtotal {
    pub observed_subtotal: Option<u64>,
    pub covered_records: u64,
    pub missing_records: u64,
    pub conflicting_records: u64,
    pub invalid_records: u64,
    pub indeterminate_records: u64,
}

impl TokenAnalysis {
    pub const METHOD_VERSION: u32 = 1;

    pub fn field(&self, field: TokenField) -> &ObservedTokenSubtotal {
        self.fields.get(field)
    }
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotRef {
    pub snapshot_id: String,
    pub created_at: String,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Page {
    pub offset: usize,
    pub limit: usize,
    pub total: usize,
    pub next_offset: Option<usize>,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Quality {
    /// Issues and source statuses describe the whole snapshot, not this query window.
    pub issue_scope: QualityIssueScope,
    /// Snapshot warnings without query-bound attribution cannot prove a scoped defect.
    pub query_impact: QualityQueryImpact,
    pub status: String,
    pub issues: Vec<Issue>,
    pub sources: Vec<SourceReport>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub detail_summary: Option<QualityDetailSummary>,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QualityIssueScope {
    Index,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum QualityQueryImpact {
    None,
    Unknown,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct QualityDetailSummary {
    pub issue_count: usize,
    pub source_count: usize,
    pub issue_counts: std::collections::BTreeMap<String, usize>,
    pub source_status_counts: std::collections::BTreeMap<String, usize>,
    pub omitted_issues: usize,
    pub omitted_sources: usize,
    pub omitted_source_issues: usize,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct AvailableRange {
    pub since: Option<String>,
    /// Exclusive local date boundary.
    pub until: Option<String>,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(
    tag = "kind",
    rename_all = "snake_case",
    rename_all_fields = "camelCase"
)]
pub enum Item {
    Usage {
        date: Option<String>,
        /// Inclusive display date. Scope.until remains exclusive for queries.
        end_date: Option<String>,
        is_subtotal: bool,
        model: Option<String>,
        reasoning_effort: Option<String>,
        usage: UsageSummary,
        scope: Scope,
        share: Option<f64>,
        cost_share: Option<f64>,
    },
    Thread {
        id: String,
        #[serde(default)]
        upstream_id: Option<String>,
        #[serde(default)]
        matched_last_activity_at: Option<String>,
        agent_kind: String,
        source_instance_id: String,
        title: Option<String>,
        project: Option<String>,
        started_at: Option<String>,
        last_activity_at: Option<String>,
        models: Vec<String>,
        reasoning_efforts: Vec<String>,
        /// Distinct turns with measurements in the current thread-list scope.
        /// None means the matched source records provide no turn association.
        matched_turn_count: Option<usize>,
        matched_usage: UsageSummary,
        thread_usage: UsageSummary,
    },
    Turn {
        id: String,
        thread_id: String,
        ordinal: Option<u64>,
        started_at: Option<String>,
        ended_at: Option<String>,
        /// Latest reliable source event time, independent of ordinal and metering.
        last_activity_at: Option<String>,
        status: String,
        models: Vec<String>,
        reasoning_efforts: Vec<String>,
        usage: UsageSummary,
        matched_usage: UsageSummary,
        share: Option<f64>,
        cost_share: Option<f64>,
    },
    Measurement {
        #[serde(default)]
        matches_scope: bool,
        id: String,
        thread_id: Option<String>,
        turn_id: Option<String>,
        timestamp: Option<String>,
        model: Option<String>,
        reasoning_effort: Option<String>,
        usage: UsageSummary,
        share: Option<f64>,
        cost_share: Option<f64>,
        sequence: u64,
        time_precision: String,
    },
    Operation {
        id: String,
        thread_id: String,
        turn_id: Option<String>,
        timestamp: Option<String>,
        name: String,
        status: String,
        sequence: u64,
        time_precision: String,
        operation_type: String,
        exit_code: Option<i64>,
        duration_ms: Option<u64>,
        path: Option<String>,
        server: Option<String>,
        tool: Option<String>,
    },
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Distribution {
    pub unpriced_tokens: Option<u64>,
    /// Basis for maxTokens and peak token scopes/dates.
    pub token_basis: TokenBasis,
    /// Maximum of bucket analyzed subtotals; not necessarily a complete total.
    pub max_tokens: Option<u64>,
    pub max_cost: Option<String>,
    pub peak_token_dates: Vec<Option<String>>,
    pub peak_cost_dates: Vec<Option<String>>,
    pub peak_token_scopes: Vec<Scope>,
    pub peak_cost_scopes: Vec<Scope>,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum TokenBasis {
    AnalyzedTotals,
}
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Response {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub statistics: Option<TaskStatistics>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub inspection: Option<Inspection>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub comparison: Option<Comparison>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub facets: Option<Facets>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub distribution: Option<Distribution>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub price_update: Option<crate::pricing_sync::Automatic>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub freshness: Option<crate::live::Freshness>,
    #[schemars(range(min = 5, max = 5))]
    pub output_version: u32,
    pub action: Action,
    pub snapshot_ref: SnapshotRef,
    pub scope: Scope,
    pub available_range: AvailableRange,
    pub summary: UsageSummary,
    pub items: Vec<Item>,
    pub page: Page,
    pub quality: Quality,
}

/// Observed dimensions, not a project registry or a configuration inventory.
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Facets {
    /// Metadata across this snapshot's authorized sources, independent of measurement/date filters.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub discovered_thread_count: Option<usize>,
    pub directories: Vec<String>,
    pub has_unassigned: bool,
    pub models: Vec<String>,
    pub reasoning_efforts: Vec<String>,
    pub agents: Vec<String>,
}
