/* Generated from Rust. Run pnpm contracts:generate. */

export type Response =
  LocalResponse | ShareResponse | EvidenceResponse | UseObjectsResponse | UseRecordsResponse | CapabilitiesResponse;
export type UseSourceCoverage = "complete" | "partial" | "unknown";
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
export type Support = "supported" | "partial" | "unavailable";
export type UseObjectKind = "skill" | "mcp";
export type UseState = "used" | "candidate" | "unclassified";
export type SummaryAction = "summary";
export type LocalProfile = "local";
export type PrivacyProfile = "local" | "share-v1";
export type FailureRepeatMethod = "same_operation_after_failure_v1";
export type ReadRepeatMethod = "same_target_read_v1";
export type RepeatedReadLayer = "same_path_range_unconfirmed";
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
export type OperationCoverageReason =
  | "no_paired_operations"
  | "missing_window"
  | "unlocated_operations"
  | "identity_gaps"
  | "conflicting_operations"
  | "source_partial"
  | "resource_limit"
  | "numeric_range"
  | "detail_limit";
export type TimelinePresentation = "timeline" | "list";
export type TrackCategory = "command" | "compaction" | "reasoning" | "mcp";
export type FragmentEvidence = "event_records" | "turn_collection" | "unavailable";
export type TurnState = "running" | "completed" | "failed" | "cancelled" | "unknown";
export type InputChangeMethod = "request_input_observation_change_v1";
export type OutcomeMethod = "terminal_success_failure_subset_v1";
export type FindingKind = "fact" | "proxy" | "user_annotation";
export type CollectionKind =
  "turn_events" | "canonical_measurements" | "canonical_operations" | "source_controls" | "native_boundary_index";
export type ShareProfile = "share-v1";
export type EventPageKind = "turn_events";
export type EvidenceAction = "evidence";
export type UseObjectPageKind = "use_objects";
export type UseRecordPageKind = "use_records";
export type UseKind = "skill_read" | "mcp_tool" | "mcp_resource";
export type UseOutcome = "running" | "completed" | "failed" | "cancelled" | "interrupted" | "declined" | "unknown";
export type UseTimeBasis = "source_operation_time" | "unknown";
export type CapabilitiesAction = "capabilities";

