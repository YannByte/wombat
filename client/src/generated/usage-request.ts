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

export interface Request {
  comparison?: ComparisonRequest | null;
  action: Action;
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
