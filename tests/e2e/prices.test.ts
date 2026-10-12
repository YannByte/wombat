import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, mkdir, readFile, writeFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import { spawnSync } from 'node:child_process';

const entry = path.resolve(process.env.WOMBAT_CLI_ENTRY ?? 'dist/wombat.js');
test('first offline scan prices Sol 6.1 at request boundaries without a download', async () => {
  const root = await mkdtemp(path.join(tmpdir(), 'wombat-sol61-prices-'));
  try {
    const source = path.join(root, 'source');
    await mkdir(path.join(source, 'sessions'), { recursive: true });
    const rows = [
      { type: 'session_meta', payload: { id: 'sol61-price-case' } },
      { type: 'turn_context', payload: { turn_id: 'turn', model: 'gpt-6.1-sol' } },
      ...[272000, 272001].map((input, index) => ({ type: 'event_msg', payload: {
        type: 'token_usage_record', thread_id: 'sol61-price-case', turn_id: 'turn', response_id: `response-${index}`,
        usage: { input_tokens: input, cached_input_tokens: 50000, cache_write_input_tokens: 0,
          output_tokens: 10000, reasoning_output_tokens: 1000, total_tokens: input + 10000 },
      } })),
    ].map(row => ({ timestamp: '2026-10-12T00:00:00Z', ...row }));
    await writeFile(path.join(source, 'sessions', 'source.jsonl'), rows.map(row => JSON.stringify(row)).join('\n') + '\n');
    const env = { ...process.env, WOMBAT_AUTO_PRICES: '0', WOMBAT_DATA_HOME: path.join(root, 'data'),
      CODEX_HOME: source, WOMBAT_CORE_BIN: '', NODE_OPTIONS: '' };
    const result = spawnSync(process.execPath, [entry, 'refresh', '--root', source, '--json'],
      { env, encoding: 'utf8', timeout: 15000, maxBuffer: 2 * 1024 * 1024 });
    assert.ifError(result.error);
    assert.equal(result.status, 0, result.stdout + result.stderr);
    const value = JSON.parse(result.stdout);
    assert.equal(value.summary.tokens.total, 564001);
    assert.equal(value.summary.unpricedTokens, 0);
    assert.equal(value.summary.price.status, 'priced');
    // 222000*2/M + 50000*.1/M + 10000*10/M, plus the entire long request.
    assert.equal(value.summary.price.cost, '1.597004');
    assert.equal(value.summary.price.basis[0].originalModel, 'gpt-6.1-sol');
    await assert.rejects(readFile(path.join(root, 'data', 'prices', 'active.json')), { code: 'ENOENT' });
  } finally { await rm(root, { recursive: true, force: true, maxRetries: 20, retryDelay: 1000 }); }
});
test('built CLI price update changes new snapshots only, is idempotent, and failures retain prices', async () => {
  const root = await mkdtemp(path.join(tmpdir(), 'wombat-price-e2e-'));
  try {
    const document = await readFile('core/tests/fixtures/official-pricing-synthetic.txt', 'utf8');
    const hook = path.join(root, 'official-source.mjs');
    await writeFile(hook, `globalThis.fetch = async (url, options) => {
      if (url !== 'https://developers.openai.com/api/docs/pricing.md' || options.redirect !== 'error') throw new Error('unexpected destination');
      if (process.env.TEST_PRICE_ERROR) throw new Error('offline');
      return new Response(process.env.TEST_PRICE_BAD ? '# Pricing\\nInvalid' : ${JSON.stringify(document)});
    };`);
    const env: NodeJS.ProcessEnv = { ...process.env, WOMBAT_AUTO_PRICES: '0', WOMBAT_LANG: 'zh', WOMBAT_DATA_HOME: path.join(root, 'data'), NODE_OPTIONS: '', NO_COLOR: '1' };
    delete env.WOMBAT_CORE_BIN;
    const run = (args: string[], extra: NodeJS.ProcessEnv = {}) => {
      const result = spawnSync(process.execPath, ['--import', pathToFileURL(hook).href, entry, ...args, '--json'], { env: { ...env, ...extra }, encoding: 'utf8', timeout: 15_000 });
      assert.ifError(result.error);
      assert.equal(result.stdout.trim().split('\n').length, 1);
      return { ...result, value: JSON.parse(result.stdout) };
    };
    const initial = run(['prices']);
    assert.equal(initial.status, 0);
    assert.equal(initial.value.origin, 'bundled');
    const source = path.join(root, 'source');
    await mkdir(path.join(source, 'sessions'), { recursive: true });
    const rows = [
      { type: 'session_meta', payload: { id: 'price-case' } },
      { type: 'turn_context', payload: { turn_id: 'turn', model: 'gpt-6-sol' } },
      { type: 'event_msg', payload: { type: 'token_usage_record', thread_id: 'price-case', turn_id: 'turn', response_id: 'response', usage: { input_tokens: 160000, cached_input_tokens: 50000, cache_write_input_tokens: 10000, output_tokens: 20000, reasoning_output_tokens: 0, total_tokens: 180000 } } },
    ].map(row => ({ timestamp: '2026-09-29T00:00:00Z', ...row }));
    await writeFile(path.join(source, 'sessions', 'source.jsonl'), rows.map(row => JSON.stringify(row)).join('\n') + '\n');
    const before = run(['refresh', '--root', source]);
    assert.equal(before.status, 0);
    assert.equal(before.value.summary.price.cost, '0.435');
    const updated = run(['prices', 'update']);
    assert.equal(updated.status, 0, updated.stderr);
    assert.equal(updated.value.updated, true);
    assert.equal(updated.value.catalog.models.length, 10);
    assert.equal(updated.value.catalog.models.find((m: { id: string }) => m.id === 'synthetic-new').rates.input, '1.23');
    assert.match(updated.stderr, /校验并保存/);
    assert.equal(run(['prices', 'update']).value.updated, false);
    const status = run(['prices', 'status']);
    assert.equal(status.value.catalogHash, updated.value.catalogHash);
    assert.equal(status.stderr, '');
    const old = run(['usage', '--snapshot', before.value.snapshotRef.snapshotId, '--since', '2026-09-29', '--until', '2026-09-30']);
    assert.equal(old.value.summary.price.cost, '0.435');
    const after = run(['refresh', '--root', source]);
    assert.equal(after.value.summary.price.cost, '0.4125');
    assert.equal(after.value.summary.price.priceRevision, updated.value.catalog.revision);
    assert.equal(after.value.summary.tokens.total, before.value.summary.tokens.total);
    for (const [extra, code] of [[{ TEST_PRICE_ERROR: '1' }, 'PRICE_FETCH_FAILED'], [{ TEST_PRICE_BAD: '1' }, 'PRICE_SOURCE_CHANGED']] as const) {
      const failed = run(['prices', 'update'], extra);
      assert.equal(failed.status, 1);
      assert.equal(failed.value.error.code, code);
      assert.equal(run(['prices']).value.catalogHash, updated.value.catalogHash);
    }
    assert.equal(run(['prices', '--url', 'https://example.com']).value.error.code, 'INVALID_ARGUMENT');
    // Ordinary refresh must not touch the network even after a live catalog is installed.
    assert.equal(run(['refresh', '--root', source], { TEST_PRICE_ERROR: '1' }).status, 0);
  } finally { await rm(root, { recursive: true, force: true, maxRetries: 20, retryDelay: 1000 }); }
});

