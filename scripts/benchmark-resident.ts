/** Long-running synthetic truth and resource qualification; never reads user logs. */
import assert from "node:assert/strict";
import { spawn, spawnSync, type ChildProcess } from "node:child_process";
import { once } from "node:events";
import {
  appendFileSync,
  existsSync,
  mkdirSync,
  readFileSync,
  realpathSync,
  statSync,
  writeFileSync,
} from "node:fs";
import path from "node:path";
import { parseArgs } from "node:util";
import { performance } from "node:perf_hooks";
import { setTimeout as delay } from "node:timers/promises";
import { createNodeClient } from "@wombat/client/node";
import { prepareFirstInstall } from "./first-install-state.js";
import { directoryBytes } from "./benchmark-live-helpers.js";
import { fileSha256 } from "./benchmark-common.js";
import { checkBuild, sourceIdentity } from "./build-identity.js";
import {
  MIN_QUERY_SAMPLES,
  queryLatencyGate,
  requireQueryLatency,
} from "./query-latency-gate.js";

const { values } = parseArgs({
  args: process.argv.slice(2).filter((a) => a !== "--"),
  options: {
    "output-dir": { type: "string" },
    "duration-ms": { type: "string", default: "86400000" },
    "interval-ms": { type: "string", default: "60000" },
    "max-rss-mib": { type: "string", default: "256" },
    "measurements-per-task": { type: "string", default: "100" },
  },
});
assert(
  process.platform !== "win32",
  "Native process resource sampling currently requires POSIX",
);
assert(values["output-dir"], "--output-dir is required");
const repo = path.resolve(import.meta.dirname, ".."),
  output = path.resolve(values["output-dir"]);
assert(
  output !== repo && !output.startsWith(repo + path.sep),
  "Keep qualification output outside the repository",
);
assert(
  !existsSync(output),
  "Use a new qualification output directory; retain previous evidence and stop its owned service separately",
);
mkdirSync(output, { recursive: true, mode: 0o700 });
writeFileSync(
  path.join(output, "resident-owner.json"),
  JSON.stringify({ kind: "wombat-resident-v1" }) + "\n",
  { mode: 0o600 },
);
const data = path.join(output, "data"),
  source = path.join(output, "source");
prepareFirstInstall(output, [data]);
assert(!existsSync(data) && !existsSync(source));
mkdirSync(path.join(source, "sessions"), { recursive: true, mode: 0o700 });
const duration = Number(values["duration-ms"]),
  interval = Number(values["interval-ms"]),
  maxRss = Number(values["max-rss-mib"]),
  measurements = Number(values["measurements-per-task"]);
for (const n of [duration, interval, maxRss, measurements])
  assert(Number.isSafeInteger(n) && n > 0);
assert(
  duration <= 7 * 86400000 && interval >= 1000 && measurements <= 10000,
  "Qualification duration is at most seven days; interval is at least one second",
);
const binary = realpathSync(path.join(repo, "dist", "wombat-core"));
const at = new Date().toISOString();
const record = (type: string, payload: unknown) =>
  JSON.stringify({ type, payload, timestamp: at }) + "\n";
const usage = (response: string) =>
  record("event_msg", {
    type: "token_usage_record",
    thread_id: "task-0",
    turn_id: "turn",
    response_id: response,
    usage: { input_tokens: 100, output_tokens: 10, total_tokens: 110 },
  });
let expected = 0;
for (let task = 0; task < 100; task++) {
  let rows =
    record("session_meta", {
      id: "task-" + task,
      cwd: "/synthetic/project-" + (task % 5),
    }) + record("turn_context", { turn_id: "turn", model: "gpt-5.4" });
  for (let event = 0; event < measurements; event++) {
    rows += record("event_msg", {
      type: "token_usage_record",
      thread_id: "task-" + task,
      turn_id: "turn",
      response_id: `initial-${task}-${event}`,
      usage: { input_tokens: 100, output_tokens: 10, total_tokens: 110 },
    });
    expected += 110;
  }
  writeFileSync(path.join(source, "sessions", `task-${task}.jsonl`), rows, {
    mode: 0o600,
  });
}
Object.assign(process.env, {
  WOMBAT_DATA_HOME: data,
  CODEX_HOME: source,
  WOMBAT_AUTO_PRICES: "0",
  WOMBAT_ENDPOINT_FILE: path.join(output, "endpoint.json"),
});
delete process.env.WOMBAT_INSTALL_ROOT;
delete process.env.WOMBAT_CORE_BIN;
const client = createNodeClient({ automaticPrices: false });
const controller = new AbortController(),
  cancel = () => controller.abort();
process.once("SIGINT", cancel);
process.once("SIGTERM", cancel);
let child: ChildProcess | undefined,
  stderr = "";
const samples: Record<string, unknown>[] = [],
  peaks: number[] = [];
