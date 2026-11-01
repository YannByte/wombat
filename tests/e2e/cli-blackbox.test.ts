import { queryInvocation, spawnProductCli, type CliInput } from '../../scripts/cli-invocation.js';
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { mkdtemp, mkdir, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { test } from 'node:test';

const repository = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const entry = process.env.WOMBAT_CLI_ENTRY ?? path.join(repository, 'dist/wombat.js');
const dates = ['--since', '2026-09-28', '--until', '2026-09-30'];
const row = (timestamp: string, type: string, payload: object) => ({ timestamp, type, payload });
const event = (timestamp: string, payload: object) => row(timestamp, 'event_msg', payload);
const counts = (input: number, read: number, write: number, output: number) => ({
  input_tokens: input, cached_input_tokens: read, cache_write_input_tokens: write,
  output_tokens: output, reasoning_output_tokens: 5, total_tokens: input + output,
});
const jsonl = (rows: object[]) => rows.map(value => JSON.stringify(value)).join('\n') + '\n';

async function stopFixture(env:NodeJS.ProcessEnv,cwd:string):Promise<void> {
  // The private data home binds this control request to the fixture's own runtime.
  // Release SQLite/file handles before removal instead of waiting for idle expiry.
  const result=spawnProductCli(entry,{method:'service',params:{action:'stop'}},{cwd,env,encoding:'utf8',timeout:30000,maxBuffer:65536});
  assert.ifError(result.error);assert.equal(result.status,0,result.stderr+result.stdout);
  assert.equal(JSON.parse(result.stdout).running,false,'Fixture service shutdown was not confirmed');
}

async function fixture() {
  // Recorded POSIX identities deliberately differ from the host filesystem grammar.
  // Query filters must preserve them on Windows, where these folders need not exist.
  const root = await mkdtemp(path.join(tmpdir(), 'wombat-cli-blackbox-'));
  const source = path.join(root, 'source with spaces');
  const data = path.join(root, 'data');
  await mkdir(path.join(source, 'sessions'), { recursive: true });
  await mkdir(path.join(source, 'archived_sessions'));
  const active = path.join(source, 'sessions', 'alpha.jsonl');
  await writeFile(active, jsonl([
    row('2026-09-28T23:00:00Z', 'session_meta', { id: 'alpha', cwd: '/synthetic/项目甲' }),
    event('2026-09-28T23:00:01Z', { type: 'thread_settings_applied', thread_id: 'alpha', thread_settings: { model: 'gpt-6-sol', model_provider_id: 'openai', reasoning_effort: 'high' } }),
    event('2026-09-28T23:00:02Z', { type: 'task_started', turn_id: 'a1' }),
    event('2026-09-28T23:30:00Z', { type: 'token_usage_record', thread_id: 'alpha', turn_id: 'a1', response_id: 'r1', usage: counts(160_000, 50_000, 10_000, 20_000) }),
    row('2026-09-28T23:31:00Z', 'response_item', { type: 'function_call', call_id: 'tool1', name: 'exec_command', arguments: '{"cmd":"SYNTHETIC_BODY_SENTINEL"}' }),
    row('2026-09-28T23:31:01Z', 'response_item', { type: 'function_call_output', call_id: 'tool1', output: '{"stdout":"SYNTHETIC_BODY_SENTINEL","exit_code":0}' }),
    event('2026-09-28T23:32:00Z', { type: 'task_complete', turn_id: 'a1' }),
    event('2026-09-29T00:00:00Z', { type: 'task_started', turn_id: 'a2' }),
    event('2026-09-29T00:30:00Z', { type: 'token_usage_record', thread_id: 'alpha', turn_id: 'a2', response_id: 'r2', usage: counts(290_000, 50_000, 10_000, 20_000) }),
    event('2026-09-29T00:31:00Z', { type: 'task_complete', turn_id: 'a2' }),
  ]));
  await writeFile(path.join(source, 'archived_sessions', 'beta.jsonl'), jsonl([
    row('2026-09-29T01:00:00Z', 'session_meta', { id: 'beta', cwd: '/synthetic/项目乙' }),
    row('2026-09-29T01:00:01Z', 'turn_context', { turn_id: 'b1', model: 'gpt-6-astra', effort: 'low' }),
    event('2026-09-29T01:00:02Z', { type: 'token_usage_record', thread_id: 'beta', turn_id: 'b1', response_id: 'r3', usage: counts(1_000, 200, 0, 100) }),
  ]));
  await writeFile(path.join(source, 'sessions', 'unpriced.jsonl'), jsonl([
    row('2026-09-29T02:00:00Z', 'session_meta', { id: 'unpriced' }),
    row('2026-09-29T02:00:01Z', 'turn_context', { turn_id: 'c1', model: 'synthetic-unpriced', effort: 'low' }),
    event('2026-09-29T02:00:02Z', { type: 'token_usage_record', thread_id: 'unpriced', turn_id: 'c1', response_id: 'r4', usage: counts(100, 0, 0, 10) }),
  ]));
  await writeFile(path.join(source, 'session_index.jsonl'), jsonl([
    { id: 'alpha', thread_name: 'Alpha 合成对话', updated_at: '2026-09-29T00:31:00Z' },
    { id: 'beta', thread_name: 'Beta 合成对话', updated_at: '2026-09-29T01:00:02Z' },
  ]));
  const env: NodeJS.ProcessEnv = { ...process.env, WOMBAT_AUTO_PRICES: '0', WOMBAT_LANG: 'zh', WOMBAT_DATA_HOME: data, CODEX_HOME: source, NO_COLOR: '1' };
  delete env.WOMBAT_CORE_BIN;
  const run = (args: CliInput, json = true) => {
    const result = spawnProductCli(entry, Array.isArray(args) ? [...args, ...(json ? ['--json'] : [])] : args, {
      cwd: root, env, encoding: 'utf8', input: '', timeout: 15_000, maxBuffer: 8 * 1024 * 1024,
    });
    assert.ifError(result.error);
    assert.doesNotMatch(result.stdout + result.stderr, /SYNTHETIC_BODY_SENTINEL/);
    if (json) assert.equal(result.stdout.trim().split('\n').length, 1, 'JSON stdout must contain one object');
    return { code: result.status, stdout: result.stdout, stderr: result.stderr, value: json ? JSON.parse(result.stdout) : undefined };
  };
  return { root, source, data, active, run, cleanup: async () => {
    await stopFixture(env,root);
    await rm(root, { recursive: true, force: true, maxRetries: 20, retryDelay: 1000 });
  } };
}

test('real CLI starts its shared service through a capability probe without scanning sources', async () => {
  const root = await mkdtemp(path.join(tmpdir(), 'wombat-cli-probe-'));
  const data = path.join(root, 'data');
  const missingSource = path.join(root, 'must-not-be-scanned');
  const env: NodeJS.ProcessEnv = {
    ...process.env,
    WOMBAT_AUTO_PRICES: '0',
    WOMBAT_DATA_HOME: data,
    CODEX_HOME: missingSource,
  };
  delete env.WOMBAT_CORE_BIN;
  try {
    const result = spawnProductCli(entry, {method:'optimize',params:{action:'capabilities'}}, {
      cwd: root, env, encoding: 'utf8', input: '', timeout: 15_000, maxBuffer: 8 * 1024 * 1024,
    });
    assert.ifError(result.error);
    assert.equal(result.status, 0, result.stdout + result.stderr);
    const value = JSON.parse(result.stdout);
    assert.equal(value.action, 'capabilities');
    assert.equal(value.resultStatus, 'complete');
    assert.equal(value.readView, null);
  } finally {
    await stopFixture(env,root);
    await rm(root, { recursive: true, force: true, maxRetries: 20, retryDelay: 1000 });
  }
});

test('real CLI refresh, usage, threads, turns and steps conserve independent Token and price totals', async () => {
  const f = await fixture();
  try {
    const digest = createHash('sha256').update(await readFile(f.active)).digest('hex');
    const initial = f.run(['usage', ...dates]);
    if (initial.code === 0) {
      assert.equal(initial.value.summary.tokens.total, 491_210);
    } else if (initial.code === 2) {
      assert.equal(initial.code, 2, initial.stdout);
      assert.equal(initial.value.freshness.status, 'syncing');
      assert.equal(initial.value.quality.issues.some((issue: any) => issue.code === 'initialScanIncomplete'), true);
    } else {
      assert.equal(initial.code, 1, initial.stdout);
      assert.equal(initial.value.error.code, 'SYNC_PENDING');
    }
    const refresh = f.run({"method":"usage","params":{"query":{"action":"refresh","roots":[f.source,f.source]},"mode":"auto"}});
    assert.equal(refresh.code, 0);
    assert.ok(['current','stale'].includes(refresh.value.freshness.status));
    assert.equal(refresh.value.summary.tokens.total, 491_210);
    // Sol short .435 + Sol long 1.29 + Astra .0132; fourth model has no price.
    assert.equal(refresh.value.summary.price.knownCost, '1.7382');
    assert.equal(refresh.value.summary.price.cost, null);
    assert.equal(refresh.value.summary.price.status, 'partial');
    const report = f.run(['usage', ...dates]);
    assert.equal(report.code, 0);
    assert.equal(report.stderr, '');
    assert.deepEqual(report.value.summary, refresh.value.summary);
    const threads = f.run({"method":"usage","params":{"query":{"action":"threads","sort":"tokens"},"mode":"auto"}});
    const alpha = threads.value.items[0];
    assert.equal(alpha.title, 'Alpha 合成对话');
    assert.equal(alpha.threadUsage.tokens.total, 490_000);
    assert.equal(alpha.threadUsage.price.cost, '1.725');
    const turns = f.run({"method":"usage","params":{"query":{"action":"turns","threadId":alpha.id,"sort":"tokens"},"mode":"auto"}});
    assert.equal(turns.code, 0);
    assert.equal(turns.value.summary.price.cost, '1.725');
    assert.deepEqual(turns.value.items.map((item: any) => item.usage.tokens.total), [310_000, 180_000]);
    const steps = f.run({"method":"usage","params":{"query":{"action":"steps","threadId":alpha.id,"turnId":turns.value.items[1].id,"sort":"time"},"mode":"auto"}});
    assert.equal(steps.code, 0);
    assert.equal(steps.value.summary.price.cost, '0.435');
    assert.equal(steps.value.items.filter((item: any) => item.kind === 'operation').length, 1);
    assert.equal(steps.value.items.filter((item: any) => item.kind === 'measurement').length, 1);
    const plain = f.run(['usage', ...dates], false);
    assert.match(plain.stdout, /\$1\.74\*/);
    assert.match(plain.stdout, /未计价/);
    assert.doesNotMatch(plain.stdout, /模型未知/);
    assert.equal(createHash('sha256').update(await readFile(f.active)).digest('hex'), digest);
  } finally { await f.cleanup(); }
});

test('real CLI filters, timezones, grouping and pagination retain the full matching totals', async () => {
  const f = await fixture();
  try {
    f.run({"method":"usage","params":{"query":{"action":"refresh","roots":[f.source]},"mode":"auto"}});
    for (const filter of [['--model', 'gpt-6-sol'], ['--effort', 'high'], ['--project', '/synthetic/项目甲']]) {
      const result = f.run(['usage', ...dates, ...filter]);
      assert.equal(result.code, 0, result.stdout);
      assert.equal(result.value.summary.tokens.total, 490_000);
      assert.equal(result.value.summary.price.cost, '1.725');
    }
    for (const filter of [['--model-unknown'], ['--effort-unknown'], ['--project', '/synthetic/项目'], ['--model', 'does-not-exist']]) {
      assert.equal(f.run(['usage', ...dates, ...filter]).value.summary.measurementCount, 0);
    }
    const utc = f.run(['usage', '--since', '2026-09-28', '--until', '2026-09-29', '--timezone', 'UTC']);
    assert.equal(utc.value.summary.tokens.total, 180_000);
    const shanghai = f.run(['usage', '--since', '2026-09-29', '--until', '2026-09-30', '--timezone', 'Asia/Shanghai']);
    assert.equal(shanghai.value.summary.tokens.total, 491_210);
    for (const group of ['day', 'week', 'month']) {
      assert.equal(f.run(['usage', ...dates, '--group', group]).value.summary.price.knownCost, '1.7382');
    }
    assert.equal(f.run({"method":"usage","params":{"query":{"action":"threads","search":"aLPHa"},"mode":"auto"}}).value.summary.tokens.total, 490_000);
    const first = f.run({"method":"usage","params":{"query":{"action":"threads","sort":"tokens","limit":1},"mode":"auto"}}).value;
    const ids = [first.items[0].id];
    for (const offset of [1, 2]) {
      const page = f.run(queryInvocation({action:'threads',sort:'tokens',limit:1,offset,snapshotId:first.snapshotRef.snapshotId},{mode:'cached'})).value;
      assert.deepEqual(page.summary, first.summary);
      ids.push(page.items[0].id);
    }
    assert.equal(new Set(ids).size, 3);
    const emptyPage = f.run({"method":"usage","params":{"query":{"action":"threads","offset":99},"mode":"auto"}});
    assert.equal(emptyPage.code, 0);
    assert.equal(emptyPage.value.items.length, 0);
    assert.deepEqual(emptyPage.value.summary, first.summary);
    const plain = f.run({"method":"usage","params":{"query":{"action":"threads","offset":99},"mode":"auto"}}, false);
    assert.doesNotMatch(plain.stdout, /100—99/);
    assert.deepEqual(JSON.parse(plain.stdout).items,[]);
  } finally { await f.cleanup(); }
});

test('real CLI rejects invalid requests and reports missing detail without scanning', async () => {
  const f = await fixture();
  try {
    const invalid = [
      ['usage', '--limit', '0'], ['usage', '--limit', '501'], ['usage', '--offset', '-1'],
      ['usage', '--offset', '9007199254740992'], ['usage', '--since', '2026-02-30'],
      ['usage', '--since', '2026-09-30', '--until', '2026-09-28'], ['usage', '--timezone', 'Invalid/Zone'],
      ['refresh', '--model', 'gpt-6-sol'], ['usage', '--fresh', '--cached'], ['usage', '--group', 'year'],
      ['threads', '--sort', 'time'], ['usage', '--model-unknown', '--model', 'gpt-6-sol'],
      ['usage', '--undated', '--since', '2026-09-28'], ['turns'], ['steps', '--thread', 'missing'],
      ['usage', '--model'], ['usage', '--limit', '1', '--limit', '2'], ['scan'],
    ];
    for (const args of invalid) {
      const result = f.run(args);
      assert.equal(result.code, 1, args.join(' '));
      assert.equal(result.value.error.code, 'INVALID_ARGUMENT', args.join(' '));
      assert.equal(result.stderr, '');
    }
    assert.equal(f.run(['--help']).code, 0);
    assert.equal(f.run(['--version']).code, 0);
    f.run({"method":"usage","params":{"query":{"action":"refresh","roots":[f.source]},"mode":"auto"}});
    const missing = f.run({"method":"usage","params":{"query":{"action":"turns","threadId":"missing"},"mode":"auto"}});
    assert.equal(missing.code, 1);
    assert.equal(missing.value.error.code, 'NOT_FOUND');
  } finally { await f.cleanup(); }
});

test('real CLI partial sources, failed refresh and corrupted snapshot have explicit outcomes', async () => {
  const f = await fixture();
  try {
    const partial = f.run({"method":"usage","params":{"query":{"action":"refresh","roots":[f.source,path.join(f.root, 'missing')]},"mode":"auto"}});
    assert.equal(partial.code, 2);
    assert.equal(partial.value.quality.status, 'partial');
    assert.equal(partial.value.summary.tokens.total, 491_210);
    const failed = f.run({"method":"usage","params":{"query":{"action":"refresh","roots":[path.join(f.root, 'missing')]},"mode":"auto"}});
    assert.equal(failed.code, 1);
    assert.equal(failed.value.error.code, 'SOURCE_UNREADABLE');
    const retained = f.run(queryInvocation({"action":"threads","snapshotId":partial.value.snapshotRef.snapshotId}, {mode:"auto"}));
    assert.equal(retained.code, 2);
    assert.equal(retained.value.snapshotRef.snapshotId, partial.value.snapshotRef.snapshotId);
    const ledger = path.join(f.data, 'usage-v4', 'generations', retained.value.snapshotRef.snapshotId, 'committed', 'ledger.json');
    await writeFile(ledger, '[]');
    const corrupt = f.run(['usage', ...dates, '--snapshot', retained.value.snapshotRef.snapshotId]);
    assert.equal(corrupt.code, 1);
    assert.equal(corrupt.value.error.code, 'SNAPSHOT_CORRUPT');
  } finally { await f.cleanup(); }
});

test('language changes presentation while real core data and source titles stay unchanged', async () => {
  const f = await fixture();
  try {
    // The first live query may contain only preview metadata while scanning.
    const refresh = f.run({"method":"usage","params":{"query":{"action":"refresh","roots":[f.source]},"mode":"auto"}});
    assert.equal(refresh.code, 0, refresh.stdout);
    assert.ok(['current','stale'].includes(refresh.value.freshness.status));
    const initial = f.run(['usage', ...dates]);
    assert.equal(initial.code, 0, initial.stdout);
    assert.equal(initial.value.summary.tokens.total, 491_210);
    const args = ['usage','--snapshot',initial.value.snapshotRef.snapshotId];
    const zh = f.run([...args, '--lang=zh']);
    const en = f.run(['--lang', 'en', ...args]);
    assert.equal(zh.code, en.code);
    for (const field of ['summary', 'items', 'scope', 'page', 'snapshotRef', 'quality']) assert.deepEqual(en.value[field], zh.value[field]);
    assert.equal(en.value.items[0], zh.value.items[0]);
    const plain = f.run(['usage', ...dates, '--lang=en'], false);
    assert.match(plain.stdout, /Wombat · Usage/);
    assert.match(plain.stdout, /Not priced/);
    const help = f.run(['--help', '--lang=en'], false);
    assert.match(help.stdout, /Usage/); assert.match(help.stdout, /wombat system/);
    const invalid = f.run(['usage', '--lang=en', '--unknown']);
    assert.equal(invalid.value.error.code, 'INVALID_ARGUMENT');
    assert.match(invalid.value.error.message, /Invalid .*option/);
  } finally { await f.cleanup(); }
});
