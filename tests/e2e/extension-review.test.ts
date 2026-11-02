import {comparable} from '../helpers/comparable.js';
import {spawnProductCli} from '../../scripts/cli-invocation.js';
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { realpathSync } from 'node:fs';
import { mkdtemp, mkdir, writeFile, readFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { spawn, spawnSync } from 'node:child_process';
import { once } from 'node:events';
import { createNodeClient } from '@wombat/client/node';
import { projectTestClient } from '../helpers/project-client.js';
import { nativeCodexFixture } from '../helpers/native-codex.js';

test('extension review joins native metadata and pinned use without changing the ledger', { timeout: 60000 }, async () => {
  const dir = realpathSync.native(await mkdtemp(path.join(tmpdir(), 'wombat-extension-review-')));
  const root = path.join(dir, 'source'), project = path.join(dir, 'project');
  const file = path.join(root, 'plugins/cache/sample/skills/review/SKILL.md');
  const saved = { WOMBAT_DATA_HOME: process.env.WOMBAT_DATA_HOME, CODEX_HOME: process.env.CODEX_HOME, WOMBAT_AUTO_PRICES: process.env.WOMBAT_AUTO_PRICES };
  await mkdir(path.dirname(file), { recursive: true }); await mkdir(path.join(root, 'sessions')); await mkdir(project);
  const body = '---\nname: review\ndescription: Synthetic evidence\n---\n';
  await writeFile(file, body + 'Short body.');
  await writeFile(path.join(root, 'config.toml'), '[plugins."sample@synthetic"]\nenabled=true\n');
  const event = (type: string, payload: object) => ({ timestamp: '2026-10-29T01:00:00Z', type, payload });
  await writeFile(path.join(root, 'sessions/test.jsonl'), [
    event('session_meta', { id: 't', cwd: project }),
    event('turn_context', { turn_id: 'u', model: 'gpt-5.4', cwd: project }),
    event('response_item', { type: 'function_call', call_id: 'read', name: 'read_file', arguments: JSON.stringify({ path: file }) }),
    event('response_item', { type: 'function_call_output', call_id: 'read', output: 'Synthetic body' }),
    event('event_msg', { type: 'token_usage_record', thread_id: 't', turn_id: 'u', response_id: 'r', usage: { input_tokens: 100, cached_input_tokens: 20, cache_write_input_tokens: 0, output_tokens: 10, reasoning_output_tokens: 2, total_tokens: 110 } }),
  ].map(r => JSON.stringify(r)).join('\n') + '\n');
  const native = await nativeCodexFixture(dir);
  const skill = { name: 'sample:review', path: file, description: 'Synthetic evidence', scope: 'user', enabled: true };
  const mode = (skills: object[]) => writeFile(native.mode, JSON.stringify({ skills }));
  await mode([skill]);
  Object.assign(process.env, { WOMBAT_DATA_HOME: path.join(dir, 'data'), CODEX_HOME: root, WOMBAT_AUTO_PRICES: '0' });
  const binary = path.resolve('dist', process.platform === 'win32' ? 'wombat-core.exe' : 'wombat-core');

  const client = projectTestClient(createNodeClient({ binaryPath: binary, codexBinaryPath: native.binary, automaticPrices: false }), [root], [project]);
  const scope = { since: '2026-10-29', until: '2026-10-30', timezone: 'UTC', project };
  try {
    const usage = await client.live!({ query: { action: 'usage', scope }, mode: 'fresh' });
    const request = { review: true, kind: 'skill' as const, scope };
    const before = await client.config!(request);
    const item = before.items.find(i => i.path === file)!;
    assert.ok(item, 'native-only plugin Skill must join the physical inventory');
    assert.equal(item.usageCount, 1); assert.equal(item.relatedTasks, 1);
    assert.equal(item.contextFootprint?.contexts[0].name, 'sample:review');
    assert.equal(item.contextFootprint?.contexts[0].enabled, true);
    assert.equal(item.contextFootprint?.visibility, 'unconfirmed');
    assert.equal(before.extensionReview?.enabledItems, 1);
    assert.equal(before.extensionReview?.confirmedVisibleTokens, null);
    const estimate = item.contextFootprint?.estimate?.tokens;
    assert.ok(estimate && estimate > 0);
    const calls = await readFile(native.calls, 'utf8');
    const pinned = await client.config!({ review: true, kind: 'skill', scope });
    assert.deepEqual(comparable(pinned.items), comparable(before.items));
    assert.ok((await readFile(native.calls, 'utf8')).length > calls.length, 'each request captures current discovery');
    await writeFile(file, body + 'Long body. '.repeat(1000));
    const grown = await client.config!(request);
    assert.equal(grown.items.find(i => i.path === file)!.contextFootprint!.estimate!.tokens, estimate, "Catalog metadata estimate excludes an on-demand body");
    assert.ok(grown.items.find(i => i.path === file)!.bodyTokenEstimate!.tokens > item.bodyTokenEstimate!.tokens);
    await mode([{ ...skill, enabled: false }]);
    const disabled = await client.config!(request);
    assert.equal(disabled.extensionReview?.disabledItems, 1);
    assert.equal(disabled.items.find(i => i.path === file)?.usageCount, 1);
    await mode([skill]);
    const restored = await client.config!(request);
    assert.equal(restored.extensionReview?.enabledItems, 1);
    assert.equal(restored.items.find(i => i.path === file)?.id, item.id);
    const after = await client.live!({ query: { action: 'usage', scope  }, mode: 'cached' });
    assert.deepEqual(after.result.summary, usage.result.summary);
    assert.equal(after.result.summary.tokens.total, 110);
    await mode([skill, { ...skill, enabled: false }]);
    const conflict = await client.config!(request);
    assert.ok(conflict.coverage.issues.some(i => i.code === 'nativeSkillListPartial'));
    assert.ok(!conflict.items.find(i => i.path === file)?.contextFootprint?.contexts.length);
    await writeFile(native.mode, JSON.stringify({ skillsError: true }));
    const unavailable = await client.config!(request);
    assert.ok(unavailable.coverage.issues.some(i => i.code === 'CODEX_REQUEST_REJECTED'));
    assert.equal(unavailable.extensionReview?.enabledItems, 0);
    await mode([skill]);
    const cli = spawnProductCli(path.resolve('dist/wombat.js'), {method:'config',params:{review:true,kind:'skill',roots:[root],projectRoots:[project],scope }}, { env: { ...process.env, WOMBAT_CODEX_BIN: native.binary }, encoding: 'utf8', timeout: 15000 });
    assert.ok([0, 2].includes(cli.status!), cli.stderr + cli.stdout);
    assert.equal(JSON.parse(cli.stdout).extensionReview.enabledItems, 1);
    const api = spawnSync(process.execPath, [path.resolve('dist/wombat.js'), 'system', 'call'], { env: {...process.env,WOMBAT_CODEX_BIN:native.binary}, input: JSON.stringify({ method: 'config', params: { review: true, roots: [root], projectRoots: [project], kind: 'skill', scope } }), encoding: 'utf8', timeout: 15000 });
    assert.ok([0, 2].includes(api.status!), api.stderr + api.stdout);
    assert.equal(JSON.parse(api.stdout).items[0].contextFootprint.estimate.tokens, estimate);
    const outside = path.join(dir, 'outside/SKILL.md');
    await mkdir(path.dirname(outside)); await writeFile(outside, 'SYNTHETIC_OUTSIDE_BODY_MUST_NOT_BE_READ');
    await mode([skill, { ...skill, name: 'external-review', path: outside }]);
    const external = await client.config!(request);
    const externalItem = external.items.find(i => i.path === outside)!;
    assert.equal(external.extensionReview?.enabledItems, 2);
    assert.equal(externalItem.measurementStatus, 'nativeMetadataOnly');
    assert.equal(externalItem.bytes, null); assert.equal(externalItem.bodyTokenEstimate, null);
    assert.ok(externalItem.contextFootprint?.estimate);
    assert.ok(!JSON.stringify(external).includes('SYNTHETIC_OUTSIDE_BODY_MUST_NOT_BE_READ'));
    const methods = (await readFile(native.calls, 'utf8')).trim().split('\n').map(l => JSON.parse(l).method);
    assert.ok(methods.every(m => ['initialize', 'skills/list'].includes(m)));
  } finally {

    for (const [key, value] of Object.entries(saved)) { if (value === undefined) delete process.env[key]; else process.env[key] = value; }
    await native.waitForExit(); await rm(dir, { recursive: true, force: true });
  }
});
