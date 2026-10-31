/* Generated from Rust. Run pnpm contracts:generate. */

export type AllowanceIntervalStatus =
  | "compatible"
  | "first"
  | "observation_gap"
  | "account_changed"
  | "window_changed"
  | "reset_changed"
  | "expired"
  | "time_order"
  | "duration_unknown";
export type Action = "read" | "refresh" | "history";

export interface Response {
  history?: AllowanceHistory | null;
  historyErrorCode?: string | null;
  outputVersion: number;
  action: Action;
  nativeVersion?: string | null;
  account: Section;
  allowance: Section;
  activity: Section;
  identity?: Identity | null;
  windows: Window[];
  buckets: Bucket[];
  resetCredits?: ResetCredits | null;
  ordinaryUsageAllowed?: boolean | null;
  modelRestriction?: ModelRestriction | null;
  summary?: Summary | null;
}
export interface AllowanceHistory {
  methodVersion: number;
  totalObservations: number;
  observations: AllowanceObservation[];
}
export interface AllowanceObservation {
  nativeVersion?: string | null;
  id: string;
  observedAt: string;
  accountId?: string | null;
  status: string;
  windows: AllowanceWindowObservation[];
}
export interface AllowanceWindowObservation {
  id: string;
  bucketId: string;
  usedPercent: number;
  durationMinutes?: number | null;
  resetsAt?: string | null;
  status: string;
  deltaPercentagePoints?: number | null;
  intervalStatus: AllowanceIntervalStatus;
}
export interface Section {
  status: string;
  checkedAt?: string | null;
  errorCode?: string | null;
}
export interface Identity {
  id: string;
  kind: string;
  maskedEmail?: string | null;
  plan?: string | null;
}
export interface Window {
  id: string;
  bucketId: string;
  bucketName?: string | null;
  /**
   * Native normalModelSlug: presentation metadata, not quota applicability.
   */
  model?: string | null;
  usedPercent: number;
  durationMinutes?: number | null;
  resetsAt?: string | null;
  status: string;
}
export interface Bucket {
  id: string;
  name?: string | null;
  /**
   * Native normalModelSlug: presentation metadata, not quota applicability.
   */
  model?: string | null;
  credits?: Credits | null;
  individualLimit?: SpendLimit | null;
  spendControlReached?: boolean | null;
  rateLimitReachedType?: string | null;
  status: string;
}
export interface Credits {
  /**
   * Native decimal text; no currency or token conversion is inferred.
   */
  balance?: string | null;
  hasCredits?: boolean | null;
  unlimited?: boolean | null;
}
export interface SpendLimit {
  limit?: string | null;
  used?: string | null;
  remainingPercent?: number | null;
  resetsAt?: string | null;
  status: string;
}
export interface ResetCredits {
  availableCount?: number | null;
  /**
   * None means details were not provided; an empty list is an observed empty list.
   */
  credits?: ResetCredit[] | null;
  detailsTruncated: boolean;
}
export interface ResetCredit {
  id: string;
  title?: string | null;
  description?: string | null;
  grantedAt?: string | null;
  expiresAt?: string | null;
  resetType: string;
  status: string;
}
export interface ModelRestriction {
  /**
   * Explicit blocked_model_slug from an identity-bound native usage-limit notice.
   */
  model: string;
  resetsAt?: string | null;
}
export interface Summary {
  lifetimeTokens?: number | null;
  currentStreakDays?: number | null;
  longestStreakDays?: number | null;
  peakDailyTokens?: number | null;
  longestRunningTurnSeconds?: number | null;
}
