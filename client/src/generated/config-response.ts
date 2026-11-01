/* Generated from Rust. Run pnpm contracts:generate. */

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
export type Action = "list" | "detail" | "evidence" | "related_scopes" | "capabilities";
export type Kind = "rule" | "skill" | "mcp" | "hook";
export type TokenAnalysisScope = "selected_canonical_measurements";
export type Observation = "used" | "loaded_only" | "unknown";
export type HookRegistryStatus = "unavailable" | "partial" | "observed";
export type HookTrust = "managed" | "untrusted" | "trusted" | "modified";
export type HookHandler = "command" | "mcpTool" | "prompt" | "agent";

export interface Response {
  extensionReview?: ExtensionReview | null;
  extensionActivity?: ExtensionActivityStatistics | null;
  outputVersion: number;
  action: Action;
  capabilities: Capabilities;
  readView?: string | null;
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
export interface Page {
  offset: number;
  limit: number;
  total: number;
  nextOffset?: number | null;
}
export interface Coverage {
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
