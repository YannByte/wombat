/* Generated from Rust. Run pnpm contracts:generate. */

export type Phase = "planned" | "running" | "incomplete" | "complete" | "applying" | "applied" | "undoing" | "undone";
export type Check =
  | {
      id: string;
      command: string;
      kind: "command";
    }
  | {
      id: string;
      path: string;
      equals: string;
      kind: "file";
    };
export type Recommendation = "review_composed" | "keep_baseline";

export interface Response {
  outputVersion: number;
  trialId: string;
  directory: string;
  phase: Phase;
  plan: Plan;
  original: string;
  outcome?: Outcome | null;
  recommendation?: Recommendation | null;
  revision?: string | null;
  loaded: boolean;
  failure?: string | null;
  failedStage?: string | null;
}
export interface Plan {
  project: string;
  task: string;
  target: string;
  originalSha256: string;
  inputs: Input[];
  checks: Check[];
  includeGlobalInstructions: boolean;
  globalInstructionSources: Input[];
  executions: number;
  editorialSessions: number;
  turnTimeoutSeconds: number;
}
export interface Input {
  path: string;
  sha256: string;
}
export interface Outcome {
  model: string;
  stages: Stage[];
  candidate: string;
  rationale: string;
  report: string;
}
export interface Stage {
  stage: string;
  checks: Observation[];
}
export interface Observation {
  id: string;
  passed: boolean;
}
