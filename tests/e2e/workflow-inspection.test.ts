import {queryInvocation,spawnProductCli,type CliInput} from '../../scripts/cli-invocation.js';
import { projectTestClient } from '../helpers/project-client.js';
import type { UsageClient } from '@wombat/client';
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { realpathSync } from 'node:fs';
import { mkdtemp, mkdir, writeFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { spawn, spawnSync } from 'node:child_process';
import { once } from 'node:events';
import { setTimeout as delay } from 'node:timers/promises';
import type { UsageRequest } from '@wombat/client';
import { createNodeClient } from '@wombat/client/node';
test('six workflow signals share CLI, native and fixed-view evidence without raw commands', { timeout: 60000 }, async () => {
    const dir = realpathSync.native(await mkdtemp(path.join(tmpdir(), 'wombat-workflow-'))), root = path.join(dir, 'source'), data = path.join(dir, 'data'), project = path.join(dir, 'project');
    const keys = ['WOMBAT_DATA_HOME', 'CODEX_HOME', 'WOMBAT_AUTO_PRICES', 'WOMBAT_CORE_BIN'] as const;
    const saved = keys.map(k => [k, process.env[k]] as const);
    const binary = path.resolve('dist', process.platform === 'win32' ? 'wombat-core.exe' : 'wombat-core');
    let service: ReturnType<typeof spawn> | undefined, nativeEntry: UsageClient | undefined;
    try {
        await mkdir(path.join(root, 'sessions'), { recursive: true });
        for (const thread of ['a', 'b']) {
            const rows: unknown[] = [{ type: 'session_meta', payload: { id: thread, cwd: project } }, { type: 'turn_context', payload: { turn_id: 'u', model: 'gpt-5.4' } }];
            for (let i = thread === 'a' ? 0 : 25; i < (thread === 'a' ? 25 : 27); i++) {
                const day = i < 20 ? '2026-09-02' : '2026-09-09';
                rows.push({ type: 'event_msg', timestamp: `${day}T00:00:00Z`, payload: { type: 'item_completed', turn_id: 'u', item: { type: 'CommandExecution', id: `command-${i}`, source: 'agent', cwd: project, parsed_cmd: [], command: ['SYNTHETIC_PRIVATE_COMMAND', '--stable'], aggregated_output: 'SYNTHETIC_PRIVATE_OUTPUT', status: i < 20 || i >= 25 ? 'completed' : i < 23 ? 'failed' : 'declined', exit_code: i >= 20 && i < 23 ? 2 : 0, duration: { secs: i < 20 ? 3 : 30, nanos: 0 } } } });
            }
            rows.push(rows[2]); // Exact native replay remains one operation.
            await writeFile(path.join(root, 'sessions', `${thread}.jsonl`), rows.map(r => JSON.stringify(r) + '\n').join(''));
        }
        Object.assign(process.env, { WOMBAT_DATA_HOME: data, CODEX_HOME: root, WOMBAT_AUTO_PRICES: '0', WOMBAT_CORE_BIN: binary });
        service = spawn(binary, ['--serve-usage'], { env: process.env, stdio: 'ignore' });
        await once(service, 'spawn');
        const client = createNodeClient({ binaryPath: binary, automaticPrices: false, timeoutMs: 15000 });
        const deadline = Date.now() + 10000;
        while (!(await client.service!({ action: 'status' })).running) {
            assert.ok(Date.now() < deadline, 'core listener startup timed out');
            await delay(30);
        }
        const live = await client.live!({ query: { action: 'usage', roots: [root], scope: { allTime: true } }, mode: 'fresh' });
        const snapshotId = live.result.snapshotRef.snapshotId;
        nativeEntry = projectTestClient(client, [root], [project]);

        const nativeTransport = nativeEntry;
        const query = async (request: UsageRequest) => (await nativeTransport.live!({ query: request, mode: 'cached' })).result;
        assert.equal((await query({ action: 'usage', scope: { allTime: true } })).snapshotRef.snapshotId, snapshotId);
        const scope = { since: '2026-09-08', until: '2026-09-15', timezone: 'UTC', project };
        const result = await query({ action: 'investigate', snapshotId, scope });
        const a = result.inspection!.activity!;
        assert.equal(a.findingCount, 1);
        assert.deepEqual(a.findings[0].signals, ['repeated_slow_request', 'failure_spike', 'duration_spike', 'recurring_workflow', 'repeated_failure', 'repeated_rejection']);
        assert.equal(a.findings[0].current.operations, 7);
        assert.equal(a.findings[0].baseline!.operations, 20);
        assert.equal(a.findings[0].baseline!.medianDurationMs, 3000);
        assert.doesNotMatch(JSON.stringify(result), /SYNTHETIC_PRIVATE_/);
        const request: CliInput = queryInvocation({action:'investigate',snapshotId,scope:{project,since:scope.since,until:scope.until,timezone:'UTC'}},{mode:'cached'});
        const json = spawnProductCli(path.resolve('dist/wombat.js'), request, { encoding: 'utf8', env: process.env, timeout: 15000 });
        assert.ok([0, 2].includes(json.status!), json.stderr + json.stdout);
        assert.deepEqual(JSON.parse(json.stdout).inspection.activity, a);
        for (const proof of [...a.findings[0].evidence, ...a.findings[0].baselineEvidence]) {
            const steps = await query({ action: 'steps', snapshotId, threadId: proof.threadId, turnId: proof.turnId!, scope: proof.scope });
            assert.ok(steps.items.some(i => i.kind === 'operation' && i.id === proof.operationId));
        }
        const all = await query({ action: 'investigate', snapshotId, scope: { allTime: true, project } });
        assert.equal(all.inspection!.activity!.baselineScope, null);
        assert.ok(!all.inspection!.activity!.findings[0].signals.includes('failure_spike'));
    }
    finally {
        if (service && service.exitCode === null) {
            const closed = once(service, 'close');
            service.kill('SIGTERM');
            const kill = setTimeout(() => service?.kill('SIGKILL'), 2000);
            try {
                await closed;
            }
            finally {
                clearTimeout(kill);
            }
        }
        for (const [k, v] of saved) {
            if (v === undefined)
                delete process.env[k];
            else
                process.env[k] = v;
        }
        await rm(dir, { recursive: true, force: true, maxRetries: 10, retryDelay: 100 });
    }
});
