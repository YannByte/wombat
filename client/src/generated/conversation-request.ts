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
