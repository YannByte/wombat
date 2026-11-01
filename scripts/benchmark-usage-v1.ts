import {cliInvocation,type CliInput} from './cli-invocation.ts';
import type {AgentRequest} from '@wombat/client';
/** Deterministic synthetic CLI query benchmark; never reads real Agent logs. */
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, statSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { fileSha256, formatMicros, moneyMicros, option, positiveInteger, run } from './benchmark-common.js';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const threads = positiveInteger('--threads', 500);
const turns = positiveInteger('--turns', 10);
const samples = positiveInteger('--samples', 20);
const output = path.resolve(option('--output'));
assert(turns <= 50 && samples >= 5, 'turns must be 1..50 and samples >= 5');
const core = path.join(root, 'dist', process.platform === 'win32' ? 'wombat-core.exe' : 'wombat-core');
const cli = path.join(root, 'dist/wombat.js');
assert(existsSync(core) && existsSync(cli), 'Build the release product before this benchmark');
assert(process.platform === 'darwin' || process.platform === 'linux', 'RSS measurement requires macOS or Linux');
const hashes = { coreSha256: fileSha256(core), cliSha256: fileSha256(cli) };
const stamp = (date: Date) => date.toISOString().replace('.000Z', 'Z');
const row = (at: Date, type: string, payload: object) => ({ timestamp: stamp(at), type, payload });
const jsonl = (file: string, rows: object[]) => { mkdirSync(path.dirname(file), { recursive: true }); writeFileSync(file, rows.map(value => JSON.stringify(value)).join('\n') + '\n'); };
const json = (value: string): any => JSON.parse(value);
const verify = (summary: any, tokens: number, cost: bigint) => {
  assert.equal(summary.tokens.total, tokens);
  assert.equal(summary.price.status, 'priced');
  assert.equal(moneyMicros(summary.price.cost), cost);
};
const query = (env:NodeJS.ProcessEnv,request:AgentRequest)=>{const i=cliInvocation(request);return json(run(process.execPath,[cli,...i.args],env,120000,i.input).stdout);};
const p95 = (values: number[]) => [...values].sort((a, b) => a - b)[Math.ceil(values.length * 0.95) - 1];
const median = (values: number[]) => { const sorted = [...values].sort((a, b) => a - b); return (sorted[(sorted.length - 1) >> 1] + sorted[sorted.length >> 1]) / 2; };
const round = (value: number) => Math.round(value * 1000) / 1000;
function measure(env: NodeJS.ProcessEnv, args: CliInput) {
  const i=cliInvocation(args);
  const timer = process.platform === 'darwin' ? ['/usr/bin/time', '-l'] : ['/usr/bin/time', '-f', 'WB_MAX_RSS_KIB=%M'];
  const result = run(timer[0], [...timer.slice(1), process.execPath, cli, ...i.args], env,120000,i.input);
  const rss = process.platform === 'darwin'
    ? /(?:^|\n)\s*(\d+)\s+maximum resident set size/.exec(result.stderr)?.[1]
    : /WB_MAX_RSS_KIB=(\d+)/.exec(result.stderr)?.[1];
  assert(rss, 'time did not report peak RSS');
  return { sample: { wallMs: result.wallMs, peakRssBytes: Number(rss) * (process.platform === 'linux' ? 1024 : 1), stdoutBytes: Buffer.byteLength(result.stdout) }, stdout: result.stdout };
}
function filesUnder(directory: string): string[] {
  return readdirSync(directory, { withFileTypes: true }).flatMap(entry => {
    const target = path.join(directory, entry.name);
    return entry.isDirectory() ? filesUnder(target) : [target];
  }).sort();
}

