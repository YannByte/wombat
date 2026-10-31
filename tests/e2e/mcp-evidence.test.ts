import { projectTestClient } from '../helpers/project-client.js';
import type { UsageClient } from '@wombat/client';
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { realpathSync } from 'node:fs';
import { mkdtemp, mkdir, writeFile, appendFile, rm } from 'node:fs/promises';
import { spawn, spawnSync } from 'node:child_process';
import { once } from 'node:events';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { createNodeClient } from '@wombat/client/node';
const at = '2026-10-03T00:00:02Z';
const row = (type: string, payload: unknown) => ({ type, timestamp: at, payload });
const meta = (id: string, project: string, parent?: string) => row('session_meta', { id, cwd: project, ...(parent ? { forked_from_id: parent } : {}) });
const context = (turn_id: string) => row('turn_context', { turn_id, model: 'gpt-5.4', effort: 'low' });
const request = (call_id: string, name: string, args: unknown) => row('response_item', { type: 'function_call', call_id, name, arguments: JSON.stringify(args) });
const end = (call_id: string, turn_id: string, tool: string, failed = false) => row('event_msg', { type: 'mcp_tool_call_end', call_id, turn_id, invocation: { server: 'docs', tool, arguments: { private: 'SECRET_ARGUMENT' } }, duration: { secs: 0, nanos: 2000000 }, result: failed ? { Err: 'SECRET_ERROR' } : { Ok: { content: [{ type: 'text', text: 'SECRET_OUTPUT' }], isError: false } } });
const jsonl = (rows: unknown[]) => rows.map(r => JSON.stringify(r) + '\n').join('');
test('MCP attempts, resources, fork replay and append/restart share exact CLI and native evidence', { timeout: 60000 }, async () => {
    const dir = realpathSync.native(await mkdtemp(path.join(tmpdir(), 'wombat-mcp-evidence-'))), root = path.join(dir, 'source'), project = path.join(dir, 'project');
    await mkdir(path.join(root, 'sessions'), { recursive: true });
    await mkdir(project);
    await writeFile(path.join(root, 'config.toml'), "[mcp_servers.docs]\ncommand='synthetic-not-executed'\n");
    const tokens = { input_tokens: 100, cached_input_tokens: 20, cache_write_input_tokens: 0, output_tokens: 10, reasoning_output_tokens: 2, total_tokens: 110 };
    const call = end('call', 'u', 'search', true), read = end('resource', 'u', 'read_mcp_resource');
    const parent = [meta('parent', project), context('u'), row('event_msg', { type: 'token_usage_record', thread_id: 'parent', turn_id: 'u', response_id: 'r1', usage: tokens }), request('call', 'mcp__misleading__search', {}), call, call, request('resource', 'read_mcp_resource', { server: 'docs', uri: 'synthetic://SECRET_URI' }), read, request('list', 'list_mcp_resources', { server: 'docs' }), end('list', 'u', 'list_mcp_resources')];
    const parentFile = path.join(root, 'sessions/parent.jsonl');
    await writeFile(parentFile, jsonl(parent));
    const old = { WOMBAT_DATA_HOME: process.env.WOMBAT_DATA_HOME, WOMBAT_AUTO_PRICES: process.env.WOMBAT_AUTO_PRICES };
    Object.assign(process.env, { WOMBAT_DATA_HOME: path.join(dir, 'data'), WOMBAT_AUTO_PRICES: '0' });
    const binary = path.resolve('dist', process.platform === 'win32' ? 'wombat-core.exe' : 'wombat-core');
    let service = spawn(binary, ['--serve-usage'], { stdio: 'ignore', env: process.env });
    await once(service, 'spawn');
    const client = createNodeClient({ binaryPath: binary, automaticPrices: false });
    const scope = { roots: [root], projectRoots: [project], scope: { allTime: true } };
    const nativeEntry = projectTestClient(client, [root], [project]);
    const currentInventory = async () => {
        // A completed fresh read fixes its source boundary; a late notification can still mark it stale.
        const live = await client.live!({ query: { action: 'usage', roots: [root], scope: { allTime: true } }, mode: 'fresh' });
        assert.ok(['current', 'stale'].includes(live.freshness.status));
        assert.equal(live.freshness.initialScan, false);
        assert.equal(live.freshness.error, null);
        return client.config!({ ...scope, kind: 'mcp', snapshotId: live.result.snapshotRef.snapshotId });
    };
    try {

        const nativeTransport = nativeEntry;
        const live = await nativeTransport.live!({ query: { action: 'usage', scope: { allTime: true } }, mode: 'fresh' });
        const first = await nativeTransport.config!({ kind: 'mcp', snapshotId: live.result.snapshotRef.snapshotId, scope: { allTime: true } }), item = first.items.find(i => i.name === 'docs')!;
        assert.ok(item);
        assert.equal(item.usageCount, 2);
        assert.equal(item.counts.toolCalls, 1);
        assert.equal(item.counts.resourceReads, 1);
        assert.equal(item.counts.failed, 1);
        assert.equal(item.counts.succeeded, 1);
        assert.equal(item.usage?.tokens.total, 110);
        assert.equal(first.coverage.absenceObservable, false);
        assert.equal(item.useBasis?.status, 'observed');
        assert.deepEqual(item.useBasis?.coverage, { dispatchGaps: 0, identityGaps: 0, targetGaps: 0, timeGaps: 0, turnGaps: 0 });
        const evidence = await nativeTransport.config!({ action: 'evidence', itemId: item.id, readView: first.readView, scope: { allTime: true } });
        assert.deepEqual(evidence.evidence.map(e => e.eventType).sort(), ['resource_read', 'tool_call']);
        assert.ok(!JSON.stringify(evidence).includes('SECRET'));
        const cli = spawnSync(process.execPath, [path.resolve('dist/wombat.js'), 'optimize', 'inventory', '--action', 'evidence', '--root', root, '--project-root', project, '--item', item.id, '--read-view', first.readView!, '--all-time', '--json'], { env: process.env, encoding: 'utf8', timeout: 15000 });
        assert.equal(cli.status, 2, cli.stderr + cli.stdout);
        assert.deepEqual(JSON.parse(cli.stdout).evidence, evidence.evidence);
        // A child arrives later with copied completed events and a genuinely new retry.
        await writeFile(path.join(root, 'sessions/child.jsonl'), jsonl([meta('child', project, 'parent'), context('u'), call, request('resource', 'read_mcp_resource', { server: 'docs', uri: 'synthetic://SECRET_URI' }), read, context('v'), end('retry', 'v', 'search')]));
        const second = await currentInventory();
        assert.equal(second.items[0].usageCount, 3);
        assert.equal(second.items[0].counts.toolCalls, 2);
        assert.equal(second.items[0].counts.resourceReads, 1);
        assert.equal(second.items[0].usage?.tokens.total, 110);
        const frozen = await client.config!({ ...scope, readView: first.readView, kind: 'mcp' });
        assert.equal(frozen.items[0].usageCount, 2);
        await appendFile(parentFile, jsonl([end('next', 'u', 'read_file', true)]));
        const appended = await currentInventory();
        assert.equal(appended.items[0].usageCount, 4);
        assert.equal(appended.items[0].counts.failed, 2);
        service.kill('SIGTERM');
        await once(service, 'exit');
        service = spawn(binary, ['--serve-usage'], { stdio: 'ignore', env: process.env });
        await once(service, 'spawn');
        const restored = await currentInventory();
        assert.equal(restored.items[0].usageCount, 4);
        assert.deepEqual(restored.items[0].counts, appended.items[0].counts);
        assert.equal(restored.items[0].usage?.tokens.total, 110);
        // An unclassified resource result creates a target gap; the prefix-only request remains unproven.
        await appendFile(parentFile, jsonl([request('prefix', 'mcp__docs__unconfirmed', {}), end('ambiguous', 'u', 'read_mcp_resource')]));
        const uncertain = await currentInventory(), unknown = uncertain.items[0];
        assert.equal(unknown.usageCount, 4, 'unbound evidence must not erase confirmed uses');
        assert.equal(unknown.useBasis?.status, 'partial');
        assert.ok((unknown.useBasis?.coverage.targetGaps ?? 0) > 0);
        assert.equal(uncertain.coverage.absenceObservable, false);
        assert.deepEqual(unknown.counts, restored.items[0].counts, 'unproven operations do not become confirmed uses');
        assert.equal(unknown.usage?.tokens.total, 110);
        assert.equal((await client.config!({ ...scope, readView: restored.readView, kind: 'mcp' })).items[0].usageCount, 4, 'the retained complete view stays complete');
    }
    finally {
        service.kill('SIGTERM');
        await once(service, 'exit').catch(() => { });
        for (const [k, v] of Object.entries(old)) {
            if (v === undefined)
                delete process.env[k];
            else
                process.env[k] = v;
        }
        await rm(dir, { recursive: true, force: true });
    }
});
