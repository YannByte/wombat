/* Generated from Rust. Run pnpm contracts:generate. */

export type Request =
  | {
      threadId: string;
      turnId: string;
      roots?: string[];
      scope?: Scope | null;
      mode?: Mode & string;
      privacyProfile?: PrivacyProfile & string;
      action: "summary";
    }
  | {
      threadId: string;
      turnId: string;
      roots?: string[];
      scope?: Scope | null;
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
export type Mode = "auto" | "fresh" | "cached";
export type PrivacyProfile = "local" | "share-v1";
export type EvidenceSet = "turn_events" | "use_objects" | "use_records";

export interface Scope {
  sourceInstanceId?: string | null;
  agentKind?: string | null;
}
export interface Cursor {
  token: string;
}
