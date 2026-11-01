/* Generated from Rust. Run pnpm contracts:generate. */

/**
 * Task-oriented local command discovery. No runtime or source preparation.
 */
export interface Help {
  outputVersion: number;
  command: string;
  commands: string[];
  help: string;
  examples: HelpExample[];
  next: string[];
}
export interface HelpExample {
  task: string;
  command: string;
}
