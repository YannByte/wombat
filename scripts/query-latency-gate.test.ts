import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { mkdtempSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import os from "node:os";
import path from "node:path";
import { test } from "node:test";
import { queryLatencyGate, requireQueryLatency } from "./query-latency-gate.ts";

test("query gate uses raw P95, includes cold requests and stops before dependent work", () => {
  const samples = [...Array<number>(19).fill(100), 800];
  const result = requireQueryLatency({ cli: samples });
  assert.equal(result.groups.cli.p95Ms, 100);
  assert.equal(result.groups.cli.maxMs, 800);
  assert.equal(samples.at(-1), 800);
  const stages: string[] = [];
  assert.throws(() => {
    stages.push("query-gate");
    requireQueryLatency({ daily: samples, tasks: Array<number>(20).fill(101) });
    stages.push("idle", "restart", "disable");
  }, /tasks=101.00ms/);
  assert.deepEqual(stages, ["query-gate"]);
});

test("query gate rejects missing, short, zero and invalid samples", () => {
  const invalid: Record<string, readonly number[]>[] = [
    {},
    { cli: [10] },
    { cli: Array<number>(20).fill(0) },
    { cli: Array<number>(20).fill(NaN) },
    { cli: Array<number>(20).fill(Infinity) },
  ];
  for (const groups of invalid) {
    assert.throws(() => queryLatencyGate(groups));
  }
});

test("cumulative acceptance keeps slow samples without substituting a rolling-window population", () => {
  const complete = [...Array<number>(98).fill(75), 161, 171];
  assert.equal(requireQueryLatency({ cli: complete }).groups.cli.samples, 100);
  assert.equal(requireQueryLatency({ cli: complete }).groups.cli.maxMs, 171);
  assert.equal(queryLatencyGate({ cli: complete.slice(-20) }).passed, false);
  assert.throws(
    () =>
      requireQueryLatency({
        cli: [...complete, ...Array<number>(20).fill(160)],
      }),
    /P95 exceeds/,
  );
});

test("read-only report entry exits nonzero for latency violations and untrusted summary-only reports", () => {
  const dir = mkdtempSync(path.join(os.tmpdir(), "wombat-query-gate-"));
  try {
    const file = path.join(dir, "report.json");
    for (const [input, status] of [
      [
        {
          cliQueryTimings: {
            daily: { samplesMs: Array<number>(20).fill(75), p95Ms: 999 },
          },
        },
        0,
      ],
      [
        {
          cliQueryTimings: {
            daily: { samplesMs: Array<number>(20).fill(101), p95Ms: 1 },
          },
        },
        1,
      ],
      [{ cliQueryTimings: { daily: { p95Ms: 1 } } }, 1],
      [{ cliQueryTimings: {} }, 1],
    ] as const) {
      const content = JSON.stringify(input);
      writeFileSync(file, content);
      const run = spawnSync(
        process.execPath,
        ["--import", "tsx", "scripts/query-latency-gate.ts", "--report", file],
        { encoding: "utf8", timeout: 10000, maxBuffer: 65536 },
      );
      assert.ifError(run.error);
      assert.equal(run.status, status, run.stdout + run.stderr);
      assert.equal(readFileSync(file, "utf8"), content);
    }
  } finally {
    rmSync(dir, { recursive: true, force: true });
  }
});
