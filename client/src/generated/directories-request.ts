/* Generated from Rust. Run pnpm contracts:generate. */

export type Action = "list" | "authorize" | "revoke";
export type Purpose = "source" | "project";

export interface Request {
  action?: Action & string;
  purpose?: Purpose | null;
  path?: string | null;
  grantId?: string | null;
}
