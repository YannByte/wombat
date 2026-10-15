import assert from "node:assert/strict";
import { readFileSync, statSync } from "node:fs";
import path from "node:path";
import { parseArgs } from "node:util";
import { fileURLToPath } from "node:url";

export const MIN_QUERY_SAMPLES = 20;

/** Measure complete requests; reported percentiles are never accepted as input. */
export function queryLatencyGate(groups: Record<string, readonly number[]>) {
  assert(Object.keys(groups).length > 0, "No query timing groups");
  const results = Object.fromEntries(
    Object.entries(groups).map(([name, samples]) => {
      assert(
        samples.length >= MIN_QUERY_SAMPLES,
        `${name}: need at least ${MIN_QUERY_SAMPLES} query samples`,
      );
      assert(
        samples.every((ms) => Number.isFinite(ms) && ms > 0),
        `${name}: invalid query timing`,
      );
      const sorted = [...samples].sort((a, b) => a - b);
      const p95Ms = sorted[Math.ceil(sorted.length * 0.95) - 1];
      return [
        name,
        {
          samples: samples.length,
          p95Ms,
          maxMs: sorted.at(-1)!,
          passed: p95Ms <= 100,
        },
      ];
    }),
  );
  return {
    limitMs: 100,
    minimumSamples: MIN_QUERY_SAMPLES,
    passed: Object.values(results).every((r) => r.passed),
    groups: results,
  };
}

export function requireQueryLatency(groups: Record<string, readonly number[]>) {
  const result = queryLatencyGate(groups);
  assert(
    result.passed,
    "Query P95 exceeds 100 ms: " +
      Object.entries(result.groups)
        .filter(([, r]) => !r.passed)
        .map(([name, r]) => `${name}=${r.p95Ms.toFixed(2)}ms`)
        .join(", "),
  );
  return result;
}

function reportGroups(value: unknown): Record<string, number[]> {
  const object = (v: unknown): v is Record<string, unknown> =>
    v !== null && typeof v === "object" && !Array.isArray(v);
  assert(
    object(value) && object(value.cliQueryTimings),
    "Report requires cliQueryTimings with raw samplesMs",
  );
  return Object.fromEntries(
    Object.entries(value.cliQueryTimings).map(([name, group]) => {
      assert(
        object(group) &&
          Array.isArray(group.samplesMs) &&
          group.samplesMs.every((v) => typeof v === "number"),
        `${name}: missing raw samplesMs`,
      );
      return [name, group.samplesMs];
    }),
  );
}

if (
  process.argv[1] &&
  path.resolve(process.argv[1]) === fileURLToPath(import.meta.url)
) {
  try {
    const { values } = parseArgs({
      args: process.argv.slice(2).filter((a) => a !== "--"),
      options: { report: { type: "string" } },
    });
    assert(values.report, "--report is required");
    assert(
      statSync(values.report).size <= 8 * 1024 * 1024,
      "Report exceeds 8 MiB",
    );
    const result = queryLatencyGate(
      reportGroups(JSON.parse(readFileSync(values.report, "utf8"))),
    );
    console.log(JSON.stringify(result));
    process.exitCode = result.passed ? 0 : 1;
  } catch (error) {
    console.error(error instanceof Error ? error.message : String(error));
    process.exitCode = 1;
  }
}
