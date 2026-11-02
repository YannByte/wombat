/* Generated from Rust. Run pnpm contracts:generate. */

export type Request =
  | {
      projectRoot: string;
      paths?: string[];
      keepPaths?: string[];
      productTemporary?: boolean;
      codex?: boolean;
      action: "inspect";
    }
  | {
      baseline: Response;
      /**
       * @maxItems 100
       */
      selection: string[];
      action: "apply";
    }
  | {
      baseline: Response;
      selection?: string[];
      action: "recheck";
    };
export type Kind =
  | "build_output"
  | "selected_temporary"
  | "product_temporary"
  | "codex_desktop_cache"
  | "codex_update_stage"
  | "codex_marketplace_stage"
  | "codex_daemon_release"
  | "codex_runtime"
  | "codex_crash_report";
export type Assessment = "candidate" | "review" | "protected" | "unknown" | "absent";
export type Regeneration =
  | "cargo_cache"
  | "type_script_output"
  | "codex_cache"
  | "superseded_package"
  | "discarded_stage"
  | "diagnostic"
  | "unknown";
export type UseState = "no_observed_open_files" | "in_use" | "unknown";
export type Reason =
  | "declared_build_output"
  | "cache_tag"
  | "task_ownership_required"
  | "product_stage"
  | "user_kept"
  | "user_asset"
  | "tracked_content"
  | "untracked_content"
  | "nested_repository"
  | "symlink"
  | "special_file"
  | "shared_output"
  | "no_repository"
  | "git_unavailable"
  | "regeneration_unknown"
  | "in_use"
  | "use_unknown"
  | "read_unavailable"
  | "scan_limit"
  | "changed_during_scan"
  | "not_found"
  | "owner_changed"
  | "referenced_product_data"
  | "known_codex_cache"
  | "superseded_version"
  | "active_version"
  | "pending_update"
  | "recent"
  | "incomplete_runtime"
  | "active_runtime"
  | "process_unknown"
  | "installation_busy"
  | "installation_unknown"
  | "unsupported_platform"
  | "conditions_changed"
  | "removal_failed";
export type RecheckState = "unchanged" | "changed" | "gone" | "unavailable";
export type Removal = "removed" | "already_absent" | "skipped" | "failed";

export interface Response {
  outputVersion: number;
  policyVersion: number;
  action: string;
  projectIdentity: string;
  baselineId: string;
  checkedAt: string;
  scope: Scope;
  items: Item[];
  summary: Summary;
  partial: boolean;
}
export interface Scope {
  projectRoot: string;
  paths: string[];
  keepPaths: string[];
  productTemporary: boolean;
  codex: boolean;
}
export interface Item {
  id: string;
  path: string;
  owner?: string | null;
  kind: Kind;
  assessment: Assessment;
  regeneration: Regeneration;
  regenerationVersion?: string | null;
  useState: UseState;
  observation?: Observation | null;
  reasons: Reason[];
  recheck?: RecheckState | null;
  codexEvidence?: CodexEvidence | null;
  removal?: Removal | null;
}
export interface Observation {
  physicalIdentity: string;
  fingerprint: string;
  logicalBytes: number;
  allocatedBytes?: number | null;
  files: number;
  complete: boolean;
}
export interface CodexEvidence {
  parentIdentity: string;
  installLockIdentity?: string | null;
  minimumAgeDays?: number | null;
  currentRelease?: string | null;
  pendingRelease?: string | null;
  installedBuild?: number | null;
  stagedBuild?: number | null;
}
export interface Summary {
  candidates: number;
  review: number;
  protected: number;
  unknown: number;
  absent: number;
  /**
   * Deduplicated observed logical file bytes, never a reclaim estimate.
   */
  observedLogicalBytes: number;
  complete: boolean;
}
