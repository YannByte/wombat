import { projectTestClient } from '../helpers/project-client.js';
import type { UsageClient } from '@wombat/client';
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { realpathSync } from 'node:fs';
import { mkdtemp, mkdir, writeFile, readFile, rm } from 'node:fs/promises';
import { spawn } from 'node:child_process';
import { once } from 'node:events';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { createNodeClient } from '@wombat/client/node';
import { nativeCodexFixture } from '../helpers/native-codex.js';
test('native Hook registry is version-bound, project-specific and private across HTTP and core queries', { timeout: 60000 }, async () => {
    const dir = realpathSync.native(await mkdtemp(path.join(tmpdir(), 'wombat-hook-registry-')));
    const root = path.join(dir, 'source'), a = path.join(dir, 'a'), b = path.join(dir, 'b'), config = path.join(root, 'config.toml');
    const old = { WOMBAT_DATA_HOME: process.env.WOMBAT_DATA_HOME, CODEX_HOME: process.env.CODEX_HOME, WOMBAT_AUTO_PRICES: process.env.WOMBAT_AUTO_PRICES };
    await mkdir(path.join(root, 'sessions'), { recursive: true });
    await mkdir(a);
    await mkdir(b);
    const text = "[[hooks.SessionStart]]\n[[hooks.SessionStart.hooks]]\ntype='command'\ncommand='echo SYNTHETIC_PRIVATE_COMMAND'\n";
    await writeFile(config, text);
    const native = await nativeCodexFixture(dir);
    const metadata = { key: `${config}:session_start:0:0`, eventName: 'sessionStart', sourcePath: config, source: 'user', enabled: true, isManaged: false, currentHash: 'sha256:synthetic', trustStatus: 'trusted', handlerType: 'command', command: 'echo SYNTHETIC_PRIVATE_COMMAND', displayOrder: 0, timeoutSec: 600 };
    const hooks = { data: [{ cwd: a, hooks: [metadata], warnings: [], errors: [] }, { cwd: b, hooks: [{ ...metadata, enabled: false, trustStatus: 'untrusted' }], warnings: [], errors: [] }] };
    const setMode = (extra: object = {}) => writeFile(native.mode, JSON.stringify({ hooks, ...extra }));
    await setMode();
    Object.assign(process.env, { WOMBAT_DATA_HOME: path.join(dir, 'data'), CODEX_HOME: root, WOMBAT_AUTO_PRICES: '0' });
    const binary = path.resolve('dist', process.platform === 'win32' ? 'wombat-core.exe' : 'wombat-core');
    const service = spawn(binary, ['--serve-usage'], { stdio: 'ignore', env: process.env });
    await once(service, 'spawn');
    const client = createNodeClient({ binaryPath: binary, codexBinaryPath: native.binary, automaticPrices: false });
    const nativeEntry = projectTestClient(client, [root], [a, b]);
    try {

        const nativeTransport = nativeEntry;
        const first = await nativeTransport.config!({ kind: 'hook' });
        assert.equal(first.hookRegistry.status, 'observed');
        assert.equal(first.hookRegistry.contexts.length, 2);
        const registration = first.hookRegistry.contexts[0].registrations[0];
        assert.equal(registration.itemId, first.items[0].id);
        assert.equal(registration.contentHash, first.items[0].contentHash);
        assert.equal(registration.enabled, true);
        assert.equal(registration.trust, 'trusted');
        assert.equal(first.hookRegistry.contexts[1].registrations[0].enabled, false);
        assert.equal(first.hookRegistry.contexts[1].registrations[0].trust, 'untrusted');
        assert.equal(first.items[0].configuredState, 'declared');
        assert.equal(first.items[0].usageCount, null);
        assert.ok(!JSON.stringify(first).includes('SYNTHETIC_PRIVATE_COMMAND'));
        const next = await nativeTransport.config!({ kind: 'hook' });
        assert.equal(first.configRevision, next.configRevision, 'observation time must not invalidate an unchanged review');
        await setMode({ hooksError: true });
        const calls = await readFile(native.calls, 'utf8');
        const pinned = await nativeTransport.config!({ kind: 'hook', readView: first.readView });
        assert.deepEqual(pinned.hookRegistry, first.hookRegistry);
        assert.equal(await readFile(native.calls, 'utf8'), calls);
        const unavailable = await nativeTransport.config!({ kind: 'hook' });
        assert.equal(unavailable.hookRegistry.status, 'unavailable');
        const scoped = await nativeTransport.config!({ kind: 'hook', readView: first.readView, scope: { project: b } });
        assert.deepEqual(scoped.hookRegistry.contexts.map(c => c.project), [b]);
        await setMode({ hooksAfter: { data: hooks.data.map(c => ({ ...c, hooks: c.hooks.map(h => ({ ...h, enabled: !h.enabled })) })) } });
        assert.equal((await nativeTransport.config!({ kind: 'hook' })).hookRegistry.status, 'unavailable', 'registry changed during capture');
        await setMode({ mutateOnHookList: config, mutatedHookText: text.replace('PRIVATE', 'CHANGED') });
        const changed = await nativeTransport.config!({ kind: 'hook' });
        assert.equal(changed.hookRegistry.status, 'partial');
        assert.ok(changed.hookRegistry.contexts.every(c => !c.registrations.length));
        await writeFile(config, text);
        for (const version of ['0.160.1', '0.162.0-alpha.2', '999.0.0']) {
            await setMode({ version });
            const supported = await nativeTransport.config!({ kind: 'hook' });
            assert.equal(supported.hookRegistry.status, 'observed', 'validated evidence must not depend on a native version allowlist');
            assert.equal(supported.hookRegistry.nativeVersion, version);
            assert.equal(supported.hookRegistry.contexts[0].registrations.length, 1);
            await setMode({ version, hooks: { data: [{ ...hooks.data[0], hooks: [{ ...metadata, trustStatus: 'unknown-state' }] }] } });
            const invalid = await nativeTransport.config!({ kind: 'hook' });
            assert.equal(invalid.hookRegistry.status, 'partial');
            assert.ok(invalid.hookRegistry.contexts.every(c => !c.registrations.length));
        }
        await setMode({ hooks: { data: [{ ...hooks.data[0], hooks: Array.from({ length: 513 }, () => metadata) }] } });
        assert.equal((await nativeTransport.config!({ kind: 'hook' })).hookRegistry.status, 'unavailable');
        await setMode({ hooks: { data: [{ ...hooks.data[0], hooks: [metadata, { ...metadata, enabled: false }] }] } });
        const ambiguous = await nativeTransport.config!({ kind: 'hook' });
        assert.equal(ambiguous.hookRegistry.status, 'partial');
        assert.ok(ambiguous.hookRegistry.contexts.every(c => !c.registrations.length), 'ambiguous keys must not retain the first state');
        const outside = path.join(dir, 'outside.toml');
        await writeFile(outside, 'DO_NOT_READ');
        await setMode({ hooks: { data: [{ ...hooks.data[0], hooks: [{ ...metadata, sourcePath: outside }] }] } });
        const bounded = await nativeTransport.config!({ kind: 'hook' });
        assert.equal(bounded.hookRegistry.status, 'partial');
        assert.ok(!JSON.stringify(bounded.hookRegistry).includes(outside));
        await assert.rejects(nativeTransport.config!({ kind: 'hook', nativeHooks: {} } as never));
        const methods = (await readFile(native.calls, 'utf8')).trim().split('\n').map(l => JSON.parse(l).method);
        assert.ok(methods.every(m => ['initialize', 'hooks/list'].includes(m)), 'registry observation must never create or run a task');
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
