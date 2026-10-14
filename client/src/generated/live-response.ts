/* Generated from Rust. Run pnpm contracts:generate. */

export type TokenAnalysisScope = "selected_canonical_measurements";
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
export type Action =
  | "refresh"
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
export type Item =
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
      scope: Scope;
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

export interface Response {
  outputVersion: number;
  result: Response1;
  freshness: Freshness;
  additional?: Response1[];
}
export interface Response1 {
  statistics?: TaskStatistics | null;
  inspection?: Inspection | null;
  comparison?: Comparison | null;
  facets?: Facets | null;
  distribution?: Distribution | null;
  priceUpdate?: Automatic | null;
  freshness?: Freshness | null;
  outputVersion: number;
  action: Action;
  snapshotRef: SnapshotRef;
  scope: Scope;
  availableRange: AvailableRange;
  summary: UsageSummary;
  items: Item[];
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
export interface TaskStatisticsGroup {
  key?: string | null;
  population: TaskPopulation;
}
export interface TaskGrowth {
  baselineScope: Scope;
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
export interface Scope {
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
  scope: Scope;
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
  scope: Scope;
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
  scope: Scope;
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
  baselineScope?: Scope | null;
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
  peakTokenScopes: Scope[];
  peakCostScopes: Scope[];
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
export interface Page {
  offset: number;
  limit: number;
  total: number;
  nextOffset?: number | null;
}
export interface Quality {
  status: string;
  issues: Issue[];
  sources: SourceReport[];
  detailSummary?: QualityDetailSummary | null;
}
export interface Issue {
  code: string;
  message: string;
  sourceInstanceId?: string | null;
  evidence?: EvidenceRef | null;
}
export interface EvidenceRef {
  file: string;
  line: number;
}
export interface SourceReport {
  source: SourceInstance;
  adapterVersion: string;
  sourceVersions?: string[];
  capabilities: Capabilities;
  /**
   * complete, partial, failed, notFound, cancelled
   */
  status: string;
  filesRead: number;
  bytesRead: number;
  issues: Issue[];
}
export interface SourceInstance {
  id: string;
  agentKind: string;
  root: string;
}
export interface Capabilities {
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
