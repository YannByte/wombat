/* Generated from Rust. Run pnpm contracts:generate. */

export type Action = "activity" | "locate" | "read" | "search" | "changes";
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

export interface Response {
  outputVersion: number;
  action: Action;
  snapshotId: string;
  baselineSnapshotId?: string | null;
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
  evidence: Evidence;
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
