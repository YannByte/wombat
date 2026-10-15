import type { UsageResult } from "@wombat/client";
import type {
  ConfigResult,
  OptimizeResult,
  AccountResult,
  DirectoriesResult,
  HandoffResult,
  SetupResult,
  CollectionResult,
} from "@wombat/client";
export const configExitCode = (r: ConfigResult) =>
  r.coverage.status === "partial" ? 2 : 0;
export const optimizeExitCode = (r: OptimizeResult) =>
  r.resultStatus === "partial" ? 2 : 0;
export const accountExitCode = (r: AccountResult) =>
  [r.account, r.allowance, r.activity].some((s) =>
    ["unavailable", "partial", "stale"].includes(s.status),
  )
    ? 2
    : 0;
export const directoriesExitCode = (r: DirectoriesResult) =>
  r.grants.some((g) => g.status !== "authorized") ? 2 : 0;
export const handoffExitCode = (r: HandoffResult) =>
  r.deliveries.some((d) => d.status !== "accepted") ? 2 : 0;
export const setupExitCode = (r: SetupResult) =>
  r.errorCodes.length ||
  r.discovery.status !== "available" ||
  r.hooks?.status === "partial"
    ? 2
    : 0;
export const collectionExitCode = (r: CollectionResult) =>
  r.gaps || r.buffered || r.state === "paused" ? 2 : 0;

export function resultExitCode(result: UsageResult): number {
  return result.quality.status === "partial" ||
    result.inspection?.partial ||
    (result.freshness &&
      !["current", "fixed"].includes(result.freshness.status))
    ? 2
    : 0;
}
