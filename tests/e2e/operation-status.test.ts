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
test('unknown tool outcomes agree in CLI/API, retain fixed views and survive service restart', { timeout: 40000 }, async () => {
    const dir = realpathSync.native(await mkdtemp(path.join(tmpdir(), 'wombat-operation-status-')));
    const root = path.join(dir, 'source'), project = path.join(dir, 'project'), file = path.join(root, 'sessions', 'one.jsonl');
    await mkdir(path.dirname(file), { recursive: true });
    await mkdir(project);
    await writeFile(path.join(project, 'AGENTS.md'), 'Synthetic instructions.');
    const row = (type: string, payload: unknown) => ({ type, timestamp: '2026-10-03T00:00:00Z', payload });
    const start = (call_id: string) => row('response_item', { type: 'function_call', call_id, name: 'read_file', arguments: JSON.stringify({ path: path.join(project, 'AGENTS.md') }) });
    const output = (call_id: string, output: unknown) => row('response_item', { type: 'function_call_output', call_id, output });
    const jsonl = (rows: unknown[]) => rows.map(r => JSON.stringify(r) + '\n').join('');
    await writeFile(file, jsonl([
        row('session_meta', { id: 't', cwd: project }), row('turn_context', { turn_id: 'u', model: 'gpt-5.4', effort: 'low' }),
        row('event_msg', { type: 'token_usage_record', thread_id: 't', turn_id: 'u', response_id: 'r', usage: { input_tokens: 100, cached_input_tokens: 20, cache_write_input_tokens: 0, output_tokens: 10, reasoning_output_tokens: 2, total_tokens: 110 } }),
        start('unknown'), output('unknown', 'PRIVATE_UNKNOWN_OUTPUT'), start('success'), output('success', { isError: false }),
        start('failure'), output('failure', { isError: true }), start('pending'),
    ]));
    const old = { WOMBAT_DATA_HOME: process.env.WOMBAT_DATA_HOME, WOMBAT_AUTO_PRICES: process.env.WOMBAT_AUTO_PRICES, CODEX_HOME: process.env.CODEX_HOME };
    Object.assign(process.env, { WOMBAT_DATA_HOME: path.join(dir, 'data'), WOMBAT_AUTO_PRICES: '0', CODEX_HOME: root });
    const binary = path.resolve('dist', process.platform === 'win32' ? 'wombat-core.exe' : 'wombat-core');
    let service = spawn(binary, ['--serve-usage'], { stdio: 'ignore', env: process.env });
    await once(service, 'spawn');
    const client = createNodeClient({ binaryPath: binary, automaticPrices: false });
    const nativeEntry = projectTestClient(client, [root], [project]);
    try {

        const nativeTransport = nativeEntry;
        const first = await nativeTransport.query({ action: 'refresh' });
        const threads = await nativeTransport.query({ action: 'threads', snapshotId: first.snapshotRef.snapshotId, scope: { allTime: true } });
        const thread = threads.items.find(i => i.kind === 'thread')!;
        assert.equal(thread.kind, 'thread');
        const turns = await nativeTransport.query({ action: 'turns', snapshotId: first.snapshotRef.snapshotId, threadId: thread.id, scope: { allTime: true } });
        const turn = turns.items.find(i => i.kind === 'turn')!;
        const query = { action: 'steps' as const, snapshotId: first.snapshotRef.snapshotId, threadId: thread.id, turnId: turn.id, scope: { allTime: true } };
        const states = (result: Awaited<ReturnType<typeof nativeTransport.query>>) => result.items.filter(i => i.kind === 'operation').map(i => i.status).sort();
        const before = await nativeTransport.query(query);
        assert.deepEqual(states(before), ['completed', 'failed', 'running', 'unknown']);
        assert.equal(before.summary.tokens.total, 110);
        const cli = spawnSync(process.execPath, [path.resolve('dist/wombat.js'), 'steps', '--snapshot', query.snapshotId, '--thread', thread.id, '--turn', turn.id, '--all-time', '--json'], { env: process.env, encoding: 'utf8', timeout: 15000 });
        assert.ifError(cli.error);
        assert.equal(cli.status, 0, cli.stderr + cli.stdout);
        assert.deepEqual(states(JSON.parse(cli.stdout)), states(before));
        await appendFile(file, jsonl([output('pending', 'PRIVATE_UNKNOWN_OUTPUT'), start('unknown')]));
        const second = await nativeTransport.query({ action: 'refresh' });
        const currentQuery = { ...query, snapshotId: second.snapshotRef.snapshotId };
        const after = await nativeTransport.query(currentQuery);
        assert.deepEqual(states(after), ['completed', 'failed', 'unknown', 'unknown']);
        assert.deepEqual(states(await nativeTransport.query(query)), states(before));
        assert.equal(after.summary.tokens.total, 110);
        const live = await nativeTransport.live!({ query: { action: 'usage', scope: { allTime: true } }, mode: 'fresh' });
        const inventory = await nativeTransport.config!({ kind: 'rule', snapshotId: live.result.snapshotRef.snapshotId, scope: { allTime: true, project } });
        const item = inventory.items.find(i => i.path === path.join(project, 'AGENTS.md'))!;
        assert.equal(item.counts.fileReads, 4);
        assert.equal(item.counts.succeeded, 1);
        assert.equal(item.counts.failed, 1);
        assert.equal(item.counts.outcomeUnknown, 2);
        assert.equal(item.usage?.tokens.total, 110);
        service.kill('SIGTERM');
        await once(service, 'exit');
        service = spawn(binary, ['--serve-usage'], { stdio: 'ignore', env: process.env });
        await once(service, 'spawn');
        assert.deepEqual((await nativeTransport.query(currentQuery)).items, after.items);
        assert.ok(!JSON.stringify([before, after, inventory]).includes('PRIVATE_UNKNOWN_OUTPUT'));
    }
    finally {
        service.kill('SIGTERM');
        await once(service, 'exit').catch(() => { });
        for (const [key, value] of Object.entries(old)) {
            if (value === undefined)
                delete process.env[key];
            else
                process.env[key] = value;
        }
        await rm(dir, { recursive: true, force: true });
    }
});
