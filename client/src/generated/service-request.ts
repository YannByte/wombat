/* Generated from Rust. Run pnpm contracts:generate. */

export type Action = "status" | "start" | "pause" | "resume" | "stop" | "enable" | "disable";

export interface Request {
  action: Action;
}
