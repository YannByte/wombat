/* Generated from Rust. Run pnpm contracts:generate. */

export type State = "complete" | "partial" | "unavailable";
export type Kind = "rule" | "skill" | "mcp" | "hook";
export type UseReason =
  | "counted"
  | "identity_missing"
  | "time_missing"
  | "dispatch_unknown"
  | "target_missing"
  | "target_not_found"
  | "target_ambiguous"
  | "identity_conflict"
  | "duplicate"
  | "outside_window"
  | "outside_project"
  | "catalog_only"
  | "instruction_load"
  | "not_use";
export type UseComparisonStatus = "unchanged" | "changed" | "input_changed" | "scope_changed" | "configuration_changed";
export type UseBasisStatus = ("observed" | "unavailable") | "partial";
export type UseUnit = "object_use" | "rule_read" | "rule_load_or_read";
export type UseWindow =
  | {
      kind: "all_history";
    }
  | {
      since: string;
      until: string;
      timezone: string;
      kind: "date_window";
    };
export type UseTimeBasis = "source_operation_time";
export type UseSourceCompleteness = "complete" | "partial" | "unknown";
export type Action = "list" | "detail" | "evidence" | "explain" | "related_scopes" | "capabilities";
export type TokenAnalysisScope = "selected_canonical_measurements";
export type Observation = "used" | "loaded_only" | "unknown";
export type HookRegistryStatus = "unavailable" | "partial" | "observed";
export type HookTrust = "managed" | "untrusted" | "trusted" | "modified";
export type HookHandler = "command" | "mcpTool" | "prompt" | "agent";
export type Action2 =
  "list" | "detail" | "keep" | "not_applicable" | "recheck" | "capabilities" | "checks" | "activity";
export type HookSupportStatus = "no_verified_adapter" | "registry_observed" | "registry_partial";
export type Category = "repair" | "trim";
/**
 * Captured before the first persisted observation; never replaced by later checks.
 * Missing problem location permits only the exact complete suggestion/content version.
 */
export type DecisionIdentityBasis = "stable_problems" | "exact_suggestion_version" | "unavailable";
export type DecisionKind = "keep" | "not_applicable";
export type DecisionReason = "necessary" | "object_changed" | "incorrect_evidence";
/**
 * Values consumed by the rule, including successful measurements and thresholds.
 */
export type RuleMeasurement =
  | {
      configuredState: string;
      measurementStatus: string;
      missing: boolean;
      kind: "existence";
    }
  | {
      basis: string;
      observed?: number | null;
      threshold: number;
      inclusive: boolean;
      standardMax?: number | null;
      suppressedByStandard: boolean;
      kind: "numeric";
    }
  | {
      status?: string | null;
      issues: string[];
      kind: "skill_metadata";
    }
  | {
      complete?: boolean | null;
      findings: number;
      kind: "static";
    };
export type ComparisonStatus = "not_requested" | "comparable" | "incomparable" | "unknown";
export type RuleOutcome = "hit" | "miss" | "insufficient" | "unsupported" | "error";
export type RelationKind = "chain" | "copy";
export type MetricStatus = "observed" | "derived" | "proxy" | "unavailable";
export type Basis =
  | "native_record"
  | "explicit_boundary"
  | "lifecycle_union"
  | "lifecycle_sum"
  | "interval_mask"
  | "operation_union"
  | "operation_residual"
  | "repeat_after_failure"
  | "successful_read_repeat"
  | "known_operation_duration"
  | "failure_recovery_span"
  | "same_request_observation"
  | "request_input"
  | "historical_window"
  | "type7"
  | "safe_message_record"
  | "safe_message_delay"
  | "safe_event_count"
  | "response_gap_v1"
  | "not_recorded"
  | "adapter_not_mapped"
  | "unsupported_method"
  | "missing_identity"
  | "missing_time"
  | "running_turn"
  | "exact_event_page"
  | "boundary_conflict"
  | "source_partial"
  | "resource_limit"
  | "numeric_range"
  | "no_candidates"
  | "missing_batch_cycle"
  | "missing_repository_baseline"
  | "unknown_message_origin"
  | "canonical_operation_identity"
  | "determinate_terminal_outcomes"
  | "reported_file_paths"
  | "canonical_use_identity"
  | "canonical_use_records"
  | "unassigned_use_index"
  | "dispatch_not_proven"
  | "missing_target"
  | "target_conflict"
  | "outcome_conflict"
  | "missing_turn";
export type RepeatCoverageReason =
  | "missing_matching"
  | "excluded_receivers"
  | "identity_gaps"
  | "conflicting_operations"
  | "missing_start"
  | "indeterminate_outcomes"
  | "order_gaps"
  | "context_boundaries"
  | "crossed_context"
  | "missing_clock_domain"
  | "source_metadata_gaps"
  | "duration_conflicts"
  | "missing_durations"
  | "missing_recovery_spans"
  | "missing_intervals"
  | "missing_window"
  | "source_partial"
  | "resource_limit"
  | "numeric_range";
export type InputChangeMethod = "request_input_observation_change_v1";
export type Support = "supported" | "partial" | "unavailable";
export type OutcomeMethod = "terminal_success_failure_subset_v1";
export type ActivityRule =
  | "inspect_calls_after_failure"
  | "inspect_repeated_reads"
  | "inspect_repeated_requests"
  | "inspect_failure_share"
  | "inspect_input_change";
export type ActivityReason =
  "activityMeasureUnavailable" | "activityCoverageIncomplete" | "activityBasisUnsupported" | "activitySampleTooSmall";
export type Presentation = "distribution" | "details" | "projects" | "models";
export type InspectionKind = "investigate" | "trajectory" | "resources" | "review" | "context";
export type InspectionLimit =
  | "source_partial"
  | "context_occupancy_unavailable"
  | "actual_changes_unavailable"
  | "unlocated_operations"
  | "unknown_input_order"
  | "operation_outcomes_partial"
  | "operation_model_association"
  | "context_metadata_unavailable"
  | "selected_turn_only";
export type InspectionSignal = "high_usage" | "low_cache_reuse" | "input_jump" | "failure_share" | "repeated_request";
export type InspectionEvidenceView = "task" | "turn" | "operation";
export type InputBoundary =
  | "first"
  | "compaction"
  | "model_change"
  | "source_change"
  | "ambiguous_order"
  | "missing_input"
  | "scope_gap"
  | "source_gap"
  | "turn_change"
  | "missing_context";
export type Comparison =
  | {
      dimension: DriverDimension;
      baseline: ComparedUsage;
      current: ComparedUsage;
      delta: UsageDelta;
      drivers: UsageDriver[];
      /**
       * Contribution of all drivers outside this page, so pages remain reconcilable.
       */
      remaining: UsageDelta;
      undatedRecords: number;
      kind: "periods";
    }
  | {
      left: ComparedSession;
      right: ComparedSession;
      delta: UsageDelta;
      includeDescendants: boolean;
      kind: "sessions";
    };
export type DriverDimension = "project" | "model" | "thread";
export type ContextRecordKind = "injected_context" | "model_window";
export type ActivitySignal =
  | "repeated_slow_request"
  | "failure_spike"
  | "duration_spike"
  | "recurring_workflow"
  | "repeated_failure"
  | "repeated_rejection";