const temporary = mkdtempSync(path.join(os.tmpdir(), 'wombat-query-benchmark-'));
try {
  const source = path.join(temporary, 'synthetic-codex');
  const origin = Date.parse('2026-09-28T00:00:00Z');
  console.error(`Generating ${threads} synthetic threads × ${turns} turns`);
  const titles: object[] = [];
  for (let index = 0; index < threads; index++) {
    const id = `synthetic-thread-${String(index).padStart(5, '0')}`;
    const project = `/synthetic/project-${String(index % 10).padStart(2, '0')}`;
    const rows: object[] = [row(new Date(origin + index * 1000), 'session_meta', { id, cwd: project })];
    for (let turn = 0; turn < turns; turn++) {
      const turnId = `turn-${String(turn).padStart(3, '0')}`;
      const at = origin + Math.floor(turn * 2 / turns) * 86_400_000 + turn * 300_000 + index * 1000;
      rows.push(row(new Date(at), 'turn_context', { turn_id: turnId, model: 'gpt-5.3-codex', effort: 'high', cwd: project }));
      rows.push(row(new Date(at), 'event_msg', { type: 'task_started', turn_id: turnId }));
      for (let response = 0; response < 2; response++) rows.push(row(new Date(at + (1 + response * 3) * 1000), 'event_msg', {
        type: 'token_usage_record', thread_id: id, turn_id: turnId, response_id: `response-${String(turn).padStart(3, '0')}-${response}`,
        usage: { input_tokens: 1000, cached_input_tokens: 200, cache_write_input_tokens: 0, output_tokens: 100, reasoning_output_tokens: 40, total_tokens: 1100 },
      }));
      rows.push(row(new Date(at + 2000), 'response_item', { type: 'function_call', call_id: `call-${String(turn).padStart(3, '0')}`, name: 'read_file', arguments: '{"path":"/synthetic/source.ts"}' }));
      rows.push(row(new Date(at + 3000), 'response_item', { type: 'function_call_output', call_id: `call-${String(turn).padStart(3, '0')}`, output: 'SYNTHETIC_BODY_NOT_FOR_SNAPSHOT' }));
      rows.push(row(new Date(at + 5000), 'event_msg', { type: 'task_complete', turn_id: turnId }));
    }
    jsonl(path.join(source, index % 5 === 0 ? 'archived_sessions' : 'sessions', `rollout-${String(index).padStart(5, '0')}.jsonl`), rows);
    titles.push({ id, thread_name: `合成对话 ${String(index).padStart(5, '0')}`, updated_at: '2026-09-29T23:59:59Z' });
  }
  jsonl(path.join(source, 'session_index.jsonl'), titles);
  const corpusHash = createHash('sha256');
  const files = filesUnder(source).filter(file => file.endsWith('.jsonl'));
  let bytes = 0;
  for (const file of files) {
    const relative = Buffer.from(path.relative(source, file).split(path.sep).join('/'));
    const data = readFileSync(file);
    const length = Buffer.alloc(8); length.writeBigUInt64BE(BigInt(relative.length)); corpusHash.update(length).update(relative);
    length.writeBigUInt64BE(BigInt(data.length)); corpusHash.update(length).update(data);
    bytes += data.length;
  }
  const measurements = threads * turns * 2;
  const totalTokens = measurements * 1100;
  const totalCost = BigInt(measurements) * 2835n;
  const corpus = { threads, turnsPerThread: turns, responsesPerTurn: 2, operationsPerTurn: 1, measurements, files: files.length, bytes, sha256: corpusHash.digest('hex'), expectedTotalTokens: totalTokens, expectedCostUsd: formatMicros(totalCost) };
  const env = { ...process.env, WOMBAT_DATA_HOME: path.join(temporary, 'data'), WOMBAT_CORE_BIN: core };
  const refreshedRun = measure(env, {"method":"usage","params":{"query":{"action":"refresh","roots":[source]},"mode":"auto"}});
  const refreshed = json(refreshedRun.stdout).result;
  verify(refreshed.summary, totalTokens, totalCost);
  const snapshot = refreshed.snapshotRef.snapshotId;
  const common = ['--snapshot', snapshot, '--timezone', 'UTC'];
  const generation = path.join(temporary, 'data/usage-v4/generations', snapshot, 'committed');
  const selected = json(readFileSync(path.join(generation, 'manifest.json'), 'utf8')).threads[0];
  const thread = selected.thread.id;
  const turn = Object.keys(selected.turns)[0];
  const commands = {
    usage:{method:'snapshot',params:{action:'usage',snapshotId:snapshot,scope:{since:'2026-09-28',until:'2026-09-30',timezone:'UTC'}}},
    threads:{method:'snapshot',params:{action:'threads',snapshotId:snapshot,sort:'tokens',scope:{timezone:'UTC'}}},
    turns:{method:'snapshot',params:{action:'turns',snapshotId:snapshot,threadId:thread,sort:'tokens',scope:{timezone:'UTC'}}},
    steps:{method:'snapshot',params:{action:'steps',snapshotId:snapshot,threadId:thread,turnId:turn,sort:'tokens',scope:{timezone:'UTC'}}},
  } satisfies Record<string,AgentRequest>;
  const measured = [];
  for (const [name, command] of Object.entries(commands)) {
    console.error(`Measuring ${name}: first invocation + ${samples} fresh-process warm samples`);
    const first = measure(env, command);
    assert(first.stdout.trim());
    const warmSamples = [];
    for (let index = 0; index < samples; index++) {
      const current = measure(env, command);
      assert.equal(current.stdout, first.stdout, `Fixed-snapshot text changed: ${name}`);
      warmSamples.push(current.sample);
    }
    const latencies = warmSamples.map(value => value.wallMs);
    measured.push({ command: name, output: 'Generated JSON, piped stdout', firstInvocation: first.sample, warmSamples,
      warmP50Ms: round(median(latencies)), warmP95Ms: p95(latencies),
      maxObservedRssBytes: Math.max(first.sample.peakRssBytes, ...warmSamples.map(value => value.peakRssBytes)),
      warmP95Within300Ms: p95(latencies) <= 300, firstInvocationWithin1000Ms: first.sample.wallMs <= 1000 });
  }
  const usage = query(env, commands.usage); verify(usage.summary, totalTokens, totalCost);
  const subtotal = usage.items.filter((value: any) => value.isSubtotal);
  assert.equal(subtotal.reduce((sum: number, value: any) => sum + value.usage.tokens.total, 0), totalTokens);
  assert.equal(subtotal.reduce((sum: bigint, value: any) => sum + moneyMicros(value.usage.price.cost), 0n), totalCost);
  const seen = new Set<string>(); let pageTokens = 0; let pageCost = 0n; let offset = 0;
  do {
    const page = query(env, {...commands.threads,params:{...commands.threads.params,limit:50,offset}});
    verify(page.summary, totalTokens, totalCost); assert.equal(page.page.total, threads);
    for (const item of page.items) {
      assert(!seen.has(item.id)); seen.add(item.id);
      pageTokens += item.threadUsage.tokens.total; pageCost += moneyMicros(item.threadUsage.price.cost);
    }
    offset = page.page.nextOffset;
  } while (offset !== null);
  assert.equal(seen.size, threads); assert.equal(pageTokens, totalTokens); assert.equal(pageCost, totalCost);
  const turnCost = BigInt(turns) * 5670n;
  const turnResult = query(env, commands.turns); verify(turnResult.summary, turns * 2200, turnCost); assert.equal(turnResult.items.length, turns);
  for (const item of turnResult.items) { verify(item.usage, 2200, 5670n); assert(Math.abs(item.share - 1 / turns) < 1e-12); }
  const pagedTurn = query(env, {...commands.turns,params:{...commands.turns.params,limit:1}});
  assert.deepEqual(pagedTurn.summary, turnResult.summary); assert.deepEqual(pagedTurn.items[0], turnResult.items[0]);
  const steps = query(env, commands.steps); verify(steps.summary, 2200, 5670n); assert.equal(steps.items.length, 3);
  for (const item of steps.items) {
    if (item.kind === 'measurement') { verify(item.usage, 1100, 2835n); assert.equal(item.share, 0.5); }
    else { assert(!('usage' in item) && !('price' in item)); }
  }
  const pagedStep = query(env, {...commands.steps,params:{...commands.steps.params,limit:1}});
  assert.deepEqual(pagedStep.summary, steps.summary); assert.deepEqual(pagedStep.items[0], steps.items[0]);
  assert.deepEqual({ coreSha256: fileSha256(core), cliSha256: fileSha256(cli) }, hashes, 'Build changed during benchmark');
  const snapshotBytes = filesUnder(generation).reduce((sum, file) => sum + statSync(file).size, 0);
  const report = {
    benchmark: 'usage-v1-query', generatedAt: new Date().toISOString(),
    environment: { os: os.type(), osVersion: os.release(), architecture: os.arch(), nodeVersion: process.version, cpuCount: os.cpus().length },
    build: { profile: 'release (built by project build script)', ...hashes },
    scope: 'Each measured invocation starts Node CLI and Rust core, reads a fixed snapshot, serializes its DTO and renders Chinese text to a pipe; wall time includes /usr/bin/time launch. No TUI key navigation is measured.',
    cacheControl: 'The first invocation of each command is a fresh-process first-use measurement after refresh. OS file cache is NOT flushed; refresh and preceding commands may warm files. Warm samples also start fresh Node and Rust processes. No in-process cache or daemon is reused. Physical-disk cold-cache latency is not verified.',
    rssMethod: 'Darwin /usr/bin/time -l maximum resident set size in bytes (or GNU time %M KiB converted to bytes), for the launched command and waited child resource accounting. This is the maximum reported process RSS, not simultaneous summed process-tree memory; fixture generator is excluded.',
    percentileMethod: 'p50 median, p95 nearest-rank ceil(0.95*N), without discarding outliers',
    corpus, snapshotBytes, refresh: refreshedRun.sample, queries: measured,
    consistency: { passed: true, fullTokenTotal: totalTokens, fullCostUsd: formatMicros(totalCost), threadPagesChecked: Math.ceil(threads / 50), fixedSnapshotTextStable: true, dailySubtotalsConserved: true, threadPagesConserved: true, turnAndStepAmountsConserved: true, sharesIndependentOfPagination: true, operationHasNoInventedCost: true },
    targets: { warmP95Ms: 300, firstInvocationMs: 1000, allWarmTargetsMet: measured.every(value => value.warmP95Within300Ms), allFirstInvocationTargetsMet: measured.every(value => value.firstInvocationWithin1000Ms), physicalDiskColdCacheVerified: false },
    reproduce: `corepack pnpm build && node --import tsx scripts/benchmark-usage-v1.ts --threads ${threads} --turns ${turns} --samples ${samples} --output <report.json>`,
  };
  mkdirSync(path.dirname(output), { recursive: true }); writeFileSync(output, JSON.stringify(report, null, 2) + '\n');
  console.log(JSON.stringify({ output, targets: report.targets, queries: measured.map(value => ({ command: value.command, p95Ms: value.warmP95Ms, peakRssBytes: value.maxObservedRssBytes })) }));
} finally { rmSync(temporary, { recursive: true, force: true }); }
