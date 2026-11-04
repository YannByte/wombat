/* Generated from Rust. Run pnpm contracts:generate. */

export type Request =
  | {
      method: "project";
      params: Request1;
    }
  | {
      method: "artifacts";
      params: Request2;
    }
  | {
      method: "conversation";
      params: Request3;
    }
  | {
      method: "usage";
      params: Request4;
    }
  | {
      method: "config";
      params: Request6;
    }
  | {
      method: "optimize";
      params: Request7;
    }
  | {
      method: "timing";
      params: Request8;
    }
  | {
      method: "setup";
      params: Request9;
    }
  | {
      method: "account";
      params: Request10;
    }
  | {
      method: "directories";
      params: Request11;
    }
  | {
      method: "preferences";
      params: Request12;
    }
  | {
      method: "prices";
      params: Request13;
    }
  | {
      method: "collection";
      params: Request14;
    }
  | {
      method: "monitor";
      params: Request15;
    };
export type Request2 =
  | {
      projectRoot: string;
      paths?: string[];
      keepPaths?: string[];
      productTemporary?: boolean;
      codex?: boolean;
      action: "inspect";
    }
  | {
      baseline: Response;
      /**
       * @maxItems 100
       */
      selection: string[];
      action: "apply";
    }
  | {
      baseline: Response;
      selection?: string[];
      action: "recheck";
    };
export type Kind =
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
export type Action = ("activity" | "locate" | "read" | "search") | "records" | "resolve";
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
export type ComparisonRequest =
  | {
      baselineSince: string;
      baselineUntil: string;
      dimension: DriverDimension;
      kind: "periods";
    }
  | {
      leftThreadId: string;
      rightThreadId: string;
      includeDescendants?: boolean;
      kind: "sessions";
    };
export type DriverDimension = "project" | "model" | "thread";
export type Action2 =
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
export type Group = "day" | "week" | "month";
export type Sort = "tokens" | "cost" | "recent" | "time";
export type Presentation = "distribution" | "details" | "projects" | "models";
export type Mode = "auto" | "fresh" | "cached";
export type Action3 = "list" | "detail" | "evidence" | "explain" | "related_scopes" | "capabilities";
export type Kind2 = "rule" | "skill" | "mcp" | "hook";
export type Observation2 = "used" | "loaded_only" | "unknown";
export type Sort2 =
  | "tokens"
  | "activity"
  | "size"
  | "name"
  | "content_tokens"
  | "characters"
  | "recent"
  | "review"
  | "context_tokens"
  | "least_activity";
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
export type Action4 =
  "list" | "detail" | "keep" | "not_applicable" | "recheck" | "capabilities" | "checks" | "activity";
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
export type Request8 =
  | {
      threadId: string;
      turnId: string;
      roots?: string[];
      scope?: Scope4 | null;
      mode?: Mode2 & string;
      privacyProfile?: PrivacyProfile & string;
      action: "summary";
    }
  | {
      threadId: string;
      turnId: string;
      roots?: string[];
      scope?: Scope4 | null;
      cursor?: Cursor | null;
      limit?: number;
      collection?: EvidenceSet & string;
      objectRef?: string | null;
      privacyProfile?: PrivacyProfile & string;
      action: "evidence";
    }
  | {
      privacyProfile?: PrivacyProfile & string;
      action: "capabilities";
    };
export type Mode2 = "auto" | "fresh" | "cached";
export type PrivacyProfile = "local" | "share-v1";
export type EvidenceSet = "turn_events" | "use_objects" | "use_records";
export type Action5 = "read" | "refresh" | "history";
export type Action6 = "list" | "authorize" | "revoke";
export type Purpose = "source" | "project";
export type Action7 = "get" | "set";
export type Language = "zh" | "en";
export type Action8 = "status" | "update" | "auto_update";
export type Action9 = "status" | "events" | "configure" | "pause" | "resume";
export type Mode3 = "logs" | "hooks";
export type Request15 =
  | {
      action: "list";
    }
  | {
      plan: Plan;
      action: "upsert";
    }
  | {
      id: string;
      action: "remove";
    }
  | {
      roots?: string[];
      ids: string[];
      action: "check";
    }
  | {
      notificationId: string;
      action: "acknowledge";
    };
export type Period = "day" | "week" | "month";