export type OpportunityRule =
  | "unpriced_usage"
  | "estimate_concentration"
  | "estimate_outlier"
  | "estimate_increase"
  | "cache_creation_reuse"
  | "model_review"
  | "sensitive_read"
  | "sensitive_change"
  | "outside_project_change"
  | "risky_command"
  | "secret_exposure"
  | "sensitive_outbound"
  | "repeated_risky_decline"
  | "permission_friction"
  | "unanswered_question"
  | "long_interaction"
  | "frequent_polling";
export type OpportunityStatus = "hit" | "miss" | "insufficient";
export type OpportunityGap =
  | "source_partial"
  | "no_observations"
  | "incomplete_prices"
  | "no_baseline"
  | "unknown_cache"
  | "path_identity"
  | "runtime_coverage"
  | "interaction_association"
  | "outcome_unknown";
export type OpportunityMetricName =
  | "unpriced_tokens"
  | "amount"
  | "baseline_amount"
  | "share"
  | "multiple"
  | "operations"
  | "tasks"
  | "median_operations"
  | "cache_created"
  | "cache_read"
  | "read_create_ratio"
  | "requests"
  | "unanswered"
  | "interval_ms"
  | "samples"
  | "unknown";
export type OpportunityUnit = "count" | "token" | "usd" | "ratio" | "factor" | "milliseconds";
export type SafetyLabel =
  "remote_script_execution" | "broad_deletion" | "broad_permissions" | "decode_execution" | "possible_credential";
export type TokenBasis = "analyzed_totals";
export type AutomaticStatus = "checking" | "updated" | "unchanged" | "failed";
export type ProjectLoadState = "pending" | "loading" | "ready";
export type Action3 =
  | "usage"
  | "threads"
  | "turns"
  | "steps"
  | "compare"
  | "investigate"
  | "trajectory"
  | "resources"
  | "review"
  | "context"
  | "statistics";
export type Item2 =
  | {
      date?: string | null;
      /**
       * Inclusive display date. Scope.until remains exclusive for queries.
       */
      endDate?: string | null;
      isSubtotal: boolean;
      model?: string | null;
      reasoningEffort?: string | null;
      usage: UsageSummary;
      scope: Scope2;
      share?: number | null;
      costShare?: number | null;
      kind: "usage";
    }
  | {
      id: string;
      upstreamId?: string | null;
      matchedLastActivityAt?: string | null;
      agentKind: string;
      sourceInstanceId: string;
      title?: string | null;
      project?: string | null;
      startedAt?: string | null;
      lastActivityAt?: string | null;
      models: string[];
      reasoningEfforts: string[];
      /**
       * Distinct turns with measurements in the current thread-list scope.
       * None means the matched source records provide no turn association.
       */
      matchedTurnCount?: number | null;
      matchedUsage: UsageSummary;
      threadUsage: UsageSummary;
      kind: "thread";
    }
  | {
      id: string;
      threadId: string;
      ordinal?: number | null;
      startedAt?: string | null;
      endedAt?: string | null;
      /**
       * Latest reliable source event time, independent of ordinal and metering.
       */
      lastActivityAt?: string | null;
      status: string;
      models: string[];
      reasoningEfforts: string[];
      usage: UsageSummary;
      matchedUsage: UsageSummary;
      share?: number | null;
      costShare?: number | null;
      kind: "turn";
    }
  | {
      matchesScope?: boolean;
      id: string;
      threadId?: string | null;
      turnId?: string | null;
      timestamp?: string | null;
      model?: string | null;
      reasoningEffort?: string | null;
      usage: UsageSummary;
      share?: number | null;
      costShare?: number | null;
      sequence: number;
      timePrecision: string;
      kind: "measurement";
    }
  | {
      id: string;
      threadId: string;
      turnId?: string | null;
      timestamp?: string | null;
      name: string;
      status: string;
      sequence: number;
      timePrecision: string;
      operationType: string;
      exitCode?: number | null;
      durationMs?: number | null;
      path?: string | null;
      server?: string | null;
      tool?: string | null;
      kind: "operation";
    };
export type QualityIssueScope = "index";
export type QualityQueryImpact = "none" | "unknown";
export type Action4 = ("activity" | "locate" | "read" | "search") | "records" | "resolve";
/**
 * Source-backed provenance only; an unclassified user record is not authorization.
 */
export type MessageOrigin =
  | "assistant_visible"
  | "user_input"
  | "user_unclassified"
  | "injected_context"
  | "reasoning"
  | "inter_agent"
  | "inherited"
  | "compaction"
  | "unknown";
export type ContentPhase = "commentary" | "final_answer" | "unknown";
export type MessageRecordKind = "legacy_snapshot" | "response_snapshot" | "native_snapshot" | "delta" | "unknown";
export type Change = "new_source_file" | "appended" | "generation_changed";
export type Kind2 =
  | "build_output"
  | "selected_temporary"
  | "product_temporary"
  | "codex_desktop_cache"
  | "codex_update_stage"
  | "codex_marketplace_stage"
  | "codex_daemon_release"
  | "codex_runtime"
  | "codex_crash_report";
export type Assessment = "candidate" | "review" | "protected" | "unknown" | "absent";
export type Regeneration =
  | "cargo_cache"
  | "type_script_output"
  | "codex_cache"
  | "superseded_package"
  | "discarded_stage"
  | "diagnostic"
  | "unknown";
export type UseState = "no_observed_open_files" | "in_use" | "unknown";
export type Reason =
  | "declared_build_output"
  | "cache_tag"
  | "task_ownership_required"
  | "product_stage"
  | "user_kept"
  | "user_asset"
  | "tracked_content"
  | "untracked_content"
  | "nested_repository"
  | "symlink"
  | "special_file"
  | "shared_output"
  | "no_repository"
  | "git_unavailable"
  | "regeneration_unknown"
  | "in_use"
  | "use_unknown"
  | "read_unavailable"
  | "scan_limit"
  | "changed_during_scan"
  | "not_found"
  | "owner_changed"
  | "referenced_product_data"
  | "known_codex_cache"
  | "superseded_version"
  | "active_version"
  | "pending_update"
  | "recent"
  | "incomplete_runtime"
  | "active_runtime"
  | "process_unknown"
  | "installation_busy"
  | "installation_unknown"
  | "unsupported_platform"
  | "conditions_changed"
  | "removal_failed";
export type RecheckState = "unchanged" | "changed" | "gone" | "unavailable";
export type Removal = "removed" | "already_absent" | "skipped" | "failed";

