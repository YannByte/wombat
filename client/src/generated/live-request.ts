/* Generated from Rust. Run pnpm contracts:generate. */

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
export type Group = "day" | "week" | "month";
export type Sort = "tokens" | "cost" | "recent" | "time";
export type Presentation = "distribution" | "details" | "projects" | "models";
export type Mode = "auto" | "fresh" | "cached";

export interface Request {
  query: Request1;
  mode?: Mode & string;
  verify?: boolean;
  /**
   * One bounded wait owned by this caller; cancelling it leaves shared work running.
   */
  waitMs?: number | null;
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
    | [Request1]
    | [Request1, Request1]
    | [Request1, Request1, Request1]
    | [Request1, Request1, Request1, Request1]
    | [Request1, Request1, Request1, Request1, Request1]
    | [Request1, Request1, Request1, Request1, Request1, Request1]
    | [Request1, Request1, Request1, Request1, Request1, Request1, Request1]
    | [Request1, Request1, Request1, Request1, Request1, Request1, Request1, Request1];
}
export interface Request1 {
  comparison?: ComparisonRequest | null;
  action: Action;
  snapshotId?: string | null;
  roots?: string[] | null;
  scope?: Scope;
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