let completed = false;
const report: Record<string, unknown> = {
  format: 1,
  status: "running",
  ownerPid: process.pid,
  platform: process.platform + "/" + process.arch,
  sourceSha256: sourceIdentity(repo),
  coreSha256: fileSha256(binary),
  startedAt: new Date().toISOString(),
  durationMs: duration,
  intervalMs: interval,
  resourceThresholdMiB: maxRss,
  source: `100 synthetic tasks, ${measurements} measurements each; append ten measurements per interval`,
  cache:
    "release core; fresh product state; OS cache uncontrolled; no automatic price network",
  preparation: { fresh: true, sourceIsSynthetic: true },
  peakRssBytes: peaks,
  samples,
};
const save = () =>
  writeFileSync(
    path.join(output, "resident-report.json"),
    JSON.stringify(report, null, 2) + "\n",
    {
      mode: 0o600,
    },
  );
const status = () =>
  client.service!({ action: "status" }, { signal: controller.signal });
function unchangedBuild() {
  checkBuild(repo);
  assert.equal(
    JSON.parse(readFileSync(path.join(repo, "dist/build.json"), "utf8"))
      .sourceSha256,
    report.sourceSha256,
    "Qualification build changed",
  );
}
async function waitReady(prior?: string | null) {
  const deadline = Date.now() + 120_000;
  while (Date.now() < deadline) {
    if (child && (child.exitCode !== null || child.signalCode !== null))
      throw new Error("Resident process exited before readiness: " + stderr);
    const s = await status();
    if (s.errorCode) throw new Error("Indexing failed: " + s.errorCode);
    if (
      s.running &&
      s.ready &&
      !s.preparing &&
      (!prior || s.revision !== prior)
    )
      return s;
    await delay(100, undefined, { signal: controller.signal });
  }
  throw new Error("Committed readiness deadline exceeded");
}
async function start() {
  unchangedBuild();
  stderr = "";
  child = spawn(
    "/usr/bin/time",
    [process.platform === "darwin" ? "-l" : "-v", binary, "--daemon"],
    { stdio: ["ignore", "ignore", "pipe"], env: process.env, detached: true },
  );
  child.stderr?.on("data", (chunk: Buffer) => {
    stderr = (stderr + chunk.toString("utf8")).slice(-4096);
  });
  await once(child, "spawn");
  return waitReady();
}
async function stop() {
  if (!child || child.exitCode !== null || child.signalCode !== null) return;
  const closed = once(child, "close"),
    force = setTimeout(() => {
      try {
        if (child?.pid) process.kill(-child.pid, "SIGKILL");
      } catch {}
    }, 10_000);
  try {
    await client.service!({ action: "stop" });
    await closed;
    const match = stderr.match(
      process.platform === "darwin"
        ? /(\d+)\s+maximum resident set size/
        : /Maximum resident set size \(kbytes\):\s*(\d+)/,
    );
    assert(match, "Native peak-memory observation missing");
    peaks.push(Number(match[1]) * (process.platform === "darwin" ? 1 : 1024));
  } finally {
    clearTimeout(force);
  }
}
function resources() {
  const endpoint = JSON.parse(
    readFileSync(path.join(output, "endpoint.json"), "utf8"),
  );
  assert(
    Number.isSafeInteger(endpoint.pid) && endpoint.pid > 0,
    "Native service PID missing",
  );
  const result = spawnSync(
    "/bin/ps",
    ["-o", "rss=,time=", "-p", String(endpoint.pid)],
    {
      encoding: "utf8",
      timeout: 3000,
      maxBuffer: 65536,
    },
  );
  assert.ifError(result.error);
  assert.equal(result.status, 0);
  const match = result.stdout.trim().match(/^(\d+)\s+([\d:.]+)$/);
  assert(match, "Invalid process resource observation");
  const parts = match[2].split(":").map(Number);
  let seconds = 0;
  for (const part of parts) seconds = seconds * 60 + part;
  return { rssMiB: Number(match[1]) / 1024, cpuSeconds: seconds };
}
async function measureQuery(revision: string | null | undefined) {
  assert(revision, "Ready service has no committed revision");
  const startQuery = performance.now();
  const view = await client.live!(
    {
      query: { action: "usage", scope: { allTime: true }, compact: true },
      mode: "cached",
    },
    { signal: controller.signal },
  );
  const queryMs = performance.now() - startQuery;
  assert.equal(
    view.result.summary.tokens.total,
    expected,
    "Accounting differs from independent generated truth",
  );
  assert.equal(view.result.snapshotRef.snapshotId, revision);
  const cliStarted = performance.now();
  const cli = spawnSync(
    process.execPath,
    [path.join(repo, "dist/wombat.js"), "call"],
    {
      input: JSON.stringify({
        method: "usage",
        params: {
          query: { action: "usage", scope: { allTime: true }, compact: true },
          mode: "cached",
        },
      }),
      encoding: "utf8",
      timeout: 10000,
      maxBuffer: 2 * 1024 * 1024,
      env: process.env,
    },
  );
  const cliMs = performance.now() - cliStarted;
  assert.ifError(cli.error);
  assert([0, 2].includes(cli.status ?? -1), "CLI query failed");
  assert.equal(
    JSON.parse(cli.stdout).result.summary.tokens.total,
    expected,
    "CLI accounting differs from independent truth",
  );
  return { queryMs, cliMs };
}
function timingGroups(rows: readonly { queryMs: number; cliMs: number }[]) {
  return { sdk: rows.map((s) => s.queryMs), cli: rows.map((s) => s.cliMs) };
}
try {
  save();
  const firstStart = performance.now();
  let current = await start();
  report.initialPreparationMs = performance.now() - firstStart;
  // Reject a slow candidate before spending hours on increments and restarts.
  const preflight: { queryMs: number; cliMs: number }[] = [];
  for (let i = 0; i < MIN_QUERY_SAMPLES; i++)
    preflight.push(await measureQuery(current.revision));
  report.queryPreflightSamples = preflight;
  report.queryPreflight = queryLatencyGate(timingGroups(preflight));
  save();
  requireQueryLatency(timingGroups(preflight));
  const begun = performance.now(),
    deadline = begun + duration;
  let iteration = 0;
  while (performance.now() < deadline) {
    unchangedBuild();
    const tick = performance.now(),
      before = resources(),
      prior = current.revision;
    appendFileSync(
      path.join(source, "sessions/task-0.jsonl"),
      Array.from({ length: 10 }, (_, n) =>
        usage(`append-${iteration}-${n}`),
      ).join(""),
    );
    expected += 1100;
    current = await waitReady(prior);
    const appendMs = performance.now() - tick;
    const { queryMs, cliMs } = await measureQuery(current.revision);
    const after = resources();
    assert(
      after.rssMiB <= maxRss,
      `Steady process RSS exceeds qualification threshold: ${after.rssMiB} MiB`,
    );
    assert(current.parserCacheBytes <= current.parserCacheBudgetBytes);
    const index = path.join(data, "live-v2/index.sqlite");
    const sample = {
      iteration,
      elapsedMs: performance.now() - begun,
      appendMs,
      queryMs,
      cliMs,
      expectedTokens: expected,
      ...after,
      cpuSecondsBeforeUpdate: before.cpuSeconds,
      instanceId: current.instanceId,
      idleCpuSeconds:
        samples.at(-1)?.instanceId === current.instanceId
          ? before.cpuSeconds - Number(samples.at(-1)!.cpuSeconds)
          : null,
      productBytes: directoryBytes(data),
      databaseBytes: existsSync(index) ? statSync(index).size : 0,
      walBytes: existsSync(index + "-wal") ? statSync(index + "-wal").size : 0,
      cacheBytes: current.parserCacheBytes,
      revision: current.revision,
    };
    samples.push(sample);
    console.log(JSON.stringify(sample));
    save();
    iteration++;
    if (iteration % MIN_QUERY_SAMPLES === 0) {
      const accumulated = samples.map((s) => ({
        queryMs: Number(s.queryMs),
        cliMs: Number(s.cliMs),
      }));
      report.latestQueryGate = {
        iteration,
        population: "all_incremental_samples",
        ...queryLatencyGate(timingGroups(accumulated)),
      };
      save();
      requireQueryLatency(timingGroups(accumulated));
    }
    if (iteration % 60 === 0) {
      await stop();
      current = await start();
      assert(
        (
          await client.live!({
            query: { action: "usage", scope: { allTime: true } },
            mode: "cached",
          })
        ).result.summary.tokens.total === expected,
      );
    }
    await delay(
      Math.max(
        0,
        Math.min(
          deadline - performance.now(),
          interval - (performance.now() - tick),
        ),
      ),
      undefined,
      { signal: controller.signal },
    );
  }
  const timings = samples.map((s) => Number(s.queryMs)).sort((a, b) => a - b);
  unchangedBuild();
  report.queryP95Ms = timings[Math.ceil(timings.length * 0.95) - 1];
  assert(Number(report.queryP95Ms) <= 100, "Cached query P95 exceeds 100 ms");
  const cliTimings = samples.map((s) => Number(s.cliMs)).sort((a, b) => a - b);
  report.cliP95Ms = cliTimings[Math.ceil(cliTimings.length * 0.95) - 1];
  assert(Number(report.cliP95Ms) <= 100, "CLI cached query P95 exceeds 100 ms");
  completed = true;
  report.observedMs = performance.now() - begun;
} catch (error) {
  report.status = controller.signal.aborted ? "cancelled" : "failed";
  report.error = error instanceof Error ? error.message : String(error);
  report.stderr = stderr;
  throw error;
} finally {
  try {
    await stop();
    if (completed) {
      report.status = "passed";
      report.finishedAt = new Date().toISOString();
    }
  } catch (error) {
    report.status = "failed";
    report.cleanupError =
      error instanceof Error ? error.message : String(error);
    throw error;
  } finally {
    save();
    process.off("SIGINT", cancel);
    process.off("SIGTERM", cancel);
  }
}