export interface Response {
  outputVersion: number;
  scope: Request;
  partial: boolean;
  configuration: Section;
  checks: Section2;
  history: Section3;
  records: Section4;
  artifacts: Section5;
}
export interface Request {
  projectRoot: string;
  roots?: string[];
  since: string;
  until: string;
  timezone: string;
  sourceInstanceId?: string | null;
  ruleOverrides?: RuleOverrides | null;
}
export interface RuleOverrides {
  agentsBytes?: number | null;
  descriptionCharacters?: number | null;
}
export interface Section {
  state: State;
  result?: Response1 | null;
  error?: Failure | null;
}
export interface Response1 {
  explanation?: UseExplanation | null;
  extensionReview?: ExtensionReview | null;
  extensionActivity?: ExtensionActivityStatistics | null;
  outputVersion: number;
  action: Action;
  capabilities: Capabilities;
  usageRevision?: string | null;
  configRevision: string;
  checkedAt: string;
  scope: Scope;
  authorizedProjects: string[];
  /**
   * Startup read authorization, not proof that logs exist or are readable.
   */
  authorizedSourceRoots?: string[];
  hostRestartCommand?: string | null;
  summary: Summary;
  items: Item[];
  evidence: Evidence[];
  relatedScopes: RelatedScope[];
  page: Page;
  coverage: Coverage;
  hookRegistry: HookRegistry;
}
export interface UseExplanation {
  baseline: UseReplayBaseline;
  /**
   * Counts and digests cover the whole selection; only this page is retained.
   */
  records: UseTraceRecord[];
  page: Page;
  comparison?: UseComparison | null;
  /**
   * Source-level diagnostics can extend beyond the selected dates. No raw messages.
   */
  sourceIssues: UseSourceIssue[];
  sourceIssuesTotal: number;
}
export interface UseReplayBaseline {
  formatVersion: number;
  methodVersion: number;
  scope: Scope;
  roots: string[];
  projectRoots: string[];
  kind?: Kind | null;
  itemId?: string | null;
  unassigned: boolean;
  /**
   * Normalized input facts, not raw source bodies or a guarantee of native coverage.
   */
  inputRevision: string;
  configurationRevision: string;
  resultRevision: string;
  counted: number;
  totalRecords: number;
  reasons: UseReasonCount[];
}
export interface Scope {
  allTime?: boolean | null;
  since?: string | null;
  until?: string | null;
  timezone?: string | null;
  project?: string | null;
  agentKind?: string | null;
  sourceInstanceId?: string | null;
  threadId?: string | null;
}
export interface UseReasonCount {
  reason: UseReason;
  records: number;
}
export interface UseTraceRecord {
  operationId: string;
  sourceInstanceId: string;
  threadId: string;
  turnId?: string | null;
  itemId?: string | null;
  timestamp?: string | null;
  eventType: string;
  outcome: string;
  reason: UseReason;
  /**
   * None means the evidence cannot establish a numeric contribution.
   */
  countDelta?: number | null;
  evidence?: EvidenceRef | null;
  sourceObservationCount: number;
  /**
   * Up to three further source observations; phases are not independent uses.
   */
  additionalEvidence: EvidenceRef[];
}
export interface EvidenceRef {
  file: string;
  line: number;
}
export interface Page {
  offset: number;
  limit: number;
  total: number;
  nextOffset?: number | null;
}
export interface UseComparison {
  status: UseComparisonStatus;
  before: number;
  after: number;
  /**
   * Available only when scope, normalized evidence and configuration are identical.
   */
  delta?: number | null;
  changes: UseRecordChange[];
  changesComplete: boolean;
}
export interface UseRecordChange {
  operationId: string;
  threadId: string;
  itemId?: string | null;
  before?: UseReason | null;
  after?: UseReason | null;
  beforeDelta?: number | null;
  afterDelta?: number | null;
}
export interface UseSourceIssue {
  code: string;
  sourceInstanceId?: string | null;
  evidence?: EvidenceRef | null;
}
export interface ExtensionReview {
  methodVersion: number;
  enabledItems: number;
  disabledItems: number;
  unconfirmedItems: number;
  estimatedItems: number;
  unmeasuredItems: number;
  /**
   * Native listings do not expose model selection, aliases or prompt budget truncation.
   */
  confirmedVisibleTokens?: number | null;
}
export interface ExtensionActivityStatistics {
  methodVersion: number;
  observedUse: number;
  noObservedUse: number;
  unavailable: number;
  scope: Scope;
  checkedAt: string;
  /**
   * Same page as inventory items; summary counts are computed before pagination.
   */
  items: ExtensionActivity[];
}
export interface ExtensionActivity {
  itemId: string;
  observedRecords?: number | null;
  /**
   * Whole elapsed 24-hour periods without a recorded use in the selected window.
   * Does not establish continuous enablement, loading, or complete observation.
   */
  noObservedUseDays?: number | null;
  lastRecordAt?: string | null;
  absenceObservable: boolean;
  useBasis?: UseBasis | null;
}
export interface UseBasis {
  methodVersion: number;
  status: UseBasisStatus;
  unit: UseUnit;
  /**
   * Observer cutoff, distinct from individual event times and source dispatch.
   */
  capturedAt: string;
  snapshotId?: string | null;
  scope: UseScope;
  timeBasis: UseTimeBasis;
  coverage: UseCoverage;
  /**
   * Completeness of selected source reports, not proof of all native use mechanisms.
   */
  sourceCompleteness: UseSourceCompleteness;
}
export interface UseScope {
  sourceInstanceIds: string[];
  project?: string | null;
  threadId?: string | null;
  agentKind?: string | null;
  window: UseWindow;
}
export interface UseCoverage {
  dispatchGaps?: number | null;
  identityGaps?: number | null;
  targetGaps?: number | null;
  timeGaps?: number | null;
  turnGaps?: number | null;
}
export interface Capabilities {
  historicalContentHashes: boolean;
  kinds: Kind[];
  evidenceTypes: string[];
  tokenEstimates: boolean;
  historicalContent: boolean;
  writes: boolean;
  projectRegistry: boolean;
}
export interface Summary {
  currentItems: number;
  historicalItems: number;
  observedItems: number;
  usage?: UsageSummary | null;
}
export interface UsageSummary {
  /**
   * All input, including cache reads and writes. Existing tokens.input stays uncached.
   */
  inputTotal?: number | null;
  cacheHitRate?: number | null;
  unpricedTokens?: number | null;
  tokens: TokenUsage;
  tokenAnalysis: TokenAnalysis;
  price: PriceResult;
  measurementCount: number;
}
export interface TokenUsage {
  input?: number | null;
  cacheRead?: number | null;
  cacheCreate?: number | null;
  output?: number | null;
  /**
   * Subset of output, never an additional billable category.
   */
  reasoning?: number | null;
  total?: number | null;
  /**
   * Source input including caches; retained for request-level price conditions.
   */
  rawInput?: number | null;
}
/**
 * Token subtotals are scoped to the canonical measurements selected by this query.
 * Existing `tokens` fields remain complete totals; partial observations live here.
 */
export interface TokenAnalysis {
  methodVersion: number;
  scope: TokenAnalysisScope;
  fields: TokenFields;
  /**
   * Independent analysis; immutable historical review items may contain only
   * the native observations captured when the user made the decision.
   */
  totalAnalysis?: AnalyzedTokenTotal | null;
}
export interface TokenFields {
  input: ObservedTokenSubtotal;
  cacheRead: ObservedTokenSubtotal;
  cacheCreate: ObservedTokenSubtotal;
  output: ObservedTokenSubtotal;
  reasoning: ObservedTokenSubtotal;
  total: ObservedTokenSubtotal;
  rawInput: ObservedTokenSubtotal;
}
/**
 * An observed subtotal never implies that unavailable records contributed zero.
 */
