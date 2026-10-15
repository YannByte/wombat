/* Generated from Rust. Run pnpm contracts:generate. */

export type Action = "status" | "start" | "pause" | "resume" | "stop" | "enable" | "disable";

export interface Response {
  outputVersion: number;
  action: Action;
  running: boolean;
  resident: boolean;
  paused: boolean;
  stopping: boolean;
  instanceId?: string | null;
  runtime?: RuntimeIdentity | null;
  autoStart: boolean;
  controlsAvailable: boolean;
  preparing?: boolean | null;
  ready?: boolean | null;
  checkedAt?: string | null;
  revision?: string | null;
  errorCode?: string | null;
  watcher: string;
  /**
   * Encoded parser-state cache budget; this is not a hard process RSS limit.
   */
  parserCacheBytes: number;
  parserCacheBudgetBytes: number;
}
export interface RuntimeIdentity {
  binary: string;
  bytes: number;
  modifiedNs: string;
  contextKey: string;
}
