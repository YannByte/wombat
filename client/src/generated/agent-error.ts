/* Generated from Rust. Run pnpm contracts:generate. */

export type Recovery = "read_schema" | "narrow_query" | "retry_same_scope" | "check_setup" | "inspect_state" | "none";

export interface ErrorOutput {
  outputVersion: number;
  error: Error;
}
export interface Error {
  code: string;
  message: string;
  recovery: Recovery;
}
