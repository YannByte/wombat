/** Release-core preparation/restart benchmark. Product state starts empty; sources stay read-only. */
import assert from 'node:assert/strict';
import { spawn, spawnSync } from 'node:child_process';
import { createConnection } from 'node:net';
import { mkdirSync, existsSync, writeFileSync, readFileSync, rmSync, utimesSync } from 'node:fs';
import path from 'node:path';
import { performance } from 'node:perf_hooks';
import { setTimeout as delay } from 'node:timers/promises';
import {
  createUsageClient,
  CoreError,
  type LiveRequest,
  type LiveResult,
  type UsageRequest,
} from '@wombat/client';
import { option, positiveInteger, fileSha256 } from './benchmark-common.js';
import { directoryBytes } from './benchmark-live-helpers.js';
import { validateReadinessPaths, readinessCliEnvironment } from './readiness-benchmark-paths.js';
import { requireQueryLatency } from './query-latency-gate.js';

assert(process.platform !== 'win32', 'Peak-RSS measurement requires POSIX /usr/bin/time');
const repo = path.resolve(import.meta.dirname, '..');
const output = path.resolve(option('--output'));
assert(
  !output.startsWith(repo + path.sep) && output !== repo,
  'Use an output directory outside the repository',
);
const core = path.resolve(option('--core', path.join(repo, 'dist/wombat-core')));
const cli = path.resolve(option('--cli', path.join(repo, 'dist/wombat.js')));
assert(existsSync(core) && existsSync(cli), 'Build the release core and CLI first');
const coreSha256 = fileSha256(core), cliSha256 = fileSha256(cli);
const sourceOption = process.argv.includes('--source-root')
  ? path.resolve(option('--source-root'))
  : undefined;
validateReadinessPaths(output, repo, sourceOption);
const marker = path.join(output, 'readiness-owner.json');
if (existsSync(output))
  assert(
    existsSync(marker) && JSON.parse(readFileSync(marker, 'utf8')).kind === 'wombat-readiness-v1',
    'Refuse to clear an unowned output directory',
  );
mkdirSync(output, { recursive: true, mode: 0o700 });
writeFileSync(marker, JSON.stringify({ kind: 'wombat-readiness-v1' }), { mode: 0o600 });
const data = path.join(output, 'data');
assert(
  !sourceOption ||
    !(
      sourceOption === data ||
      sourceOption.startsWith(data + path.sep) ||
      output === sourceOption ||
      output.startsWith(sourceOption + path.sep)
    ),
  'Source and disposable product paths must be separate',
);
// Preparation precedes service startup or the first query. No old checkpoint is reused.
rmSync(data, { recursive: true, force: true });
rmSync(path.join(output, 'runtime'), { recursive: true, force: true });
assert(!existsSync(data));
const source = sourceOption ?? path.join(output, 'source');
const day = new Date().toISOString().slice(0, 10);
const defaultSince = new Date(Date.parse(day + 'T00:00:00Z') - 2 * 86400000)
  .toISOString()
  .slice(0, 10);
const defaultUntil = new Date(Date.parse(day + 'T00:00:00Z') + 86400000).toISOString().slice(0, 10);
const since = option('--since', defaultSince),
  until = option('--until', defaultUntil),
  timezone = option('--timezone', 'UTC');
const hotFiles = 4,
  coldFiles = 40,
  records = positiveInteger('--records-per-file', 200);