export interface ObservedTokenSubtotal {
  observedSubtotal?: number | null;
  coveredRecords: number;
  missingRecords: number;
  conflictingRecords: number;
  invalidRecords: number;
  indeterminateRecords: number;
}
export interface AnalyzedTokenTotal {
  methodVersion: number;
  subtotal?: number | null;
  coveredRecords: number;
  recordedRecords: number;
  /**
   * Unavailable native totals use recorded input (including caches) plus output
   * only for an identified response grain. Reasoning is already in output.
   */
  calculatedRecords: number;
  unavailableRecords: number;
  /**
   * Alternative records excluded by individual or combined safe-integer limits.
   */
  overflowRecords: number;
}
export interface PriceResult {
  currency: string;
  policy: string;
  priceRevision: string;
  /**
   * Present only when every applicable Token category has a known amount.
   */
  cost?: string | null;
  knownCost: string;
  /**
   * priced, partial, unknown
   */
  status: string;
  components: PriceComponent[];
  basis: PriceBasis[];
  issues: string[];
}
export interface PriceComponent {
  category: string;
  tokens?: number | null;
  cost?: string | null;
  knownCost: string;
  status: string;
  ratePerMillion?: string | null;
}
export interface PriceBasis {
  originalModel: string;
  pricingModel: string;
  modelProvider: string;
  apiProvider?: string | null;
  matchMethod: string;
  source: string;
  verifiedAt: string;
  priceRevision: string;
  catalogHash: string;
  /**
   * standard, longContext, conditionUnknown
   */
  condition: string;
  requestInputTokens?: number | null;
  requestScoped: boolean;
}
export interface Item {
  id: string;
  name: string;
  kind: Kind;
  sourceInstanceId: string;
  path: string;
  project?: string | null;
  nativeKey?: string | null;
  /**
   * Authorized inventory memberships, not proof of joint host loading.
   */
  authorizedProjects: string[];
  /**
   * Current physical object keeps each source inventory identity and observation.
   */
  sourceContexts: SourceContext[];
  configuredState: string;
  contentHash: string;
  observedAt: string;
  current: boolean;
  stale: boolean;
  bytes?: number | null;
  contentTokens?: number | null;
  estimateStatus: string;
  characters?: number | null;
  measurementStatus: string;
  bytesSource?: string | null;
  estimate?: ContentEstimate | null;
  skillMetadata?: SkillMetadata | null;
  bodyTokenEstimate?: ContentEstimate | null;
  bodyEstimateStatus: string;
  contextFootprint?: ContextFootprint | null;
  usageCount?: number | null;
  useBasis?: UseBasis | null;
  lastRecordAt?: string | null;
  observation: Observation;
  counts: Counts;
  relatedTurns: number;
  relatedTasks: number;
  usage?: UsageSummary | null;
}
export interface SourceContext {
  inventoryId: string;
  global: boolean;
  sourceInstanceId: string;
  contentHash: string;
  configuredState: string;
  counts: Counts;
  observation: Observation;
  lastRecordAt?: string | null;
}
export interface Counts {
  fileReads: number;
  toolCalls: number;
  resourceReads: number;
  succeeded: number;
  failed: number;
  outcomeUnknown: number;
}
export interface ContentEstimate {
  tokens: number;
  encoding: string;
  method: string;
  payload: string;
  contentHash: string;
  applicability: string;
  tokenizerVersion?: string | null;
}
export interface SkillMetadata {
  status: string;
  descriptionCharacters?: number | null;
  issues: string[];
  /**
   * Bounded current-file diagnostics for directly reviewable static fields.
   */
  diagnostics: SkillDiagnostic[];
}
export interface SkillDiagnostic {
  code: string;
  field?: string | null;
  line?: number | null;
  column?: number | null;
  current?: string | null;
  expected?: string | null;
}
/**
 * A reference catalog row, never proof of model-visible context or historical loading.
 */
export interface ContextFootprint {
  status: string;
  estimate?: ContentEstimate | null;
  renderer?: string | null;
  nativeVersion?: string | null;
  checkedAt?: string | null;
  contexts: SkillContext[];
  visibility: string;
}
export interface SkillContext {
  project: string;
  name: string;
  enabled: boolean;
  scope: string;
}
export interface Evidence {
  id: string;
  sourceInstanceId?: string | null;
  itemId: string;
  threadId: string;
  turnId?: string | null;
  title?: string | null;
  project?: string | null;
  timestamp?: string | null;
  eventType: string;
  outcome: string;
  association: string;
  usage?: UsageSummary | null;
}
export interface RelatedScope {
  project?: string | null;
  evidenceCount: number;
  usage?: UsageSummary | null;
}
export interface Coverage {
  /**
   * Whether selected source bytes were read completely; independent of attribution gaps.
   */
  historyReadComplete?: boolean | null;
  status: string;
  historyStatus: string;
  issues: Issue[];
  supportedEvidence: string[];
  absenceObservable: boolean;
}
export interface Issue {
  code: string;
  path?: string | null;
}
/**
 * A current native registry observation; it never proves that a Hook ran.
 */
