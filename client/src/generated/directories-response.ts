/* Generated from Rust. Run pnpm contracts:generate. */

export type Action = "list" | "authorize" | "revoke";
export type Purpose = "source" | "project";

export interface Response {
  outputVersion: number;
  action: Action;
  grants: Grant[];
}
export interface Grant {
  id: string;
  purpose: Purpose;
  path: string;
  directoryIdentity: string;
  authorizedAt: string;
  status: string;
}
