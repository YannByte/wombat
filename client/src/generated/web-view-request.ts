/* Generated from Rust. Run pnpm contracts:generate. */

export type Page = "usage" | "threads" | "instructions" | "extensions" | "optimize";
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
export type Action2 = "list" | "detail" | "evidence" | "related_scopes" | "capabilities";
export type Kind = "rule" | "skill" | "mcp" | "hook";
export type Observation = "used" | "loaded_only" | "unknown";
export type Sort2 = "tokens" | "activity" | "size" | "name" | "content_tokens" | "characters" | "recent";
export type Action3 =
  "list" | "detail" | "keep" | "not_applicable" | "redisplay" | "recheck" | "capabilities" | "checks" | "activity";
export type DecisionReason = "necessary" | "object_changed" | "incorrect_evidence";
export type Group2 = "pending" | "history";
export type Category = "repair" | "trim" | "organize" | "space";

export interface Request {
  page: Page;
  usage?: Request1 | null;
  configuration?: Request2 | null;
  optimization?: Request3 | null;
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
export interface Request2 {
  action?: Action2 & string;
  roots?: string[] | null;
  projectRoots?: string[] | null;
  snapshotId?: string | null;
  readView?: string | null;
  /**
   * Check current configuration against committed usage without scheduling collection.
   */
  cachedUsage?: boolean;
  scope?: Scope2;
  kind?: Kind | null;
  kinds?: Kind[] | null;
  observation?: Observation | null;
  search?: string | null;
  sort?: Sort2 & string;
  itemId?: string | null;
  offset?: number | null;
  limit?: number | null;
}
export interface Scope2 {
  allTime?: boolean | null;
  since?: string | null;
  until?: string | null;
  timezone?: string | null;
  project?: string | null;
  agentKind?: string | null;
  sourceInstanceId?: string | null;
  threadId?: string | null;
}
export interface Request3 {
  action?: Action3 & string;
  roots?: string[] | null;
  projectRoots?: string[] | null;
  readView?: string | null;
  decisionRevision?: string | null;
  project?: string | null;
  sourceInstanceId?: string | null;
  suggestionId?: string | null;
  itemId?: string | null;
  decisionReason?: DecisionReason | null;
  group?: Group2 & string;
  category?: Category | null;
  offset?: number | null;
  limit?: number | null;
  ruleOverrides?: RuleOverrides | null;
  activity?: ActivitySelection | null;
}
export interface RuleOverrides {
  agentsBytes?: number | null;
  descriptionCharacters?: number | null;
}
/**
 * Fixed turn analysis, separate from configuration identities and durable handling decisions.
 */
export interface ActivitySelection {
  snapshotId: string;
  threadId: string;
  turnId: string;
}