test('missing model prices trigger one official fetch across processes, preserve fixed views and reprice live totals', async () => {
  const root = await mkdtemp(path.join(tmpdir(), 'wombat-auto-prices-'));
  try {
    const document = await readFile('core/tests/fixtures/official-pricing-synthetic.txt', 'utf8');
    const hook = path.join(root, 'official-source.mjs');
    const count = path.join(root, 'fetches');
    await writeFile(hook, `import fs from 'node:fs';globalThis.fetch = async (url, options) => {
      if (url !== 'https://developers.openai.com/api/docs/pricing.md' || options.redirect !== 'error') throw Error('unexpected destination');
      fs.appendFileSync(${JSON.stringify(count)}, 'fetch\\n');
      if (process.env.TEST_PRICE_ERROR) return new Response('forbidden', {status:403});
      return new Response(${JSON.stringify(document)});
    };`);
    const source = path.join(root, 'source');
    await mkdir(path.join(source, 'sessions'), { recursive: true });
    const rows = [
      { type: 'session_meta', payload: { id: 'auto-price' } },
      { type: 'turn_context', payload: { turn_id: 'turn', model: 'synthetic-new' } },
      { type: 'event_msg', payload: { type: 'token_usage_record', thread_id: 'auto-price', turn_id: 'turn', response_id: 'response', usage: { input_tokens: 100, cached_input_tokens: 0, cache_write_input_tokens: 0, output_tokens: 10, reasoning_output_tokens: 0, total_tokens: 110 } } },
    ].map(row => ({ timestamp: '2026-09-29T00:00:00Z', ...row }));
    await writeFile(path.join(source, 'sessions', 'source.jsonl'), rows.map(row => JSON.stringify(row)).join('\n') + '\n');
    const env = { ...process.env, WOMBAT_AUTO_PRICES: '1', WOMBAT_LANG: 'zh', WOMBAT_DATA_HOME: path.join(root, 'data'), CODEX_HOME: source, NODE_OPTIONS: '' };
    const run = (args: string[], extra = {}) => {
      const p = spawnSync(process.execPath, ['--import', pathToFileURL(hook).href, entry, ...args, '--json'], { env: { ...env, ...extra }, encoding: 'utf8', timeout: 25_000 });
      assert.ifError(p.error); assert.ok(p.status === 0 || p.status === 2, p.stdout + p.stderr);
      return JSON.parse(p.stdout);
    };
    const saved = run(['refresh'], { WOMBAT_AUTO_PRICES: '0' });
    assert.equal(saved.summary.price.cost, null);
    const query = ['usage', '--since', '2026-09-29', '--until', '2026-09-30'];
    assert.equal(run([...query, '--cached']).priceUpdate, undefined);
    assert.equal(run([...query, '--snapshot', saved.snapshotRef.snapshotId]).summary.price.cost, null);
    await assert.rejects(readFile(count), { code: 'ENOENT' });
    const failed = run([...query, '--fresh'], { TEST_PRICE_ERROR: '1' });
    assert.equal(failed.priceUpdate.status, 'failed');
    assert.equal(failed.priceUpdate.errorCode, 'PRICE_FETCH_FAILED');
    assert.equal(failed.summary.tokens.total, 110);
    assert.equal(run([...query, '--fresh']).priceUpdate.status, 'failed');
    assert.equal((await readFile(count, 'utf8')).trim().split('\n').length, 1);
    const statePath = path.join(root, 'data', 'prices', 'automatic.json');
    const state = JSON.parse(await readFile(statePath, 'utf8')); state.retryAt = '2000-01-01T00:00:00Z';
    await writeFile(statePath, JSON.stringify(state));
    const updated = run([...query, '--fresh']);
    assert.equal(updated.priceUpdate.status, 'updated');
    // Independent synthetic rates: 100 input at 1.23/M + 10 output at 4.56/M.
    assert.equal(updated.summary.price.cost, '0.0001686');
    assert.equal(updated.summary.tokens.total, 110);
    assert.equal(updated.summary.price.status, 'priced');
    assert.equal(run([...query, '--snapshot', saved.snapshotRef.snapshotId]).summary.price.cost, null);
    assert.equal(run([...query, '--fresh']).priceUpdate, undefined);
    assert.equal((await readFile(count, 'utf8')).trim().split('\n').length, 2);
  } finally { await rm(root, { recursive: true, force: true, maxRetries: 20, retryDelay: 1000 }); }
});