export interface HookRegistry {
  nativeVersion?: string | null;
  checkedAt?: string | null;
  status: HookRegistryStatus;
  contexts: HookContext[];
}
export interface HookContext {
  project: string;
  complete: boolean;
  registrations: HookRegistration[];
}
export interface HookRegistration {
  itemId: string;
  nativeKey: string;
  contentHash: string;
  registrationHash: string;
  enabled: boolean;
  trust: HookTrust;
  handler: HookHandler;
  source: string;
  pluginId?: string | null;
}
export interface Failure {
  code: string;
  message: string;
}
export interface Section2 {
  state: State;
  result?: Response2 | null;
  error?: Failure | null;
}
export interface Response2 {
  outputVersion: number;
  action: Action2;
  capabilities: Capabilities2;
  configRevision: string;
  usageRevision?: string | null;
  decisionRevision: string;
  checkedAt: string;
  suggestions: Suggestion[];
  pending: number;
  page: Page;
  issues: Issue[];
  resultStatus: string;
  ruleParameters: RuleParameters;
  ruleCatalog: RuleDefinition[];
  checks: RuleAssessment[];
  activity?: ActivityResult | null;
}
export interface Capabilities2 {
  staticChecks: boolean;
  manualEditReview: boolean;
  decisions: boolean;
  exactInstructionBlocks: boolean;
  declaredCopyDrift: boolean;
  hookSupport: HookSupport;
}
export interface HookSupport {
  host?: string | null;
  hostVersion?: string | null;
  effectiveRegistry: boolean;
  status: HookSupportStatus;
}
export interface Suggestion {
  baselineId?: string | null;
  reviewFormatVersion: number;
  scopeProject?: string | null;
  id: string;
  item: Item;
  category: Category;
  decision?: UserDecision | null;
  /**
   * Current rule facts; user decisions never stand in for check outcomes.
   */
  checks: RuleAssessment[];
  findings: Finding[];
  checkedAt: string;
  ruleVersion: string;
  ruleParameters?: RuleParameters | null;
}
export interface UserDecision {
  binding: DecisionBinding;
  kind: DecisionKind;
  reason: DecisionReason;
  recordedAt: string;
}
export interface DecisionBinding {
  identityBasis: DecisionIdentityBasis;
  version: number;
  suggestionId: string;
  findingIds: string[];
  assessmentIds: string[];
  contentVersion: string;
  scope: AssessmentScope;
  /**
   * Includes relevant dependencies, methods and parameters, but no check cutoff or log revision.
   */
  applicabilityId?: string | null;
  gap?: string | null;
}
export interface AssessmentScope {
  sourceInstanceId?: string | null;
  itemProject?: string | null;
  global: boolean;
  project?: string | null;
  sourceInstances: string[];
  authorizedProjects: string[];
  roots: string[];
  projectRoots: string[];
  sourceRoots: string[];
  complete: boolean;
}
export interface RuleAssessment {
  assessmentId?: string | null;
  identityGap?: string | null;
  ruleSemanticsVersion: number;
  methodVersions: MethodVersion[];
  basis: AssessmentBasis;
  comparison: AssessmentComparison;
  rule: string;
  ruleVersion: string;
  itemId: string;
  contentVersion: string;
  checkedAt: string;
  outcome: RuleOutcome;
  reason?: string | null;
  findings: Finding[];
}
export interface MethodVersion {
  method: string;
  version: number;
}
export interface AssessmentBasis {
  version: number;
  dependencyRevision?: string | null;
  scope: AssessmentScope;
  cutoff: string;
  applicability: string;
  measurement: RuleMeasurement;
  /**
   * Absent revisions never act as equality wildcards.
   */
  gaps: string[];
}
export interface AssessmentComparison {
  status: ComparisonStatus;
  baselineAssessmentId?: string | null;
  reason?: string | null;
}
export interface Finding {
  identity: FindingIdentity;
  rule: string;
  status: string;
  observed?: number | null;
  threshold?: number | null;
  evidenceCodes: string[];
  basis?: string | null;
  /**
   * Positions and relationships only; never retain source text or command arguments.
   */
  evidence?: StaticEvidence | null;
}
/**
 * Problem identity is independent of revisions, thresholds and check timestamps.
 */
export interface FindingIdentity {
  version: number;
  findingId?: string | null;
  gap?: string | null;
}
export interface StaticEvidence {
  method: string;
  applicability: string;
  declarationHash?: string | null;
  relationId?: string | null;
  direction?: string | null;
  transform?: string | null;
  versions: FileVersion[];
  positions: BlockPosition[];
  /**
   * Scope-bound owner required to prove a declared relation.
   */
  relation?: RelationIdentity | null;
  references: ReferenceEvidence[];
  hook?: HookTargetEvidence | null;
}
export interface FileVersion {
  itemId: string;
  path: string;
  contentHash: string;
}
export interface BlockPosition {
  itemId: string;
  startByte: number;
  endByte: number;
  startLine: number;
  endLine: number;
  blockHash: string;
}
export interface RelationIdentity {
  sourceInstanceId: string;
  project: string;
  declarationPath: string;
  declarationHash: string;
  relationId: string;
  kind: RelationKind;
}
export interface ReferenceEvidence {
  target: string;
  baseDirectory: string;
  expectedType?: string | null;
  status: string;
  startByte: number;
  endByte: number;
  startLine: number;
  endLine: number;
}
export interface HookTargetEvidence {
  project: string;
  nativeKey: string;
  registrationHash: string;
  hostVersion: string;
  trust: HookTrust;
  target: string;
  status: string;
}
export interface RuleParameters {
  version: string;
  agentsBytesDefault: number;
  descriptionCharactersDefault: number;
  overrides: RuleOverrides;
  bodyTokens: number;
  descriptionStandardMax: number;
  applicability: string;
}
export interface RuleDefinition {
  rule: string;
  version: string;
  kinds: Kind[];
  basis: string;
}
export interface ActivityResult {
  formatVersion: number;
  basis: ReadView;
  scope: LocalScope;
  analysisMethod: string;
  freshness: QueryFreshness;
  sourceStatus: string;
  coverage: RepeatCoverage;
  /**
   * @minItems 5
   * @maxItems 5
   */
  checks: [ActivityCheck, ActivityCheck, ActivityCheck, ActivityCheck, ActivityCheck];
  /**
   * Positive inspection signals; never fault, resolution, causal waste, or savings claims.
   *
   * @maxItems 5
   */
  advice:
    | []
    | [ActivityRule]
    | [ActivityRule, ActivityRule]
    | [ActivityRule, ActivityRule, ActivityRule]
    | [ActivityRule, ActivityRule, ActivityRule, ActivityRule]
    | [ActivityRule, ActivityRule, ActivityRule, ActivityRule, ActivityRule];
}
export interface ReadView {
  snapshotId: string;
  snapshotSchema: number;
  createdAt: string;
  adapterVersions: string[];
  projectionVersion: number;
}
export interface LocalScope {
  sourceInstanceId: string;
  threadId: string;
  turnId: string;
  agentKind: string;
  wholeTurn: boolean;
}
/**
 * Internal selector observation; not a request option or source capability.
 */
export interface QueryFreshness {
  status: string;
  checkedAt?: string | null;
  revision?: number | null;
  errorCode?: string | null;
}
export interface RepeatCoverage {
  candidateOperations: TimingMetricUint64;
  eligibleCommands: TimingMetricUint64;
  missingIdentityRecords: TimingMetricUint64;
  excludedReceivers: TimingMetricUint64;
  missingMatching: TimingMetricUint64;
  conflictingOperations: TimingMetricUint64;
  missingStart: TimingMetricUint64;
  indeterminateOutcomes: TimingMetricUint64;
  orderGaps: TimingMetricUint64;
  contextBoundaries: TimingMetricUint64;
  crossedContext: TimingMetricUint64;
  missingClockDomain: TimingMetricUint64;
  sourceMetadataGaps: TimingMetricUint64;
  durationConflicts: TimingMetricUint64;
  partial: boolean;
  reasonCodes: RepeatCoverageReason[];
}
export interface TimingMetricUint64 {
  value: number | null;
  status: MetricStatus;
  basis: Basis;
  evidenceRefs: string[];
}
export interface ActivityCheck {
  inputChange?: InputChange | null;
  inputPolicy?: InputChangePolicy | null;
  outcomes?: OutcomeStatistics | null;
  failurePolicy?: FailureSharePolicy | null;
  rule: ActivityRule;
  version: number;
  method: string;
  outcome: RuleOutcome;
  observed: TimingMetricUint64;
  partial: boolean;
  reason?: ActivityReason | null;
}
export interface InputChange {
  method: InputChangeMethod;
  availability: Capability;
  statistics?: InputChangeStatistics | null;
}
export interface Capability {
  support: Support;
  reason: Basis;
}
export interface InputChangeStatistics {
  candidates: number;
  orderedSamples: number;
  nonRequestScoped: number;
  missingInput: number;
  unassociated: number;
  numericRange: number;
  comparableStages: number;
  increasingStages: number;
  decreasingStages: number;
  unchangedStages: number;
  /**
   * Zero means comparable stages had no positive first-to-last change; null means no comparison.
   */
  maximumIncrease: TimingMetricUint64;
  largestIncrease?: InputChangeStage | null;
  /**
   * @maxItems 32
   */
  stages: InputChangeStage[];
  detailsOmitted: boolean;
  partial: boolean;
}
export interface InputChangeStage {
  id: string;
  samples: number;
  firstRef: string;
  lastRef: string;
  firstInput: number;
  lastInput: number;
  delta: number;
  factor?: number | null;
}
export interface InputChangePolicy {
  minimumIncrease: number;
}
/**
 * Counts in the captured subset, rather than inferred complete-turn totals.
 */