if (!sourceOption) {
  rmSync(source, { recursive: true, force: true });
  mkdirSync(path.join(source, 'sessions'), { recursive: true, mode: 0o700 });
  for (let file = 0; file < hotFiles + coldFiles; file++) {
    const hot = file < hotFiles,
      id = `synthetic-${file}`,
      date = hot ? day : '2020-01-01';
    const rows = [
      JSON.stringify({
        type: 'session_meta',
        payload: { id, cwd: `/synthetic/project-${file % 3}` },
      }),
      JSON.stringify({ type: 'turn_context', payload: { turn_id: 'turn', model: 'gpt-5.4' } }),
    ];
    for (let record = 0; record < records; record++) {
      rows.push(
        JSON.stringify({
          type: 'event_msg',
          timestamp: date + 'T00:00:00Z',
          payload: {
            type: 'token_usage_record',
            response_id: `${id}-${record}`,
            usage: { input_tokens: 100, output_tokens: 10, total_tokens: 110 },
          },
        }),
      );
      rows.push(
        JSON.stringify({
          type: 'response_item',
          timestamp: date + 'T00:00:00Z',
          payload: {
            type: 'message',
            role: 'assistant',
            content: [{ type: 'output_text', text: 'SYNTHETIC_PRIVATE_TEXT_' + 'x'.repeat(1024) }],
          },
        }),
      );
    }
    const filePath = path.join(source, 'sessions', `${id}.jsonl`);
    writeFileSync(filePath, rows.join('\n') + '\n', { mode: 0o600 });
    if (!hot) utimesSync(filePath, new Date('2020-01-01'), new Date('2020-01-01'));
  }
}
assert(existsSync(source), 'Source root does not exist');
const env = {
  ...process.env,
  WOMBAT_DATA_HOME: data,
  WOMBAT_CORE_BIN: core,
  WOMBAT_AUTO_PRICES: '0',
  CODEX_HOME: source,
  WOMBAT_ENDPOINT_FILE: path.join(output, 'runtime', 'endpoint.json'),
};
const cliEnv = readinessCliEnvironment(core, cli, env);
const cliEntry = cliEnv.WOMBAT_CORE_BIN ? 'explicit core override' : 'managed installed discovery';
const endpoint = spawnSync(core, [], {
  input: JSON.stringify({ op: 'live_endpoint', args: {} }),
  env,
  encoding: 'utf8',
  timeout: 10_000,
  maxBuffer: 1024 * 1024,
});
assert.equal(endpoint.status, 0);
const socket = JSON.parse(endpoint.stdout).value.socket as string;
interface Service {
  child: ReturnType<typeof spawn>;
  closed: Promise<number | null>;
  stderr: string;
  rssSamples: { ms: number; phase: string; bytes: number }[];
  sampler: ReturnType<typeof setInterval>;
}
let phase = 'startup';
const services: Service[] = [];
async function start(): Promise<Service> {
  const timeArgs = process.platform === 'darwin' ? ['-l'] : ['-f', 'WB_MAX_RSS_KIB=%M'];
  const child = spawn('/usr/bin/time', [...timeArgs, core, '--serve-usage'], {
    env,
    stdio: ['ignore', 'ignore', 'pipe'],
    detached: true,
  });
  const start = performance.now(),
    rssSamples: Service['rssSamples'] = [];
  const sampler = setInterval(() => {
    // Synchronous process probes delay this parent's CLI completion callbacks.
    // Native /usr/bin/time still records the entire core lifetime's peak RSS.
    if (phase.endsWith('query-gate')) return;
    const pids =
      spawnSync('pgrep', ['-P', String(child.pid)], {
        encoding: 'utf8',
        timeout: 1000,
        maxBuffer: 4096,
      })
        .stdout?.trim()
        .split(/\s+/)
        .filter(Boolean) ?? [];
    const pid = pids[0] ?? String(child.pid);
    const sample = spawnSync('ps', ['-p', pid, '-o', 'pgid=,rss='], {
      encoding: 'utf8',
      timeout: 1000,
      maxBuffer: 4096,
    })
      .stdout?.trim()
      .split(/\s+/);
    if (sample?.length === 2 && Number(sample[0]) === child.pid && rssSamples.length < 2400)
      rssSamples.push({ ms: performance.now() - start, phase, bytes: Number(sample[1]) * 1024 });
  }, 500);
  sampler.unref();
  const service: Service = {
    child,
    stderr: '',
    rssSamples,
    sampler,
    closed: new Promise((resolve, reject) => {
      child.once('error', reject);
      child.once('close', resolve);
    }),
  };
  const ordinal = services.length;
  child.once('close', () => {
    clearInterval(sampler);
    writeFileSync(path.join(output, `native-resource-${ordinal}.log`), service.stderr, {mode:0o600});
  });
  services.push(service);
  child.stderr?.on('data', (chunk: Buffer) => {
    service.stderr = (service.stderr + chunk.toString()).slice(-8192);
  });
  const deadline = Date.now() + 10_000;
  while (!existsSync(socket)) {
    assert(Date.now() < deadline, 'Service endpoint did not become ready');
    await delay(20);
  }
  return service;
}
function rpc(request: LiveRequest): Promise<unknown> {
  return new Promise((resolve, reject) => {
    const stream = createConnection(socket);
    const chunks: Buffer[] = [];
    let bytes = 0,
      done = false;
    const timeout = setTimeout(() => finish(new Error('Benchmark request deadline')), 605_000);
    const finish = (error?: Error, value?: unknown) => {
      if (done) return;
      done = true;
      clearTimeout(timeout);
      stream.destroy();
      error ? reject(error) : resolve(value);
    };
    stream.on('connect', () => stream.write(JSON.stringify(request) + '\n'));
    stream.on('error', (error) => finish(error));
    stream.on('data', (chunk: Buffer) => {
      bytes += chunk.length;
      if (bytes > 2 * 1024 * 1024 + 1024) {
        finish(new Error('Benchmark response limit'));
        return;
      }
      chunks.push(chunk);
      if (chunk.includes(10)) {
        try {
          const response = JSON.parse(Buffer.concat(chunks).toString('utf8')) as {
            ok?: boolean;
            code?: string;
            value?: unknown;
          };
          if (!response.ok)
            throw new CoreError(response.code ?? 'CORE_ERROR', 'Core benchmark request failed');
          finish(undefined, response.value);
        } catch (error) {
          finish(error instanceof Error ? error : new Error('Invalid response'));
        }
      }
    });
    stream.on('end', () => {
      if (!done) finish(new Error('Incomplete benchmark response'));
    });
  });
}
const client = createUsageClient({
  query: async () => {
    throw new Error('Unexpected snapshot export');
  },
  prices: async () => {
    throw new Error('Unexpected price network');
  },
  live: (request) => rpc(request),
});
const scope = { since, until, timezone };
const query: UsageRequest = {
  action: 'usage',
  scope,
  compact: true,
  presentation: 'distribution',
  limit: 3,
};
const also: NonNullable<LiveRequest['also']> = [
  { action: 'threads', scope, compact: true, limit: 3 },
  { action: 'usage', scope, presentation: 'models', compact: true, limit: 3 },
  { action: 'usage', scope, presentation: 'projects', compact: true, limit: 3 },
];
const timings: Record<string, unknown> = {};
const cliQueryTimings: Record<string, {samplesMs: number[]}> = {};
function saveProgress(status: string) {
  writeFileSync(path.join(output, 'readiness-progress.json'), JSON.stringify({
    formatVersion: 1, stage:status, scope, coreSha256, cliSha256, cliEntry,
    preparation:{dataAbsentBeforeStartup:true,historySeeded:false}, timings, cliQueryTimings,
    observedPeakRssBytes:services.map(s=>Math.max(0,...s.rssSamples.map(sample=>sample.bytes))),
  },null,2)+'\n',{mode:0o600});
}
async function timed(name: string, request: LiveRequest): Promise<LiveResult> {
  const before = performance.now();
  const result = await client.live!(request);
  timings[name] = {
    elapsedMs: performance.now() - before,
    revision: result.freshness.revision,
    quality: result.result.quality.status,
    total: result.result.summary.tokens.total,
  };
  saveProgress(name);
  assert(!result.freshness.initialScan, 'A preview is not a completed measurement result');
  for (const additional of result.additional ?? [])
    assert.equal(additional.snapshotRef.snapshotId, result.result.snapshotRef.snapshotId);
  return result;
}
async function firstCli(request: LiveRequest, recordInitial = true): Promise<LiveResult> {
  const before = performance.now();
  const value = await new Promise<LiveResult>((resolve, reject) => {
    const child = spawn(process.execPath, [cli, 'call', '--json'], {
      env:cliEnv,
      stdio: ['pipe', 'pipe', 'pipe'],
      detached: true,
    });
    const chunks: Buffer[] = [];
    let bytes = 0,
      stderr = '',
      failure: Error | undefined;
    const fail = (error: Error) => {
      failure ??= error;
      if (child.pid) {
        try {
          process.kill(-child.pid, 'SIGTERM');
        } catch {}
      }
    };
    const timer = setTimeout(() => fail(new Error('CLI preparation deadline')), 605_000);
    child.stdout.on('data', (chunk: Buffer) => {
      bytes += chunk.length;
      if (bytes > 2 * 1024 * 1024 + 1024) fail(new Error('CLI output limit'));
      else chunks.push(chunk);
    });
    child.stderr.on('data', (chunk: Buffer) => {
      stderr = (stderr + chunk.toString()).slice(-4096);
    });
    child.on('error', fail);
    child.on('close', (status) => {
      clearTimeout(timer);
      if (failure) {
        reject(failure);
        return;
      }
      if (status !== 0 && status !== 2) {
        writeFileSync(path.join(output, 'failed-cli-output.json'), Buffer.concat(chunks), {
          mode: 0o600,
        });
        writeFileSync(path.join(output, 'failed-cli-stderr.log'), stderr, { mode: 0o600 });
        writeFileSync(
          path.join(output, 'failed-rss.json'),
          JSON.stringify(services.map((s) => s.rssSamples)),
          { mode: 0o600 },
        );
        reject(new Error('Initial CLI call failed; preserved output in ' + output));
        return;
      }
      try {
        resolve(JSON.parse(Buffer.concat(chunks).toString('utf8')) as LiveResult);
      } catch {
        reject(new Error('Invalid CLI JSON'));
      }
    });
    child.stdin.end(JSON.stringify({ method: 'usage', params: request }));
  });
  assert(!value.freshness.initialScan, 'CLI preview cannot establish usage');
  if (recordInitial) timings.recentPreparation = {
    elapsedMs: performance.now() - before,
    entry: 'assembled CLI call',
    revision: value.freshness.revision,
    quality: value.result.quality.status,
    total: value.result.summary.tokens.total,
  };
  if (recordInitial) saveProgress('recentPreparation');
  for (const additional of value.additional ?? [])
    assert.equal(additional.snapshotRef.snapshotId, value.result.snapshotRef.snapshotId);
  return value;
}

