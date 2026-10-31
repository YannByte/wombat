/* Generated from Rust. Run pnpm contracts:generate. */

export type Action = "list" | "detail" | "keep" | "not_applicable" | "recheck" | "capabilities" | "checks" | "activity";
export type DecisionReason = "necessary" | "object_changed" | "incorrect_evidence";
export type Category = "repair" | "trim";

export interface Request {
  action?: Action & string;
  roots?: string[] | null;
  projectRoots?: string[] | null;
  readView?: string | null;
  baselineId?: string | null;
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