export interface LocalResponse {
  uses: LocalUses;
  outputVersion: number;
  action: SummaryAction;
  methodVersion: string;
  profile: LocalProfile;
  privacy: Privacy;
  basis: ReadView;
  scope: LocalScope;
  capabilities: Capabilities;
  anchors: Anchors;
  time: Time;
  context: Context;
  work: Work;
  findings: Finding[];
  coverage: Coverage;
  quality: Quality;
  freshness: QueryFreshness;
  evidence: EvidenceIndex;
}
export interface LocalUses {
  totals: UseTotals;
  detail: Capability;
  limit: number;
  /**
   * @maxItems 50
   */
  objects: UseObject[];
  nextCursor?: Cursor | null;
}
export interface UseTotals {
  methodVersion: number;
  sourceCoverage: UseSourceCoverage;
  objectCount: TimingMetricUint64;
  /**
   * Canonical rows, including replay and candidate evidence; not a dispatch count.
   */
  recordCount: TimingMetricUint64;
  unboundTargetRecords: TimingMetricUint64;
  unassignedSkillRecords: TimingMetricUint64;
  unassignedMcpRecords: TimingMetricUint64;
  coverage: UseCoverage;
}
export interface TimingMetricUint64 {
  value: number | null;
  status: MetricStatus;
  basis: Basis;
  evidenceRefs: string[];
}
export interface UseCoverage {
  dispatchGaps: TimingMetricUint64;
  identityGaps: TimingMetricUint64;
  targetGaps: TimingMetricUint64;
  timeGaps: TimingMetricUint64;
  /**
   * Gaps among associated records; unassigned membership is separately reported below.
   */
  associatedTurnGaps: TimingMetricUint64;
}
export interface Capability {
  support: Support;
  reason: Basis;
}
export interface UseObject {
  objectRef: string;
  kind: UseObjectKind;
  state: UseState;
  /**
   * Resolved historical local target; never sent by share-v1.
   */
  path?: string | null;
  server?: string | null;
  project?: string | null;
  /**
   * Exact count in the positively associated canonical set; coverage explains local gaps.
   */
  associatedUseCount: TimingMetricUint64;
  /**
   * Complete turn count is unavailable when association or unassigned-membership evidence has gaps.
   */
  useCount: TimingMetricUint64;
  recordCount: TimingMetricUint64;
  unassignedTurnRecords: TimingMetricUint64;
  coverage: UseCoverage;
}
export interface Cursor {
  token: string;
}
export interface Privacy {
  profile: PrivacyProfile;
  omittedFields: string[];
  aliases: string;
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
export interface Capabilities {
  wallClock: Capability;
  nativeTtft: Capability;
  firstContentRecordDelay: Capability;
  lifecycleIntervals: Capability;
  operationIntervals: Capability;
  contextPressure: Capability;
  strictResponseGap: Capability;
  exploratoryGap: Capability;
  commandLabels: Capability;
  fileChanges: Capability;
  messageRecords: Capability;
  objectUses: Capability;
}
export interface Anchors {
  startMs: TimingMetricInt64;
  endMs: TimingMetricInt64;
}
export interface TimingMetricInt64 {
  value: number | null;
  status: MetricStatus;
  basis: Basis;
  evidenceRefs: string[];
}
export interface Time {
  repeatedBehavior: RepeatedBehavior;
  operationCoverage: OperationCoverage;
  timeline: Timeline;
  state: TurnState;
  nativeWallClockMs: TimingMetricUint64;
  derivedWallClockMs: TimingMetricUint64;
  nativeTtftMs: TimingMetricUint64;
  firstContentRecordDelayMs: TimingMetricUint64;
  boundaryDiscrepancyMs: TimingMetricInt64;
  observedWindowMs: TimingMetricUint64;
  command: Category;
  compaction: Category;
  reasoning: Category;
  mcp: Category;
  /**
   * Masks 0..15: command bit 1, compaction bit 2, reasoning bit 4, MCP bit 8.
   *
   * @minItems 16
   * @maxItems 16
   */
  intersectionMasksMs: [
    TimingMetricUint64,
    TimingMetricUint64,
    TimingMetricUint64,
    TimingMetricUint64,
    TimingMetricUint64,
    TimingMetricUint64,
    TimingMetricUint64,
    TimingMetricUint64,
    TimingMetricUint64,
    TimingMetricUint64,
    TimingMetricUint64,
    TimingMetricUint64,
    TimingMetricUint64,
    TimingMetricUint64,
    TimingMetricUint64,
    TimingMetricUint64
  ];
  coveredMs: TimingMetricUint64;
  unclassifiedMs: TimingMetricUint64;
  coverageRatio: TimingMetricDouble;
  waitingProxyMs: TimingMetricUint64;
  strictResponseGapMs: TimingMetricUint64;
  exploratoryGapMs: TimingMetricUint64;
}
export interface RepeatedBehavior {
  failureMethod: FailureRepeatMethod;
  readMethod: ReadRepeatMethod;
  endpointMethodVersion: number;
  support: Capability;
  readLayer: RepeatedReadLayer;
  afterFailure: RepeatedMetric;
  repeatedRead: RepeatedMetric;
  sameRequestObservationCount: TimingMetricUint64;
  repeatedReadRequestCount: TimingMetricUint64;
  recoverySpanSumMs: TimingMetricUint64;
  missingRecoverySpanCount: TimingMetricUint64;
  combinedOperationCount: TimingMetricUint64;
  combinedUnionMs: TimingMetricUint64;
  combinedMissingIntervalCount: TimingMetricUint64;
  coverage: RepeatCoverage;
}
export interface RepeatedMetric {
  count: TimingMetricUint64;
  duration: RepeatedDuration;
}
export interface RepeatedDuration {
  knownSumMs: TimingMetricUint64;
  recordedCount: TimingMetricUint64;
  calculatedCount: TimingMetricUint64;
  missingCount: TimingMetricUint64;
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
export interface OperationCoverage {
  methodVersion: number;
  endpointMethodVersion: number;
  candidateOperations: TimingMetricUint64;
  pairedOperations: TimingMetricUint64;
  identityGapRecords: TimingMetricUint64;
  conflictingOperations: TimingMetricUint64;
  coveredMs: TimingMetricUint64;
  residualMs: TimingMetricUint64;
  residualRangeCount: TimingMetricUint64;
  partial: boolean;
  reasonCodes: OperationCoverageReason[];
  detail: Capability;
  detailLimit: number;
  /**
   * @maxItems 200
   */
  residualRanges: OperationResidualRange[];
}
export interface OperationResidualRange {
  startMs: number;
  endMs: number;
}
export interface Timeline {
  presentation: TimelinePresentation;
  detail: Capability;
  entryCount: TimingMetricUint64;
  trackCount: TimingMetricUint64;
  /**
   * Majority/list denominator: unique identified lifecycle intervals, including
   * excluded conflicts; records without identity are counted only in coverage.
   */
  identifiedIntervalCount: TimingMetricUint64;
  unclassifiedGapCount: TimingMetricUint64;
  /**
   * Unique identified intervals that cannot be placed; missing-identity records
   * remain in coverage and are not guessed to be separate intervals.
   */
  unlocatedIntervalCount: TimingMetricUint64;
  outsideWindowIntervalCount: TimingMetricUint64;
  detailLimit: number;
  /**
   * @maxItems 200
   */
  tracks: TimelineTrack[];
  /**
   * @maxItems 200
   */
  unclassifiedGaps: TimelineGap[];
}
export interface TimelineTrack {
  intervalAlias: string;
  category: TrackCategory;
  startMs: number;
  endMs: number;
  clipped: boolean;
  evidenceScope: FragmentEvidence;
  /**
   * @maxItems 3
   */
  evidenceRefs: [] | [string] | [string, string] | [string, string, string];
}
export interface TimelineGap {
  startMs: number;
  endMs: number;
  evidenceScope: FragmentEvidence;
  evidenceRefs: string[];
}
export interface Category {
  candidates: TimingMetricUint64;
  closed: TimingMetricUint64;
  unionMs: TimingMetricUint64;
  sumMs: TimingMetricUint64;
}
export interface TimingMetricDouble {
  value: number | null;
  status: MetricStatus;
  basis: Basis;
  evidenceRefs: string[];
}
export interface Context {
  inputChange: InputChange;
  activeContextOccupancy: TimingMetricDouble;
  compactionRecords: TimingMetricUint64;
  compactionTimeMs: TimingMetricUint64;
  method: string;
  quantileMethod: string;
  candidates: TimingMetricUint64;
  conflictingMeasurements: TimingMetricUint64;
  conflictingWindowRecords: TimingMetricUint64;
  input: Distribution;
  ratio: Distribution;
  segmentCount: TimingMetricUint64;
  segments: Segment[];
  compactionNeighbors: CompactionNeighbors[];
  detail: Capability;
}
export interface InputChange {
  method: InputChangeMethod;
  availability: Capability;
  statistics?: InputChangeStatistics | null;
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
export interface Distribution {
  samples: TimingMetricUint64;
  median: TimingMetricDouble;
  p90: TimingMetricDouble;
}
export interface Segment {
  id: string;
  candidates: TimingMetricUint64;
  nonRequestScoped: TimingMetricUint64;
  missingRawInput: TimingMetricUint64;
  missingWindow: TimingMetricUint64;
  invalidWindow: TimingMetricUint64;
  sameRecordWindows: TimingMetricUint64;
  continuedWindows: TimingMetricUint64;
  aboveWindow: TimingMetricUint64;
  input: Distribution;
  ratio: Distribution;
}
export interface CompactionNeighbors {
  evidenceRef: string;
  segmentId: string;
  before?: Neighbor | null;
  after?: Neighbor | null;
}
export interface Neighbor {
  measurementRef: string;
  rawInput: TimingMetricUint64;
  ratio: TimingMetricDouble;
  distanceMs: TimingMetricUint64;
}
export interface Work {
  outcomes: OutcomeStatistics;
  operationCandidates: TimingMetricUint64;
  closedOperations: TimingMetricUint64;
  failedOperations: TimingMetricUint64;
  labelledCommandMs: TimingMetricUint64;
  fileChangeRecords: TimingMetricUint64;
  changedFiles: TimingMetricUint64;
  addedLines: TimingMetricUint64;
  removedLines: TimingMetricUint64;
  messageRecordCandidates: TimingMetricUint64;
  nonemptyVisibleContentRecords: TimingMetricUint64;
  unknownContentRecords: TimingMetricUint64;
  missingContentTimeRecords: TimingMetricUint64;
  userBoundaryRecords: TimingMetricUint64;
  injectedContextRecords: TimingMetricUint64;
  reasoningMessageRecords: TimingMetricUint64;
  compactionRecords: TimingMetricUint64;
  repositoryBaseline: Capability;
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
export interface Finding {
  code: string;
  kind: FindingKind;
  metricRefs: string[];
  evidenceRefs: string[];
}
export interface Coverage {
  facts: TimingMetricUint64;
  bytes: TimingMetricUint64;
  metadata: TimingMetricUint64;
  eventBlocks: TimingMetricUint64;
  scopedEvents: TimingMetricUint64;
  scopedMeasurements: TimingMetricUint64;
  boundaryCandidates: TimingMetricUint64;
  /**
   * @minItems 4
   * @maxItems 4
   */
  lifecycleCandidates: [TimingMetricUint64, TimingMetricUint64, TimingMetricUint64, TimingMetricUint64];
  /**
   * @minItems 4
   * @maxItems 4
   */
  linkedLifecycles: [TimingMetricUint64, TimingMetricUint64, TimingMetricUint64, TimingMetricUint64];
  conflictingLifecycles: TimingMetricUint64;
  missingIdentityLifecycles: TimingMetricUint64;
  contentCandidates: TimingMetricUint64;
  domainCount: TimingMetricUint64;
  missingWatermarks: TimingMetricUint64;
  generationMismatches: TimingMetricUint64;
  incompleteDomains: TimingMetricUint64;
  snapshotUnassignedTotal: TimingMetricUint64;
  threadUnassignedTotal: TimingMetricUint64;
  sourceStatus: string;
}
export interface Quality {
  partial: boolean;
  running: boolean;
  censored: boolean;
  reasonCodes: Basis[];
  factLimit: number;
  summaryLimitBytes: number;
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
export interface EvidenceIndex {
  repeatPages: RepeatPages;
  intervalPages: IntervalPages;
  collections: EvidenceCollection[];
  available: boolean;
  limit: number;
  snapshotId: string;
  refs: string[];
  method: string;
}
export interface RepeatPages {
  detail: Capability;
  candidateOperationCount: TimingMetricUint64;
  locatedOperationCount: TimingMetricUint64;
  pageCount: TimingMetricUint64;
  limitBytes: number;
  /**
   * @maxItems 200
   */
  entries: RepeatEvidenceEntry[];
}
export interface RepeatEvidenceEntry {
  later: RepeatProof;
  afterFailure?: RepeatProof | null;
  /**
   * @maxItems 599
   */
  successfulReads: RepeatProof[];
  repeatedReadTargetCount: number;
  laterDurationMs: TimingMetricUint64;
  recoverySpanMs: TimingMetricUint64;
}
export interface RepeatProof {
  operationAlias: string;
  /**
   * @minItems 1
   * @maxItems 16
   */
  pages:
    | [RepeatEvidencePage]
    | [RepeatEvidencePage, RepeatEvidencePage]
    | [RepeatEvidencePage, RepeatEvidencePage, RepeatEvidencePage]
    | [RepeatEvidencePage, RepeatEvidencePage, RepeatEvidencePage, RepeatEvidencePage]
    | [RepeatEvidencePage, RepeatEvidencePage, RepeatEvidencePage, RepeatEvidencePage, RepeatEvidencePage]
    | [
        RepeatEvidencePage,
        RepeatEvidencePage,
        RepeatEvidencePage,
        RepeatEvidencePage,
        RepeatEvidencePage,
        RepeatEvidencePage
      ]
    | [
        RepeatEvidencePage,
        RepeatEvidencePage,
        RepeatEvidencePage,
        RepeatEvidencePage,
        RepeatEvidencePage,
        RepeatEvidencePage,
        RepeatEvidencePage
      ]
    | [
        RepeatEvidencePage,
        RepeatEvidencePage,
        RepeatEvidencePage,
        RepeatEvidencePage,
        RepeatEvidencePage,
        RepeatEvidencePage,
        RepeatEvidencePage,
        RepeatEvidencePage
      ]
    | [
        RepeatEvidencePage,
        RepeatEvidencePage,
        RepeatEvidencePage,
        RepeatEvidencePage,
        RepeatEvidencePage,
        RepeatEvidencePage,
        RepeatEvidencePage,
        RepeatEvidencePage,
        RepeatEvidencePage
      ]
    | [
        RepeatEvidencePage,
        RepeatEvidencePage,
        RepeatEvidencePage,
        RepeatEvidencePage,
        RepeatEvidencePage,
        RepeatEvidencePage,
        RepeatEvidencePage,
        RepeatEvidencePage,
        RepeatEvidencePage,
        RepeatEvidencePage
      ]
    | [
        RepeatEvidencePage,
        RepeatEvidencePage,
        RepeatEvidencePage,
        RepeatEvidencePage,
        RepeatEvidencePage,
        RepeatEvidencePage,
        RepeatEvidencePage,
        RepeatEvidencePage,
        RepeatEvidencePage,
        RepeatEvidencePage,
        RepeatEvidencePage
      ]
    | [
        RepeatEvidencePage,
        RepeatEvidencePage,
        RepeatEvidencePage,
        RepeatEvidencePage,
        RepeatEvidencePage,
        RepeatEvidencePage,
        RepeatEvidencePage,
        RepeatEvidencePage,
        RepeatEvidencePage,
        RepeatEvidencePage,
        RepeatEvidencePage,
        RepeatEvidencePage
      ]
    | [
        RepeatEvidencePage,
        RepeatEvidencePage,
        RepeatEvidencePage,
        RepeatEvidencePage,
        RepeatEvidencePage,
        RepeatEvidencePage,
        RepeatEvidencePage,
        RepeatEvidencePage,
        RepeatEvidencePage,
        RepeatEvidencePage,
        RepeatEvidencePage,
        RepeatEvidencePage,
        RepeatEvidencePage
      ]
    | [
        RepeatEvidencePage,
        RepeatEvidencePage,
        RepeatEvidencePage,
        RepeatEvidencePage,
        RepeatEvidencePage,
        RepeatEvidencePage,
        RepeatEvidencePage,
        RepeatEvidencePage,
        RepeatEvidencePage,
        RepeatEvidencePage,
        RepeatEvidencePage,
        RepeatEvidencePage,
        RepeatEvidencePage,
        RepeatEvidencePage
      ]
    | [
        RepeatEvidencePage,
        RepeatEvidencePage,
        RepeatEvidencePage,
        RepeatEvidencePage,
        RepeatEvidencePage,
        RepeatEvidencePage,
        RepeatEvidencePage,
        RepeatEvidencePage,
        RepeatEvidencePage,
        RepeatEvidencePage,
        RepeatEvidencePage,
        RepeatEvidencePage,
        RepeatEvidencePage,
        RepeatEvidencePage,
        RepeatEvidencePage
      ]
    | [
        RepeatEvidencePage,
        RepeatEvidencePage,
        RepeatEvidencePage,
        RepeatEvidencePage,
        RepeatEvidencePage,
        RepeatEvidencePage,
        RepeatEvidencePage,
        RepeatEvidencePage,
        RepeatEvidencePage,
        RepeatEvidencePage,
        RepeatEvidencePage,
        RepeatEvidencePage,
        RepeatEvidencePage,
        RepeatEvidencePage,
        RepeatEvidencePage,
        RepeatEvidencePage
      ];
}
/**
 * Local source-record proofs. Sharing contains no repeat locators or aliases.
 */
export interface RepeatEvidencePage {
  cursor?: Cursor | null;
  limit: number;
  /**
   * @minItems 1
   * @maxItems 16
   */
  evidenceRefs:
    | [string]
    | [string, string]
    | [string, string, string]
    | [string, string, string, string]
    | [string, string, string, string, string]
    | [string, string, string, string, string, string]
    | [string, string, string, string, string, string, string]
    | [string, string, string, string, string, string, string, string]
    | [string, string, string, string, string, string, string, string, string]
    | [string, string, string, string, string, string, string, string, string, string]
    | [string, string, string, string, string, string, string, string, string, string, string]
    | [string, string, string, string, string, string, string, string, string, string, string, string]
    | [string, string, string, string, string, string, string, string, string, string, string, string, string]
    | [string, string, string, string, string, string, string, string, string, string, string, string, string, string]
    | [
        string,
        string,
        string,
        string,
        string,
        string,
        string,
        string,
        string,
        string,
        string,
        string,
        string,
        string,
        string
      ]
    | [
        string,
        string,
        string,
        string,
        string,
        string,
        string,
        string,
        string,
        string,
        string,
        string,
        string,
        string,
        string,
        string
      ];
}
export interface IntervalPages {
  detail: Capability;
  candidateIntervalCount: TimingMetricUint64;
  locatedIntervalCount: TimingMetricUint64;
  missingEventRefCount: TimingMetricUint64;
  /**
   * Sum of page locators over intervals, after merging each interval's same-page refs.
   */
  pageCount: TimingMetricUint64;
  limitBytes: number;
  /**
   * @maxItems 200
   */
  entries: IntervalPage[];
}
/**
 * Local navigation only: the sharing DTO has no evidence index or cursor type.
 */
export interface IntervalPage {
  intervalAlias: string;
  /**
   * @maxItems 3
   */
  pages: [] | [FragmentPage] | [FragmentPage, FragmentPage] | [FragmentPage, FragmentPage, FragmentPage];
}
export interface FragmentPage {
  cursor?: Cursor | null;
  limit: number;
  /**
   * @maxItems 3
   */
  evidenceRefs: [] | [string] | [string, string] | [string, string, string];
}
export interface EvidenceCollection {
  reference: string;
  kind: CollectionKind;
  snapshotId: string;
  scope: LocalScope;
  count: TimingMetricUint64;
  method: string;
}
export interface ShareResponse {
  uses: UseTotals;
  basisCollections: ShareCollection[];
  outputVersion: number;
  action: SummaryAction;
  methodVersion: string;
  profile: ShareProfile;
  privacy: Privacy;
  scope: ShareScope;
  capabilities: Capabilities;
  relativeAnchors: Anchors;
  time: Time;
  context: Context;
  work: Work;
  findings: Finding[];
  coverage: Coverage;
  quality: Quality;
  freshness: ShareFreshness;
}
export interface ShareCollection {
  reference: string;
  kind: CollectionKind;
  taskAlias: string;
  turnAlias: string;
  count: TimingMetricUint64;
  method: string;
}
export interface ShareScope {
  taskAlias: string;
  turnAlias: string;
  wholeTurn: boolean;
}
export interface ShareFreshness {
  status: string;
  errorCode?: string | null;
}
export interface EvidenceResponse {
  collection: EventPageKind;
  outputVersion: number;
  action: EvidenceAction;
  methodVersion: string;
  profile: LocalProfile;
  snapshotId: string;
  scope: LocalScope;
  total: TimingMetricUint64;
  rows: EvidenceRow[];
  nextCursor?: Cursor | null;
}
export interface EvidenceRow {
  reference: string;
  recordKind: string;
  timestampMs?: number | null;
  phase?: string | null;
  presence?: string | null;
  origin?: string | null;
  durationMs?: number | null;
  firstTokenMs?: number | null;
  gapCodes: string[];
}
export interface UseObjectsResponse {
  outputVersion: number;
  action: EvidenceAction;
  collection: UseObjectPageKind;
  methodVersion: string;
  profile: LocalProfile;
  snapshotId: string;
  scope: LocalScope;
  totals: UseTotals;
  total: TimingMetricUint64;
  /**
   * @maxItems 200
   */
  rows: UseObject[];
  nextCursor?: Cursor | null;
}
export interface UseRecordsResponse {
  outputVersion: number;
  action: EvidenceAction;
  collection: UseRecordPageKind;
  methodVersion: string;
  profile: LocalProfile;
  snapshotId: string;
  scope: LocalScope;
  objectRef?: string | null;
  totals: UseTotals;
  /**
   * Full selected record count; the whole-turn totals do not shrink with this object page.
   */
  total: TimingMetricUint64;
  /**
   * @maxItems 200
   */
  rows: UseRecord[];
  nextCursor?: Cursor | null;
}
export interface UseRecord {
  reference: string;
  objectRef?: string | null;
  kind?: UseKind | null;
  state: UseState;
  outcome: UseOutcome;
  timestampMs?: number | null;
  timeBasis: UseTimeBasis;
  nativeDurationMs?: number | null;
  tool?: string | null;
  exitCode?: number | null;
  identityKnown: boolean;
  replayOf?: string | null;
  targetConflict: boolean;
  gapCodes: string[];
}
export interface CapabilitiesResponse {
  outputVersion: number;
  action: CapabilitiesAction;
  methodVersion: string;
  profile: PrivacyProfile;
  capabilities: Capabilities;
}