async function stopped(service: Service): Promise<number> {
  await Promise.race([
    service.closed,
    delay(30_000, undefined, { ref: false }).then(() => {
      throw new Error('Service did not finish its idle lifecycle');
    }),
  ]);
  const match =
    process.platform === 'darwin'
      ? /\s(\d+)\s+maximum resident set size/.exec(service.stderr)
      : /WB_MAX_RSS_KIB=(\d+)/.exec(service.stderr);
  assert(match, 'Missing peak-RSS evidence');
  return Number(match[1]) * (process.platform === 'darwin' ? 1 : 1024);
}
const note = (stage: string) => {
  phase = stage;
  process.stdout.write(JSON.stringify({ stage }) + '\n');
};
try {
  note('fresh-preparation');
  const service = await start();
  const first = await firstCli({ query, mode: 'fresh', waitMs: 600_000, also });
  if (!sourceOption) {
    assert.equal(first.result.summary.tokens.total, hotFiles * records * 110);
    assert(
      first.result.quality.sources.some((s) => s.issues.some((i) => i.code === 'historyDeferred')),
    );
  }
  writeFileSync(path.join(output, 'recent-result.json'), JSON.stringify(first), { mode: 0o600 });
  note('history-expansion');
  const complete = await timed('historyExpansion', {
    query,
    mode: 'fresh',
    includeHistory: true,
    waitMs: 600_000,
    also,
  });
  if (!sourceOption) assert.equal(complete.result.summary.tokens.total, hotFiles * records * 110);
  assert(
    !complete.result.quality.sources.some((s) =>
      s.issues.some((i) => i.code === 'historyDeferred'),
    ),
  );
  writeFileSync(path.join(output, 'history-result.json'), JSON.stringify(complete), {mode:0o600});
  const all = await timed('allTime', {
    query: { action: 'usage', scope: { allTime: true }, compact: true },
    mode: 'cached',
  });
  if (!sourceOption)
    assert.equal(all.result.summary.tokens.total, (hotFiles + coldFiles) * records * 110);
  writeFileSync(path.join(output, 'all-time-result.json'), JSON.stringify(all), {mode:0o600});
  note('complete-cli-query-gate');
  const entries: [string, UsageRequest][] = [
    ['usage', query], ['threads', also[0]], ['models', also[1]], ['projects', also[2]],
    ['statistics', {action:'statistics',scope}],
  ];
  for (const [name, entry] of entries) {
    const first: number[] = [], repeat: number[] = [];
    cliQueryTimings[name + 'DistinctRequests'] = {samplesMs:first};
    cliQueryTimings[name + 'RepeatedRequests'] = {samplesMs:repeat};
    for (let sample = 1; sample <= 20; sample++) {
      const request: LiveRequest = {query:{...entry, compact:false,limit:sample,snapshotId:complete.result.snapshotRef.snapshotId},mode:'cached'};
      for (const samples of [first,repeat]) {
        const began = performance.now();
        const result = await firstCli(request, false);
        samples.push(performance.now() - began);
        assert.deepEqual(result.result.summary, complete.result.summary, 'CLI query changed the selected usage');
      }
    }
    saveProgress('cli-query-group-' + name);
    writeFileSync(path.join(output,'query-gate-report.json'),JSON.stringify({
      cliQueryTimings, scope, cache:'Unique noncompact requests and exact repeats; ready pinned view; OS cache uncontrolled',
      coreSha256,cliSha256,cliEntry,
    },null,2)+'\n',{mode:0o600});
    requireQueryLatency(Object.fromEntries(Object.entries(cliQueryTimings).map(([name,group])=>[name,group.samplesMs])));
  }
  const peakFirst = await stopped(service);
  note('restart');
  const restarted = await start();
  const restored = await timed('restartRead', { query, mode: 'cached', also });
  assert.deepEqual(
    restored.result.summary,
    complete.result.summary,
    'Restart changed committed usage',
  );
  writeFileSync(path.join(output, 'restart-result.json'), JSON.stringify(restored), {mode:0o600});
  phase = 'restart-query-gate';
  const samples: number[] = [];
  const repeats: number[] = [];
  for (let sample = 1; sample <= 20; sample++) {
    const request: LiveRequest = {
      query: { ...query, snapshotId: restored.result.snapshotRef.snapshotId, limit: sample },
      mode: 'cached',
    };
    const begin = performance.now();
    const result = await client.live!(request);
    samples.push(performance.now() - begin);
    assert.deepEqual(result.result.summary, restored.result.summary);
    const repeat = performance.now();
    await client.live!(request);
    repeats.push(performance.now() - repeat);
  }
  const p95 = (values: number[]) =>
    values.toSorted((a, b) => a - b)[Math.ceil(values.length * 0.95) - 1];
  timings.uncachedQueries = { p95Ms: p95(samples), samplesMs: samples };
  timings.repeatedQueries = { p95Ms: p95(repeats), samplesMs: repeats };
  const peakRestart = await stopped(restarted);
  assert.equal(fileSha256(core), coreSha256, 'Measured core changed');
  assert.equal(fileSha256(cli), cliSha256, 'Measured CLI changed');
  const report = {
    formatVersion: 1,
    platform: process.platform,
    architecture: process.arch,
    coreSha256,
    cliSha256,
    cliEntry,
    resourceMeasurement:'Native OS core lifetime peak; 500 ms RSS polling paused during query timing to avoid delaying CLI callbacks',
    sourceRoot: source,
    sourceKind: sourceOption ? 'read-only real logs' : 'independent synthetic fixture',
    preparation: { dataAbsentBeforeStartup: true, historySeeded: false },
    scope,
    cache:
      'OS file cache uncontrolled; release core; uncached reductions and repeated requests reported separately',
    timings,
    cliQueryTimings,
    peakRssBytes: { first: peakFirst, restart: peakRestart },
    rssSamples: services.map((service) => service.rssSamples),
    productBytesAfterExit: directoryBytes(data),
    queryTargetMet: p95(samples) <= 100,
  };
  writeFileSync(
    path.join(output, 'readiness-report.json'),
    JSON.stringify(report, null, 2) + '\n',
    { mode: 0o600 },
  );
  process.stdout.write(
    JSON.stringify({
      report: path.join(output, 'readiness-report.json'),
      queryTargetMet: report.queryTargetMet,
    }) + '\n',
  );
} finally {
  for (const service of services) clearInterval(service.sampler);
  for (const service of services)
    if (service.child.exitCode === null && service.child.signalCode === null && service.child.pid) {
      try {
        process.kill(-service.child.pid, 'SIGTERM');
      } catch (error) {
        if ((error as NodeJS.ErrnoException).code !== 'ESRCH') throw error;
      }
      const stopped = await Promise.race([
        service.closed.then(() => true),
        delay(3000, undefined, { ref: false }).then(() => false),
      ]);
      if (!stopped) {
        try {
          process.kill(-service.child.pid, 'SIGKILL');
        } catch (error) {
          if ((error as NodeJS.ErrnoException).code !== 'ESRCH') throw error;
        }
        await service.closed;
      }
    }
}