export interface Request1 {
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
export interface Response {
  outputVersion: number;
  policyVersion: number;
  action: string;
  projectIdentity: string;
  baselineId: string;
  checkedAt: string;
  scope: Scope;
  items: Item[];
  summary: Summary;
  partial: boolean;
}
export interface Scope {
  projectRoot: string;
  paths: string[];
  keepPaths: string[];
  productTemporary: boolean;
  codex: boolean;
}
export interface Item {
  id: string;
  path: string;
  owner?: string | null;
  kind: Kind;
  assessment: Assessment;
  regeneration: Regeneration;
  regenerationVersion?: string | null;
  useState: UseState;
  observation?: Observation | null;
  reasons: Reason[];
  recheck?: RecheckState | null;
  codexEvidence?: CodexEvidence | null;
  removal?: Removal | null;
}
export interface Observation {
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
export interface Summary {
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
export interface Request3 {
  action: Action;
  roots?: string[];
  selection?: Selection;
  limit?: number | null;
  /**
   * Skip matching conversations in Records; other actions use bound cursors.
   */
  offset?: number | null;
  cursor?: string | null;
  query?: string | null;
  targets?: ReadTarget[];
  maxScanBytes?: number | null;
  anchor?: Evidence | null;
  radius?: number | null;
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
export interface ReadTarget {
  evidence: Evidence;
  cursor?: string | null;
  maxBytes?: number | null;
  before?: number;
  after?: number;
}
export interface Evidence {
  eventId: string;
  sourceInstanceId: string;
  threadId?: string | null;
  turnId?: string | null;
  /**
   * Exact position in the fixed target partition, not a raw file offset.
   */
  eventOffset: number;
}
export interface Request4 {
  query: Request5;
  mode?: Mode & string;
  verify?: boolean;
  /**
   * Explicitly rebuild derived storage in a new generation; user records stay separate.
   */
  rebuild?: boolean;
  /**
   * Include deferred historical files even for a recent query.
   */
  includeHistory?: boolean;
  /**
   * Additional queries share the selected immutable revision. No synchronization options.
   *
   * @maxItems 8
   */
  also?:
    | []
    | [Request5]
    | [Request5, Request5]
    | [Request5, Request5, Request5]
    | [Request5, Request5, Request5, Request5]
    | [Request5, Request5, Request5, Request5, Request5]
    | [Request5, Request5, Request5, Request5, Request5, Request5]
    | [Request5, Request5, Request5, Request5, Request5, Request5, Request5]
    | [Request5, Request5, Request5, Request5, Request5, Request5, Request5, Request5];
}
export interface Request5 {
  comparison?: ComparisonRequest | null;
  action: Action2;
  roots?: string[] | null;
  scope?: Scope2;
  group?: Group | null;
  sort?: Sort | null;
  presentation?: Presentation | null;
  threadId?: string | null;
  turnId?: string | null;
  search?: string | null;
  offset?: number | null;
  limit?: number | null;
  locateThreadId?: string | null;
  locateTurnId?: string | null;
  locateOperationId?: string | null;
  matchedOnly?: boolean | null;
  /**
   * Keep totals and the requested page, with bounded quality examples and no facets.
   */
  compact?: boolean | null;
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
export interface Request6 {
  /**
   * Observe native Skill metadata and order context footprint alongside use frequency.
   */
  review?: boolean;
  action?: Action3 & string;
  roots?: string[] | null;
  projectRoots?: string[] | null;
  /**
   * Check current configuration against committed usage without scheduling collection.
   */
  cachedUsage?: boolean;
  scope?: Scope3;
  kind?: Kind2 | null;
  kinds?: Kind2[] | null;
  observation?: Observation2 | null;
  search?: string | null;
  sort?: Sort2 & string;
  itemId?: string | null;
  offset?: number | null;
  limit?: number | null;
  /**
   * Explain unresolved records rather than attributing them to every inventory item.
   */
  unassigned?: boolean;
  baseline?: UseReplayBaseline | null;
  baselineRecords?: UseTraceRecord[] | null;
}
export interface Scope3 {
  allTime?: boolean | null;
  since?: string | null;
  until?: string | null;
  timezone?: string | null;
  project?: string | null;
  agentKind?: string | null;
  sourceInstanceId?: string | null;
  threadId?: string | null;
}
export interface UseReplayBaseline {
  formatVersion: number;
  methodVersion: number;
  scope: Scope3;
  roots: string[];
  projectRoots: string[];
  kind?: Kind2 | null;
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
export interface Request7 {
  action?: Action4 & string;
  roots?: string[] | null;
  projectRoots?: string[] | null;
  baselineId?: string | null;
  baseline?: Suggestion | null;
  decisionRevision?: string | null;
  project?: string | null;
  sourceInstanceId?: string | null;
  suggestionId?: string | null;
  itemId?: string | null;
  decisionReason?: DecisionReason | null;
  category?: Category | null;
  offset?: number | null;
  limit?: number | null;
  ruleOverrides?: RuleOverrides | null;
  activity?: ActivitySelection | null;
}
export interface Suggestion {
  baselineId?: string | null;
  reviewFormatVersion: number;
  scopeProject?: string | null;
  id: string;
  item: Item2;
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
export interface Item2 {
  id: string;
  name: string;
  kind: Kind2;
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
  observation: Observation2;
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
  observation: Observation2;
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
/**
 * Fixed turn analysis, separate from configuration identities and durable handling decisions.
 */
export interface ActivitySelection {
  threadId: string;
  turnId: string;
}
export interface Scope4 {
  sourceInstanceId?: string | null;
  agentKind?: string | null;
}
export interface Cursor {
  token: string;
}
export interface Request9 {
  project?: string | null;
  roots?: string[] | null;
  includeHooks?: boolean;
}
export interface Request10 {
  action?: Action5 & string;
}
export interface Request11 {
  action?: Action6 & string;
  purpose?: Purpose | null;
  path?: string | null;
  grantId?: string | null;
}
export interface Request12 {
  action: Action7;
  language?: Language | null;
}
export interface Request13 {
  action: Action8;
}
export interface Request14 {
  action?: Action9 & string;
  mode?: Mode3 | null;
  roots?: string[] | null;
  project?: string | null;
  sourceInstanceId?: string | null;
  after?: number | null;
  limit?: number | null;
}
export interface Plan {
  id: string;
  enabled: boolean;
  period: Period;
  /**
   * Identity filters and timezone only; date, undated, thread and turn filters are rejected.
   */
  scope?: Scope2;
  /**
   * Token budget, not an account allowance or money allocation to tools.
   */
  tokenLimit?: number | null;
  /**
   * Fraction in (0,1]; 0.8 by default.
   */
  warningRatio?: number | null;
  review: boolean;
}
