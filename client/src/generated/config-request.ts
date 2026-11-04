/* Generated from Rust. Run pnpm contracts:generate. */

export type Action = "list" | "detail" | "evidence" | "explain" | "related_scopes" | "capabilities";
export type Kind = "rule" | "skill" | "mcp" | "hook";
export type Observation = "used" | "loaded_only" | "unknown";
export type Sort =
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

export interface Request {
  /**
   * Observe native Skill metadata and order context footprint alongside use frequency.
   */
  review?: boolean;
  action?: Action & string;
  roots?: string[] | null;
  projectRoots?: string[] | null;
  /**
   * Check current configuration against committed usage without scheduling collection.
   */
  cachedUsage?: boolean;
  scope?: Scope;
  kind?: Kind | null;
  kinds?: Kind[] | null;
  observation?: Observation | null;
  search?: string | null;
  sort?: Sort & string;
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
