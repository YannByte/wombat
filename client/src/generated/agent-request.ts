/* Generated from Rust. Run pnpm contracts:generate. */

export type Request =
  | {
      method: "service";
      params: Request1;
    }
  | {
      method: "conversation";
      params: Request2;
    }
  | {
      method: "usage";
      params: Request3;
    }
  | {
      method: "snapshot";
      params: Request4;
    }
  | {
      method: "config";
      params: Request5;
    }
  | {
      method: "optimize";
      params: Request6;
    }
  | {
      method: "timing";
      params: Request7;
    }
  | {
      method: "setup";
      params: Request8;
    }
  | {
      method: "account";
      params: Request9;
    }
  | {
      method: "directories";
      params: Request10;
    }
  | {
      method: "preferences";
      params: Request11;
    }
  | {
      method: "prices";
      params: Request12;
    }
  | {
      method: "collection";
      params: Request13;
    }
  | {
      method: "handoff";
      params: Request14;
    }
  | {
      method: "monitor";
      params: Request15;
    };
export type Action = "status" | "start" | "pause" | "resume" | "stop" | "enable" | "disable";
export type Action2 = "activity" | "locate" | "read" | "search" | "changes";
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
export type Action3 =
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
export type Action4 = "list" | "detail" | "evidence" | "related_scopes" | "capabilities";
export type Kind = "rule" | "skill" | "mcp" | "hook";
export type Observation = "used" | "loaded_only" | "unknown";
export type Sort2 = "tokens" | "activity" | "size" | "name" | "content_tokens" | "characters" | "recent";
export type Action5 =
  "list" | "detail" | "keep" | "not_applicable" | "redisplay" | "recheck" | "capabilities" | "checks" | "activity";
export type DecisionReason = "necessary" | "object_changed" | "incorrect_evidence";
export type Group2 = "pending" | "history";
export type Category = "repair" | "trim" | "organize" | "space";
export type Request7 =
  | {
      threadId: string;
      turnId: string;
      snapshotId?: string | null;
      roots?: string[];
      scope?: Scope3 | null;
      mode?: Mode2 & string;
      privacyProfile?: PrivacyProfile & string;
      action: "summary";
    }
  | {
      threadId: string;
      turnId: string;
      snapshotId: string;
      roots?: string[];
      scope?: Scope3 | null;
      cursor?: Cursor | null;
      limit?: number;
      collection?: EvidenceSet & string;
      objectRef?: string | null;
      privacyProfile?: PrivacyProfile & string;
      action: "evidence";
    }
  | {
      privacyProfile?: PrivacyProfile & string;
      action: "capabilities";
    };
export type Mode2 = "auto" | "fresh" | "cached";
export type PrivacyProfile = "local" | "share-v1";
export type EvidenceSet = "turn_events" | "use_objects" | "use_records";
export type Action6 = "read" | "refresh" | "history";
export type Action7 = "list" | "choose" | "confirm" | "authorize" | "revoke";
export type Purpose = "source" | "project";
export type Action8 = "get" | "set";
export type Language = "zh" | "en";
export type Action9 = "status" | "update" | "auto_update";
export type Action10 = "status" | "events" | "configure" | "pause" | "resume";
export type Mode3 = "logs" | "hooks";
export type Action11 = "preview" | "send";
export type Request15 =
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
      snapshotId: string;
      ids: string[];
      action: "check";
    }
  | {
      notificationId: string;
      action: "acknowledge";
    };
export type Period = "day" | "week" | "month";

export interface Request1 {
  action: Action;
}
export interface Request2 {
  action: Action2;
  snapshotId: string;
  roots?: string[];
  selection?: Selection;
  limit?: number | null;
  cursor?: string | null;
  query?: string | null;
  baselineSnapshotId?: string | null;
  targets?: ReadTarget[];
  maxScanBytes?: number | null;
  anchor?: Evidence | null;
  radius?: number | null;
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
export interface Request3 {
  query: Request4;
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
    | [Request4]
    | [Request4, Request4]
    | [Request4, Request4, Request4]
    | [Request4, Request4, Request4, Request4]
    | [Request4, Request4, Request4, Request4, Request4]
    | [Request4, Request4, Request4, Request4, Request4, Request4]
    | [Request4, Request4, Request4, Request4, Request4, Request4, Request4]
    | [Request4, Request4, Request4, Request4, Request4, Request4, Request4, Request4];
}
export interface Request4 {
  comparison?: ComparisonRequest | null;
  action: Action3;
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
export interface Request5 {
  action?: Action4 & string;
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
export interface Request6 {
  action?: Action5 & string;
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
export interface Scope3 {
  sourceInstanceId?: string | null;
  agentKind?: string | null;
}
export interface Cursor {
  token: string;
}
export interface Request8 {
  project?: string | null;
  roots?: string[] | null;
}
export interface Request9 {
  action?: Action6 & string;
}
export interface Request10 {
  action?: Action7 & string;
  purpose?: Purpose | null;
  path?: string | null;
  choiceToken?: string | null;
  grantId?: string | null;
}
export interface Request11 {
  action: Action8;
  language?: Language | null;
}
export interface Request12 {
  action: Action9;
}
export interface Request13 {
  action?: Action10 & string;
  mode?: Mode3 | null;
  roots?: string[] | null;
  project?: string | null;
  sourceInstanceId?: string | null;
  after?: number | null;
  limit?: number | null;
}
export interface Request14 {
  action?: Action11 & string;
  roots?: string[] | null;
  projectRoots?: string[] | null;
  project?: string | null;
  sourceInstanceId?: string | null;
  readView?: string | null;
  decisionRevision?: string | null;
  ruleOverrides?: RuleOverrides | null;
  suggestionIds?: string[] | null;
  selectionVersion?: string | null;
  language?: string | null;
  withoutSkill?: boolean | null;
  skillSelections?: Selection2[] | null;
}
export interface Selection2 {
  projectId: string;
  path: string;
}
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