export interface OutcomeStatistics {
  method: OutcomeMethod;
  determinateOperations: TimingMetricUint64;
  succeeded: TimingMetricUint64;
  failed: TimingMetricUint64;
  interrupted: TimingMetricUint64;
  rejected: TimingMetricUint64;
  nonterminal: TimingMetricUint64;
  indeterminate: TimingMetricUint64;
  conflicting: TimingMetricUint64;
  /**
   * Missing-identity observations use physical-record units, unlike canonical group counts.
   */
  identityGapRecords: TimingMetricUint64;
  unclassified: TimingMetricUint64;
  failureRatio: TimingMetricDouble;
  partial: boolean;
}
export interface TimingMetricDouble {
  value: number | null;
  status: MetricStatus;
  basis: Basis;
  evidenceRefs: string[];
}
export interface FailureSharePolicy {
  minimumDeterminate: number;
  minimumFailures: number;
  minimumRatio: number;
}
export interface Section3 {
  state: State;
  result?: Response3 | null;
  error?: Failure | null;
}
export interface Response3 {
  outputVersion: number;
  result: Response4;
  freshness: Freshness;
  additional?: Response4[];
}
export interface Response4 {
  statistics?: TaskStatistics | null;
  inspection?: Inspection | null;
  comparison?: Comparison | null;
  facets?: Facets | null;
  distribution?: Distribution | null;
  priceUpdate?: Automatic | null;
  freshness?: Freshness | null;
  outputVersion: number;
  action: Action3;
  snapshotRef: SnapshotRef;
  scope: Scope2;
  availableRange: AvailableRange;
  summary: UsageSummary;
  items: Item2[];
  page: Page;
  quality: Quality;
}
export interface TaskStatistics {
  selectedTask?: TaskTypicality | null;
  methodVersion: number;
  /**
   * Linear interpolation at (n-1)p, shared with timing distributions.
   */
  quantileMethod: string;
  population: TaskPopulation;
  dimension?: Presentation | null;
  /**
   * A task using several models occurs in each model population; counts are not additive.
   */
  groups: TaskStatisticsGroup[];
  growth?: TaskGrowth | null;
}
export interface TaskTypicality {
  threadId: string;
  tokens?: number | null;
  completePopulationTasks: number;
  /**
   * Midrank: (number below + half of ties) / complete sample size.
   */
  percentileRank?: number | null;
  aboveP90?: boolean | null;
}
export interface TaskPopulation {
  measuredTasks: number;
  completeTasks: number;
  incompleteTasks: number;
  completeTaskTokens?: number | null;
  meanTokens?: number | null;
  medianTokens?: number | null;
  p90Tokens?: number | null;
  unassignedUsage: UsageSummary;
}
export interface TaskStatisticsGroup {
  key?: string | null;
  population: TaskPopulation;
}
export interface TaskGrowth {
  baselineScope: Scope2;
  baseline: TaskPopulation;
  taskCountDelta: number;
  meanTokensDelta?: number | null;
  /**
   * Symmetric arithmetic decomposition: ΔN × (μ0+μ1)/2.
   * Only available for complete, nonempty populations; never a causal attribution.
   */
  taskCountContribution?: number | null;
  /**
   * Δμ × (N0+N1)/2; sums with count contribution to attributed token growth.
   */
  perTaskContribution?: number | null;
  attributedTokenDelta?: number | null;
  unassignedTokenDelta?: number | null;
}
export interface Scope2 {
  allTime?: boolean | null;
  timezone?: string | null;
  since?: string | null;
  until?: string | null;
  agentKind?: string | null;
  sourceInstanceId?: string | null;
  model?: string | null;
  modelUnknown?: boolean | null;
  effortUnknown?: boolean | null;
  undated?: boolean | null;
  reasoningEffort?: string | null;
  project?: string | null;
  projectUnknown?: boolean | null;
  threadId?: string | null;
  /**
   * Exact turn inspection, always bound to a selected thread.
   */
  turnId?: string | null;
}
export interface Inspection {
  methodVersion: number;
  kind: InspectionKind;
  policy: InspectionPolicy;
  partial: boolean;
  limitations: InspectionLimit[];
  candidates: InvestigationCandidate[];
  trajectory: InputPoint[];
  resources: ResourceHotspot[];
  review?: PeriodReview | null;
  candidateCount: number;
  resourceCount: number;
  unlocatedOperations: number;
  context?: ContextInventory | null;
  activity?: ActivityReview | null;
  opportunities?: OpportunityReview | null;
}
export interface InspectionPolicy {
  minimumTokens: number;
  minimumInput: number;
  maximumCacheShare: number;
  minimumInputJump: number;
  minimumDeterminateOperations: number;
  minimumFailures: number;
  minimumFailureShare: number;
  minimumRepeatedRequests: number;
}
export interface InvestigationCandidate {
  threadId: string;
  title?: string | null;
  signals: InspectionSignal[];
  usage: UsageSummary;
  input?: number | null;
  cacheShare?: number | null;
  largestUncachedJump?: number | null;
  determinateOperations: number;
  failedOperations: number;
  outcomeGaps: number;
  /**
   * Identical callable/argument observations inside one exact turn/receiver.
   */
  repeatedRequests: number;
  evidence: InspectionEvidence[];
}
export interface InspectionEvidence {
  view: InspectionEvidenceView;
  methodVersion: number;
  snapshotId: string;
  scope: Scope2;
  threadId: string;
  turnId?: string | null;
  operationId?: string | null;
}
export interface InputPoint {
  measurementId: string;
  timestamp?: string | null;
  input?: number | null;
  uncachedInput?: number | null;
  cacheRead?: number | null;
  inputDelta?: number | null;
  uncachedDelta?: number | null;
  boundary?: InputBoundary | null;
  epoch: number;
  evidence: InspectionEvidence;
  /**
   * Two observations separated by a compaction marker; not a continuous delta or causal effect.
   */
  compactionComparison?: CompactionInputComparison | null;
}
export interface CompactionInputComparison {
  beforeMeasurementId: string;
  beforeInput: number;
  afterInput: number;
  inputDifference: number;
  beforeEvidence: InspectionEvidence;
}
export interface ResourceHotspot {
  id: string;
  sourceInstanceId: string;
  project?: string | null;
  path: string;
  /**
   * Verified lexical read target or unnormalized source-reported change path.
   */
  identityBasis: string;
  operations: number;
  reads: number;
  proposedChanges: number;
  reportedChanges: number;
  failedOperations: number;
  knownDurationMs?: number | null;
  durationCoveredOperations: number;
  /**
   * Source write reports do not establish independently observed disk changes.
   */
  actualChanges?: number | null;
  evidence: InspectionEvidence[];
}
export interface PeriodReview {
  comparison?: Comparison | null;
  topTasks: InvestigationCandidate[];
  models: ReviewGroup[];
  tools: ToolFamilyCount[];
  remainingModelUsage: UsageSummary;
  weekStart?: string | null;
  concentration?: ReviewConcentration | null;
}
export interface ComparedUsage {
  scope: Scope2;
  usage: UsageSummary;
  /**
   * Includes source coverage and windows not yet closed at publication time.
   */
  partial: boolean;
}
export interface UsageDelta {
  /**
   * Complete analyzed totals only; missing observations never become zero.
   */
  tokens?: number | null;
  /**
   * Complete configured valuations only, with exact decimal subtraction.
   */
  cost?: string | null;
  /**
   * No percentage for a zero or unavailable baseline.
   */
  tokenRatio?: number | null;
}
export interface UsageDriver {
  key?: string | null;
  baseline: ComparedUsage;
  current: ComparedUsage;
  delta: UsageDelta;
}
export interface ComparedSession {
  threadId: string;
  title?: string | null;
  own: UsageSummary;
  descendants: UsageSummary;
  selected: UsageSummary;
  memberCount: number;
  /**
   * Conflicting/cyclic ancestry, absent parents or incomplete source coverage.
   */
  partial: boolean;
  scope: Scope2;
}
export interface ReviewGroup {
  key?: string | null;
  usage: UsageSummary;
}
export interface ToolFamilyCount {
  kind: string;
  operations: number;
  failed: number;
}
export interface ReviewConcentration {
  methodVersion: number;
  measuredTasks: number;
  totalTokens?: number | null;
  topTaskTokens?: number | null;
  topTaskShare?: number | null;
  topFiveTokens?: number | null;
  topFiveShare?: number | null;
  topTenTokens?: number | null;
  topTenShare?: number | null;
  remainingTaskUsage: UsageSummary;
}
export interface ContextInventory {
  observedRecords: number;
  injectedRecords: number;
  modelWindowRecords: number;
  /**
   * Physical safe records, not logical requests, messages or injection counts.
   */
  records: ContextInventoryRecord[];
}
export interface ContextInventoryRecord {
  id: string;
  kind: ContextRecordKind;
  timestamp?: string | null;
  recordKind?: string | null;
  phase?: string | null;
  presence?: string | null;
  model?: string | null;
  modelContextWindow?: number | null;
  /**
   * Native source records do not retain historical resource versions or measured bodies.
   */
  contentVersion?: string | null;
  bytes?: number | null;
  evidence: InspectionEvidence;
}
export interface ActivityReview {
  methodVersion: number;
  policy: ActivityPolicy;
  /**
   * Same authorized scope, preceding disjoint dates; absent for unbounded selections.
   */
  baselineScope?: Scope2 | null;
  currentCoverage: ActivityCoverage;
  baselineCoverage?: ActivityCoverage | null;
  findings: ActivityFinding[];
  findingCount: number;
  limit: number;
}
export interface ActivityPolicy {
  slowDurationMs: number;
  minimumSlowOperations: number;
  minimumCurrentOutcomes: number;
  minimumBaselineOutcomes: number;
  minimumSpikeFailures: number;
  minimumFailureShare: number;
  failureShareMultiplier: number;
  baselineFailureShareFloor: number;
  minimumBaselineDurations: number;
  minimumBaselineMedianMs: number;
  durationMultiplier: number;
  minimumDurationIncreaseMs: number;
  minimumWorkflowOperations: number;
  minimumWorkflowTasks: number;
  minimumFailuresInTask: number;
  minimumRejectionsInTask: number;
}
export interface ActivityCoverage {
  observedOperations: number;
  matchedOperations: number;
  outcomeGaps: number;
  durationSamples: number;
}
export interface ActivityFinding {
  id: string;
  signals: ActivitySignal[];
  sourceInstanceId: string;
  project?: string | null;
  tool: string;
  current: ActivityStats;
  baseline?: ActivityStats | null;
  evidence: InspectionEvidence[];
  baselineEvidence: InspectionEvidence[];
}
export interface ActivityStats {
  operations: number;
  tasks: number;
  determinateOperations: number;
  failedOperations: number;
  rejectedOperations: number;
  failureShare?: number | null;
  outcomeGaps: number;
  durationSamples: number;
  slowOperations: number;
  maximumDurationMs?: number | null;
  medianDurationMs?: number | null;
  maximumFailuresInTask: number;
  maximumRejectionsInTask: number;
}
export interface OpportunityReview {
  methodVersion: number;
  policy: OpportunityPolicy;
  checks: OpportunityCheck[];
  limitPerCheck: number;
}
export interface OpportunityPolicy {
  minimumUnpricedTokens: number;
  minimumAmountUsd: string;
  concentrationShare: number;
  outlierMultiple: number;
  increaseMultiple: number;
  minimumIncreaseUsd: string;
  minimumCacheCreated: number;
  maximumReadCreateRatio: number;
  modelShare: number;
  maximumMedianOperations: number;
  minimumModelTasks: number;
  minimumOutlierTasks: number;
  minimumPermissionRequests: number;
  permissionRequestShare: number;
  longInteractionMs: number;
  outboundWindowMs: number;
  minimumRiskyDeclines: number;
  minimumPolls: number;
  maximumPollWaitMs: number;
  minimumObservedTasks: number;
  minimumPollingTasks: number;
  minimumPollingDays: number;
  pollingWindowDays: number;
}
export interface OpportunityCheck {
  rule: OpportunityRule;
  status: OpportunityStatus;
  gaps: OpportunityGap[];
  findingCount: number;
  findings: OpportunityFinding[];
}
export interface OpportunityFinding {
  id: string;
  object?: string | null;
  metrics: OpportunityMetric[];
  safetyLabels: SafetyLabel[];
  evidence: InspectionEvidence[];
  baselineEvidence: InspectionEvidence[];
}
export interface OpportunityMetric {
  name: OpportunityMetricName;
  /**
   * Exact decimal representation; no client-side measurement or ratio calculation.
   */
  value?: string | null;
  unit: OpportunityUnit;
}
/**
 * Observed dimensions, not a project registry or a configuration inventory.
 */
