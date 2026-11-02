/* Generated from Rust. Run pnpm contracts:generate. */

export type Request =
  | {
      action: "list";
    }
  | {
      plan: Plan;
      action: "upsert";
    }
  | {
      id: string;
      action: "remove";
    }
  | {
      roots?: string[];
      ids: string[];
      action: "check";
    }
  | {
      notificationId: string;
      action: "acknowledge";
    };
export type Period = "day" | "week" | "month";

export interface Plan {
  id: string;
  enabled: boolean;
  period: Period;
  /**
   * Identity filters and timezone only; date, undated, thread and turn filters are rejected.
   */
  scope?: Scope;
  /**
   * Token budget, not an account allowance or money allocation to tools.
   */
  tokenLimit?: number | null;
  /**
   * Fraction in (0,1]; 0.8 by default.
   */
  warningRatio?: number | null;
  review: boolean;
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
