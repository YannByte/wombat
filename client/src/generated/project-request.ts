/* Generated from Rust. Run pnpm contracts:generate. */

export interface Request {
  projectRoot: string;
  roots?: string[];
  since: string;
  until: string;
  timezone: string;
  sourceInstanceId?: string | null;
  ruleOverrides?: RuleOverrides | null;
}
export interface RuleOverrides {
  agentsBytes?: number | null;
  descriptionCharacters?: number | null;
}