export interface Facets {
  /**
   * Metadata across this snapshot's authorized sources, independent of measurement/date filters.
   */
  discoveredThreadCount?: number | null;
  directories: string[];
  hasUnassigned: boolean;
  models: string[];
  reasoningEfforts: string[];
  agents: string[];
}
export interface Distribution {
  unpricedTokens?: number | null;
  /**
   * Basis for maxTokens and peak token scopes/dates.
   */
  tokenBasis: TokenBasis;
  /**
   * Maximum of bucket analyzed subtotals; not necessarily a complete total.
   */
  maxTokens?: number | null;
  maxCost?: string | null;
  peakTokenDates: (string | null)[];
  peakCostDates: (string | null)[];
  peakTokenScopes: Scope2[];
  peakCostScopes: Scope2[];
}
export interface Automatic {
  status: AutomaticStatus;
  attemptId: string;
  attemptedAt: string;
  retryAt: string;
  errorCode?: string | null;
}
export interface Freshness {
  publicationChange?: PublicationChange | null;
  /**
   * Projects restored from committed facts; pending projects are not zero usage.
   */
  projectLoads?: ProjectLoad[];
  status: string;
  /**
   * Ephemeral task headers; no completed ledger or coverage is available yet.
   */
  initialScan?: boolean;
  checkedAt?: string | null;
  revision: string;
  error?: string | null;
  errorCode?: string | null;
}
export interface PublicationChange {
  methodVersion: number;
  baseline: SnapshotRef;
  current: SnapshotRef;
  measurementsAdded: number;
  measurementsRemoved: number;
  measurementsChanged: number;
  threadsAdded: number;
  turnsChanged: number;
  pricesChanged: boolean;
  coverageChanged: boolean;
  delta: UsageDelta;
}
export interface SnapshotRef {
  snapshotId: string;
  createdAt: string;
}
export interface ProjectLoad {
  project?: string | null;
  state: ProjectLoadState;
}
export interface AvailableRange {
  since?: string | null;
  /**
   * Exclusive local date boundary.
   */
  until?: string | null;
}
export interface Quality {
  /**
   * Issues and source statuses describe the whole snapshot, not this query window.
   */
  issueScope: QualityIssueScope;
  /**
   * Snapshot warnings without query-bound attribution cannot prove a scoped defect.
   */
  queryImpact: QualityQueryImpact;
  status: string;
  issues: Issue2[];
  sources: SourceReport[];
  detailSummary?: QualityDetailSummary | null;
}
export interface Issue2 {
  code: string;
  message: string;
  sourceInstanceId?: string | null;
  evidence?: EvidenceRef | null;
}
export interface SourceReport {
  source: SourceInstance;
  adapterVersion: string;
  sourceVersions?: string[];
  capabilities: Capabilities3;
  /**
   * complete, partial, failed, notFound, cancelled
   */
  status: string;
  filesRead: number;
  bytesRead: number;
  issues: Issue2[];
}
export interface SourceInstance {
  id: string;
  agentKind: string;
  root: string;
}
export interface Capabilities3 {
  usage: boolean;
  threads: boolean;
  turns: boolean;
  operations: boolean;
  reasoningEffort: boolean;
  responseIdentity: boolean;
  measurementGrain: string[];
}
export interface QualityDetailSummary {
  issueCount: number;
  sourceCount: number;
  issueCounts: {
    [k: string]: number;
  };
  sourceStatusCounts: {
    [k: string]: number;
  };
  omittedIssues: number;
  omittedSources: number;
  omittedSourceIssues: number;
}
export interface Section4 {
  state: State;
  result?: Response5 | null;
  error?: Failure | null;
}
export interface Response5 {
  record?: ResolvedRecord | null;
  outputVersion: number;
  action: Action4;
  snapshotId: string;
  selection: Selection;
  items: Activity[];
  fragments: Fragment[];
  nextCursor?: string | null;
  /**
   * All matching indexed positions were visited; source completeness is separate.
   */
  scanComplete: boolean;
  partial: boolean;
  limits: string[];
  scannedEvents: number;
  sourceBytesRead: number;
  freshness?: Freshness | null;
  /**
   * When present, completeness applies only to this requested source window.
   */
  window?: ContextWindow | null;
}
export interface ResolvedRecord {
  id: string;
  upstreamId: string;
  sourceInstanceId: string;
  project?: string | null;
  title?: string | null;
}
export interface Selection {
  since?: string | null;
  until?: string | null;
  timezone?: string | null;
  includeUndated?: boolean;
  sources?: string[];
  projects?: string[];
  threads?: string[];
  turns?: string[];
  origins?: MessageOrigin[];
  phases?: ContentPhase[];
  recordKinds?: MessageRecordKind[];
}
export interface Activity {
  record?: RecordSummary | null;
  /**
   * Read-target cursor at a source search match; bound to snapshot, selection and evidence.
   */
  readCursor?: string | null;
  evidence: Evidence2;
  timestamp?: string | null;
  collectedAt: string;
  project?: string | null;
  title?: string | null;
  kind: string;
  origin?: MessageOrigin | null;
  phase?: ContentPhase | null;
  nativeMessageId?: string | null;
  recordKind?: MessageRecordKind | null;
  name?: string | null;
  operationId?: string | null;
  status?: string | null;
  hasTextLocator: boolean;
  contentPartial: boolean;
  change?: Change | null;
}
export interface RecordSummary {
  upstreamId: string;
  startedAt?: string | null;
  lastActivityAt?: string | null;
}
export interface Evidence2 {
  eventId: string;
  sourceInstanceId: string;
  threadId?: string | null;
  turnId?: string | null;
  /**
   * Exact position in the fixed target partition, not a raw file offset.
   */
  eventOffset: number;
}
export interface Fragment {
  evidence: Evidence2;
  message: Activity;
  text: string;
  nextCursor?: string | null;
  locations: ContentLocation[];
  context: Activity[];
  partial: boolean;
}
export interface ContentLocation {
  file: string;
  byteStart: number;
  byteEnd: number;
  part: number;
}
export interface ContextWindow {
  anchor: Evidence2;
  radius: number;
}
export interface Section5 {
  state: State;
  result?: Response6 | null;
  error?: Failure | null;
}
export interface Response6 {
  outputVersion: number;
  policyVersion: number;
  action: string;
  projectIdentity: string;
  baselineId: string;
  checkedAt: string;
  scope: Scope3;
  items: Item3[];
  summary: Summary2;
  partial: boolean;
}
export interface Scope3 {
  projectRoot: string;
  paths: string[];
  keepPaths: string[];
  productTemporary: boolean;
  codex: boolean;
}
export interface Item3 {
  id: string;
  path: string;
  owner?: string | null;
  kind: Kind2;
  assessment: Assessment;
  regeneration: Regeneration;
  regenerationVersion?: string | null;
  useState: UseState;
  observation?: Observation2 | null;
  reasons: Reason[];
  recheck?: RecheckState | null;
  codexEvidence?: CodexEvidence | null;
  removal?: Removal | null;
}
export interface Observation2 {
  physicalIdentity: string;
  fingerprint: string;
  logicalBytes: number;
  allocatedBytes?: number | null;
  files: number;
  complete: boolean;
}
export interface CodexEvidence {
  parentIdentity: string;
  installLockIdentity?: string | null;
  minimumAgeDays?: number | null;
  currentRelease?: string | null;
  pendingRelease?: string | null;
  installedBuild?: number | null;
  stagedBuild?: number | null;
}
export interface Summary2 {
  candidates: number;
  review: number;
  protected: number;
  unknown: number;
  absent: number;
  /**
   * Deduplicated observed logical file bytes, never a reclaim estimate.
   */
  observedLogicalBytes: number;
  complete: boolean;
}
