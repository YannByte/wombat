import { projectTestClient } from '../helpers/project-client.js';
import type { UsageClient } from '@wombat/client';
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { realpathSync } from 'node:fs';
import { mkdtemp, mkdir, rm, writeFile, readFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { spawn, spawnSync } from 'node:child_process';
import { once } from 'node:events';
import { DatabaseSync } from 'node:sqlite';
import { createNodeClient } from '@wombat/client/node';
test('index initialization faults return actual errors, retain unsupported records and recover in the same service', { timeout: 20000 }, async () => {
    const dir = realpathSync.native(await mkdtemp(path.join(tmpdir(), 'wombat-startup-fault-'))), root = path.join(dir, 'source'), project = path.join(dir, 'project'), data = path.join(dir, 'data'), database = path.join(data, 'live-v2/index.sqlite');
    await mkdir(path.join(root, 'sessions'), { recursive: true });
    await mkdir(path.dirname(database), { recursive: true });
    await mkdir(project);
    const instructions = path.join(project, 'AGENTS.md');
    await writeFile(instructions, 'Synthetic local instructions. '.repeat(1000));
    await writeFile(path.join(root, 'AGENTS.md'), 'Synthetic global instructions.');
    const unsupported = new DatabaseSync(database);
    unsupported.exec("CREATE TABLE sample(value TEXT); INSERT INTO sample VALUES('retained'); PRAGMA user_version=99;");
    unsupported.close();
    const previous = { WOMBAT_DATA_HOME: process.env.WOMBAT_DATA_HOME, CODEX_HOME: process.env.CODEX_HOME, WOMBAT_AUTO_PRICES: process.env.WOMBAT_AUTO_PRICES };
    Object.assign(process.env, { WOMBAT_DATA_HOME: data, CODEX_HOME: root, WOMBAT_AUTO_PRICES: '0' });
    const binary = path.resolve('dist', process.platform === 'win32' ? 'wombat-core.exe' : 'wombat-core');

    const client = createNodeClient({ binaryPath: binary, automaticPrices: false });
    const nativeEntry = projectTestClient(client, [root], [project]);
    try {
        await assert.rejects(client.live!({ query: { action: 'usage', roots: [root] }, mode: 'fresh' }), { code: 'INDEX_UNSUPPORTED_VERSION' });
        const direct = await client.live!({query:{action:'usage',roots:[root]}});
        assert.equal(direct.result.summary.measurementCount,0);

        const nativeApi = nativeEntry;
        const inventory = await nativeApi.config!({});
        const item = inventory.items.find(i => i.path === instructions)!;
        assert.ok(item);
        assert.equal(inventory.coverage.historyStatus, 'unavailable');
        assert.equal(item.measurementStatus, 'complete');
        assert.equal(item.bytes, Buffer.byteLength('Synthetic local instructions. '.repeat(1000)));
        const pending = await nativeApi.optimize!({ project });
        const suggestion = pending.suggestions.find(s => s.item.path === instructions)!;
        assert.ok(suggestion);
        assert.ok(suggestion.findings.some(f => f.rule === 'fileSize'));
        const checks = await nativeApi.optimize!({ project, action: 'checks', itemId: item.id });
        assert.equal(checks.checks.find(c => c.rule === 'fileSize')?.outcome, 'hit');
        assert.ok(!checks.checks.some(c => c.rule === 'runtimeDuplicateInjection'));
        await writeFile(instructions, 'Short synthetic instructions.');
        const recheck = await nativeApi.optimize!({ project, action: 'recheck', baseline:suggestion, baselineId: suggestion.baselineId });
        assert.ok(recheck.checks.some(c=>c.rule==='fileSize'&&c.outcome==='miss'&&c.comparison.status==='comparable'));
        assert.equal(recheck.decisionRevision,'0');
        const retained = new DatabaseSync(database);
        assert.equal(retained.prepare('SELECT count(*) AS n FROM sample').get()!.n, 1);
        assert.equal(retained.prepare('PRAGMA user_version').get()!.user_version, 99);
        retained.close();
        // Explicit repair uses the real CLI and publishes only the derived generation.
        const rebuilt = spawnSync(process.execPath, [path.resolve('dist/wombat.js'), 'usage', '--root', root, '--rebuild', '--json'], { env: { ...process.env, WOMBAT_CORE_BIN: binary }, encoding: 'utf8', timeout: 25000 });
        assert.ifError(rebuilt.error);
        assert.ok([0, 2].includes(rebuilt.status!), rebuilt.stdout + rebuilt.stderr);
        const restored = JSON.parse(rebuilt.stdout);
        assert.ok(['current', 'stale'].includes(restored.freshness.status));
        assert.equal(restored.summary.measurementCount, 0);
        const pointer = JSON.parse(await readFile(path.join(data, 'live-v2/active-index.json'), 'utf8'));
        const current = new DatabaseSync(path.join(data, 'live-v2', pointer.directory, 'index.sqlite'));
        assert.equal(current.prepare('PRAGMA user_version').get()!.user_version, 9);
        current.close();
        const original = new DatabaseSync(database, { readOnly: true });
        assert.equal(original.prepare('PRAGMA user_version').get()!.user_version, 99);
        assert.equal(original.prepare('SELECT value FROM sample').get()!.value, 'retained');
        original.close();
        assert.equal((await nativeApi.optimize!({ project })).decisionRevision,'0');
    }
    finally {


        for (const [k, v] of Object.entries(previous)) {
            if (v === undefined)
                delete process.env[k];
            else
                process.env[k] = v;
        }
        await rm(dir, { recursive: true, force: true });
    }
});
