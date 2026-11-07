/* Generated from Rust. Run pnpm contracts:generate. */

export type Action = ("activity" | "locate" | "read" | "search") | "records" | "resolve" | "source_search";
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
export type ProjectLoadState = "pending" | "loading" | "ready";

/**
 * A bounded original-text read and an independent continuation for message locations.
 */
export interface HistoryRead {
  outputVersion: number;
  result: Response;
  nextLocations?: Request | null;
  /**
   * Original read targets deferred by the response budget.
   */
  retryRead?: Request | null;
}
export interface Response {
  record?: ResolvedRecord | null;
  outputVersion: 7;
  action: Action;
  snapshotId: string;
  selection: Selection;
  /**
   * Explicit roots used for this query, for a search-to-read handoff.
   */
  roots?: string[];
  items: Activity[];
  fragments: Fragment[];
  neighbors?: Neighbor[];
  sourceHits?: SourceHit[];
  sourceFiles?: SourceSearchFile[];
  sourceNeighbors?: SourceNeighbor[];
  nextSearch?: Request | null;
  readIssues?: ReadIssue[];
  sourceScope?: string | null;
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
  /**
   * One canonical operation inside an explicitly selected thread and turn.
   */
  operationId?: string | null;
  /**
   * Read/search selected tool results on demand; bodies are never indexed.
   */
  includeToolOutput?: boolean;
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
  /**
   * The literal at the returned read cursor.
   */
  matchedTerm?: string | null;
  evidence: Evidence;
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
export interface Fragment {
  /**
   * Zero-based request target indices served by this occurrence.
   */
  targetIndices: number[];
  evidence: Evidence;
  message: Activity;
  text: string;
  matchRange?: MatchRange | null;
  nextCursor?: string | null;
  locations: ContentLocation[];
  context: Activity[];
  visibleContext?: VisibleContext | null;
  partial: boolean;
  /**
   * Other read targets whose returned text is exactly equal to this text.
   */
  alsoAt?: FragmentOccurrence[];
}
export interface MatchRange {
  /**
   * Start and exclusive end in UTF-8 bytes within the returned text.
   */
  start: number;
  end: number;
}
export interface ContentLocation {
  file: string;
  byteStart: number;
  byteEnd: number;
  part: number;
}
export interface VisibleContext {
  /**
   * Event IDs into the response's shared neighbor bodies, in source order.
   */
  before: string[];
  after: string[];
  partial: boolean;
}
export interface FragmentOccurrence {
  targetIndices: number[];
  evidence: Evidence;
  message: Activity;
  matchRange?: MatchRange | null;
  nextCursor?: string | null;
  locations: ContentLocation[];
  context: Activity[];
  visibleContext?: VisibleContext | null;
  partial: boolean;
}
export interface Neighbor {
  message: Activity;
  text: string;
  locations: ContentLocation[];
  partial: boolean;
}
export interface SourceHit {
  /**
   * Zero-based index into this response's source files.
   */
  fileIndex: number;
  timestamp?: string | null;
  origin: MessageOrigin;
  byteStart: number;
  byteEnd: number;
  /**
   * Digest of the complete original JSONL record.
   */
  sha256: string;
  matchedTerm: string;
  matchRange: MatchRange;
  text: string;
  context?: SourceContext | null;
}
export interface SourceContext {
  /**
   * Indices into the response's shared source neighbors, in file order.
   */
  before: number[];
  after: number[];
  partial: boolean;
}
export interface SourceSearchFile {
  sourceInstanceId: string;
  threadId: string;
  upstreamId: string;
  project?: string | null;
  file: string;
}
export interface SourceNeighbor {
  fileIndex: number;
  timestamp?: string | null;
  origin: MessageOrigin;
  byteStart: number;
  byteEnd: number;
  sha256: string;
  text: string;
  partial: boolean;
}
export interface Request {
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
  /**
   * Search only. Match any of 1–16 literal terms. Supply this or query.
   */
  terms?: string[] | null;
  targets?: ReadTarget[];
  maxScanBytes?: number | null;
  /**
   * Use the extended foreground scan budget for search or location.
   */
  complete?: boolean;
  /**
   * Read this many visible messages on either side of each read target or direct source hit.
   */
  contextMessages?: number | null;
  /**
   * Maximum UTF-8 bytes per direct source hit.
   */
  excerptBytes?: number | null;
  /**
   * Pin an evidence scan to its first observed source scope.
   */
  fixedScope?: boolean;
  /**
   * Use the scope returned by the initial scan when reading export evidence.
   */
  sourceScope?: string | null;
  anchor?: Evidence | null;
  radius?: number | null;
}
export interface ReadTarget {
  evidence: Evidence;
  cursor?: string | null;
  maxBytes?: number | null;
  before?: number;
  after?: number;
}
export interface ReadIssue {
  targetIndices: number[];
  eventId: string;
  code: string;
  /**
   * Retry these original targets in a separate batch.
   */
  deferred: boolean;
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
export interface ProjectLoad {
  project?: string | null;
  state: ProjectLoadState;
}
export interface ContextWindow {
  anchor: Evidence;
  radius: number;
}
