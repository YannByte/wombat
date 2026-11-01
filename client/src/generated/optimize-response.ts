/* Generated from Rust. Run pnpm contracts:generate. */

export type Action = "list" | "detail" | "keep" | "not_applicable" | "recheck" | "capabilities" | "checks" | "activity";
export type HookSupportStatus = "no_verified_adapter" | "registry_observed" | "registry_partial";
export type Kind = "rule" | "skill" | "mcp" | "hook";
export type Observation = "used" | "loaded_only" | "unknown";
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
export type TokenAnalysisScope = "selected_canonical_measurements";
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
export type HookTrust = "managed" | "untrusted" | "trusted" | "modified";
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

export interface Response {
  outputVersion: number;
  action: Action;
  capabilities: Capabilities;
  readView?: string | null;
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
export interface Capabilities {
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
export interface RuleOverrides {
  agentsBytes?: number | null;
  descriptionCharacters?: number | null;
}
export interface Page {
  offset: number;
  limit: number;
  total: number;
  nextOffset?: number | null;
}
export interface Issue {
  code: string;
  path?: string | null;
}
export interface RuleDefinition {
  rule: string;
  version: string;
  kinds: Kind[];
  basis: string;
}
export interface ActivityResult {
  formatVersion: number;
  readView: ReadView;
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
