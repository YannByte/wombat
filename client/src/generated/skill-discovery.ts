/* Generated from Rust. Run pnpm contracts:generate. */

export type DiscoveryStatus = "available" | "ambiguous" | "disabled" | "missing" | "unavailable" | "selection_changed";

export interface Discovery {
  /**
   * @maxItems 32
   */
  parseErrorPaths?: string[];
  status: DiscoveryStatus;
  instances: Instance[];
  errorCode?: string | null;
}
export interface Instance {
  name: string;
  path: string;
  enabled: boolean;
}
