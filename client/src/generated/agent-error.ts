/* Generated from Rust. Run pnpm contracts:generate. */

export type Recovery = "read_schema" | "narrow_query" | "retry_same_scope" | "check_setup" | "inspect_state" | "none";

export interface ErrorOutput {
  outputVersion: number;
  error: Error;
}
export interface Error {
  details?: InputErrorDetails | null;
  code: string;
  message: string;
  recovery: Recovery;
}
export interface InputErrorDetails {
  /**
   * @maxItems 4
   */
  fields:
    | []
    | [InputErrorField]
    | [InputErrorField, InputErrorField]
    | [InputErrorField, InputErrorField, InputErrorField]
    | [InputErrorField, InputErrorField, InputErrorField, InputErrorField];
  schemaCommand: string;
}
export interface InputErrorField {
  path: string;
  reason: string;
}
