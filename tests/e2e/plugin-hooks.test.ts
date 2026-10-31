import { projectTestClient } from '../helpers/project-client.js';
import type { UsageClient } from '@wombat/client';
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { realpathSync } from 'node:fs';
import { mkdtemp, mkdir, writeFile, readFile, readdir, rm, access, symlink } from 'node:fs/promises';
import { spawn, spawnSync } from 'node:child_process';
import { once } from 'node:events';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { createNodeClient } from '@wombat/client/node';
import { nativeCodexFixture } from '../helpers/native-codex.js';
test('plugin Hook identity, project coverage, retained declarations and rechecks share the native binding', { timeout: 60000, skip: process.platform === 'win32' }, async () => {
    const dir = realpathSync.native(await mkdtemp(path.join(tmpdir(), 'wombat-plugin-hooks-')));
    const root = path.join(dir, 'source'), a = path.join(dir, 'a'), b = path.join(dir, 'b'), c = path.join(dir, 'c');
    // The product must use native-listed files rather than guessing the native plugin cache layout.
    const file = path.join(root, 'custom-package', 'hooks', '声明.json'), config = path.join(root, 'config.toml'), marker = path.join(dir, 'must-not-exist');
    await mkdir(path.join(root, 'sessions'), { recursive: true });
    await mkdir(path.dirname(file), { recursive: true });
    for (const project of [a, b, c])
        await mkdir(project);
    const enabled = '[plugins."sample.tools@test"]\nenabled=true\n';
    await writeFile(config, enabled);
    const command = "python3 './工具 script.py'";
    const text = JSON.stringify({ hooks: { SessionStart: [{ hooks: [{ type: 'command', command }, { type: 'command', command: 'UNLISTED_PRIVATE_COMMAND' }] }] } });
    await writeFile(file, text);
    const native = await nativeCodexFixture(dir);
    const metadata = { key: 'sample.tools@test:hooks/声明.json:session_start:0:0', pluginId: 'sample.tools@test', eventName: 'sessionStart', sourcePath: file, source: 'plugin', enabled: true, isManaged: false, currentHash: 'sha256:synthetic-plugin', trustStatus: 'trusted', handlerType: 'command', command, displayOrder: 0, timeoutSec: 600 };
    const hooks = { data: [{ cwd: a, hooks: [metadata], warnings: [], errors: [] }, { cwd: b, hooks: [{ ...metadata, enabled: false, trustStatus: 'untrusted' }], warnings: [], errors: [] }, { cwd: c, hooks: [], warnings: [], errors: [] }] };
    const setMode = (extra: object = {}) => writeFile(native.mode, JSON.stringify({ hooks, ...extra }));
    await setMode();
    const old = { WOMBAT_DATA_HOME: process.env.WOMBAT_DATA_HOME, CODEX_HOME: process.env.CODEX_HOME, WOMBAT_AUTO_PRICES: process.env.WOMBAT_AUTO_PRICES };
    Object.assign(process.env, { WOMBAT_DATA_HOME: path.join(dir, 'data'), CODEX_HOME: root, WOMBAT_AUTO_PRICES: '0' });
    const binary = path.resolve('dist/wombat-core'), service = spawn(binary, ['--serve-usage'], { stdio: 'ignore', env: process.env });
    await once(service, 'spawn');
    const client = createNodeClient({ binaryPath: binary, codexBinaryPath: native.binary, automaticPrices: false });
    const nativeEntry = projectTestClient(client, [root], [a, b, c]);
    try {

        const nativeTransport = nativeEntry;
        const first = await nativeTransport.config!({ kind: 'hook' });
        assert.equal(first.hookRegistry.status, 'observed');
        assert.equal(first.items.length, 1);
        const item = first.items[0];
        assert.deepEqual(item.authorizedProjects, [a, b]);
        assert.ok(item.sourceContexts.every(s => !s.global));
        assert.equal(item.usageCount, null);
        assert.equal(item.nativeKey, '/hooks/SessionStart/0/hooks/0');
        assert.equal(first.hookRegistry.contexts[0].registrations[0].source, 'plugin');
        assert.equal(first.hookRegistry.contexts[0].registrations[0].pluginId, 'sample.tools@test');
        assert.equal(first.hookRegistry.contexts[0].registrations[0].nativeKey, metadata.key);
        assert.equal(first.hookRegistry.contexts[1].registrations[0].enabled, false);
        const args = [path.resolve('dist/wombat.js'), 'optimize', 'inventory', '--root', root, ...[a, b, c].flatMap(p => ['--project-root', p]), '--kind', 'hook', '--read-view', first.readView!];
        const cli = spawnSync(process.execPath, [...args, '--json'], { env: process.env, encoding: 'utf8', timeout: 15000 });
        assert.ok(cli.status === 0 || cli.status === 2, cli.stderr);
        assert.deepEqual(JSON.parse(cli.stdout).hookRegistry, first.hookRegistry);
        const cliText = spawnSync(process.execPath, [...args, '--lang', 'en'], { env: process.env, encoding: 'utf8', timeout: 15000 });
        assert.ok(cliText.status === 0 || cliText.status === 2, cliText.stderr);
        assert.match(cliText.stdout, /Plugin: sample.tools@test/);
        assert.equal((await nativeTransport.config!({ kind: 'hook', readView: first.readView, scope: { project: c } })).items.length, 0);
        assert.equal((await nativeTransport.config!({ kind: 'hook' })).configRevision, first.configRevision);
        const suggestions = await nativeTransport.optimize!({ action: 'list', project: a });
        const original = suggestions.suggestions.find(s => s.item.id === item.id)!;
        assert.ok(original);
        assert.equal(original.findings[0].rule, 'hookTarget');
        assert.equal(original.findings[0].evidence?.hook?.nativeKey, metadata.key);
        const preview = await nativeTransport.handoff!({ action: 'preview', readView: suggestions.readView, project: a, suggestionIds: [original.id] });
        assert.deepEqual(preview.projects[0].targets.find(t => t.itemId === item.id)!.sharedProjects, [a]);
        await nativeTransport.optimize!({ action: 'keep', suggestionId: original.id, decisionReason: 'necessary', readView: suggestions.readView, decisionRevision: preview.decisionRevision, project: a });
        await setMode({ hooksError: true });
        const unavailable = await nativeTransport.config!({ kind: 'hook' });
        assert.equal(unavailable.hookRegistry.status, 'unavailable');
        assert.equal(unavailable.items[0].current, true);
        assert.equal(unavailable.items[0].stale, true);
        const uncertain = await nativeTransport.optimize!({ action: 'recheck', group: 'history', suggestionId: original.id, project: a });
        assert.equal(uncertain.suggestions.find(s => s.id === original.id)?.status, 'recheckUnavailable');
        await setMode();
        await writeFile(path.join(a, '工具 script.py'), `from pathlib import Path\nPath(${JSON.stringify(marker)}).touch()\n`);
        const resolved = await nativeTransport.optimize!({ action: 'recheck', group: 'history', suggestionId: original.id, project: a });
        assert.equal(resolved.suggestions.find(s => s.id === original.id)?.status, 'verified');
        assert.equal(resolved.suggestions.find(s => s.id === original.id)?.decision?.kind, 'keep');
        for (const change of [{ pluginId: null }, { pluginId: 'other@test' }, { key: 'sample.tools@test:hooks/../hooks/声明.json:session_start:0:0' }, { key: 'sample.tools@test:wrong/声明.json:session_start:0:0' }]) {
            await setMode({ hooks: { data: [{ ...hooks.data[0], hooks: [{ ...metadata, ...change }] }] } });
            const bad = await nativeTransport.config!({ kind: 'hook' });
            assert.equal(bad.hookRegistry.status, 'partial');
            assert.ok(bad.hookRegistry.contexts.every(context => !context.registrations.length));
        }
        await setMode({ hooksAfter: { data: hooks.data.map(context => ({ ...context, hooks: context.hooks.map(h => ({ ...h, pluginId: 'other@test' })) })) } });
        assert.equal((await nativeTransport.config!({ kind: 'hook' })).hookRegistry.status, 'unavailable');
        await setMode({ mutateOnHookList: file, mutatedHookText: text.replace('工具 script', 'changed') });
        assert.equal((await nativeTransport.config!({ kind: 'hook' })).hookRegistry.status, 'partial');
        await writeFile(file, text);
        await setMode({ hooks: { data: [a, b, c].map(cwd => ({ cwd, hooks: [], warnings: [], errors: [] })) } });
        const omitted = await nativeTransport.config!({ kind: 'hook' });
        assert.equal(omitted.hookRegistry.status, 'observed');
        assert.equal(omitted.items[0].current, true);
        assert.equal(omitted.items[0].stale, true);
        await writeFile(config, enabled.replace('true', 'false'));
        const before = await readFile(native.calls, 'utf8');
        assert.equal((await nativeTransport.config!({ kind: 'hook' })).hookRegistry.status, 'unavailable');
        assert.equal(await readFile(native.calls, 'utf8'), before, 'disabled plugins alone do not launch the native host');
        await writeFile(config, enabled);
        const outside = path.join(dir, 'outside.json');
        await writeFile(outside, text);
        await setMode({ hooks: { data: [{ ...hooks.data[0], hooks: [{ ...metadata, sourcePath: outside, key: 'sample.tools@test:outside.json:session_start:0:0' }] }] } });
        const bounded = await nativeTransport.config!({ kind: 'hook' });
        assert.equal(bounded.hookRegistry.status, 'partial');
        assert.ok(!JSON.stringify(bounded.hookRegistry).includes(outside));
        await rm(file);
        await symlink(outside, file);
        await setMode();
        const linked = await nativeTransport.config!({ kind: 'hook' });
        assert.equal(linked.hookRegistry.status, 'partial');
        assert.ok(linked.hookRegistry.contexts.every(context => !context.registrations.length));
        await rm(file);
        await setMode();
        const removed = await nativeTransport.config!({ kind: 'hook' });
        assert.equal(removed.items[0].current, false);
        assert.equal(removed.items[0].stale, false);
        const cacheDir = path.join(dir, 'data/config-v2');
        for (const entry of await readdir(cacheDir)) {
            const cache = await readFile(path.join(cacheDir, entry), 'utf8');
            assert.ok(!cache.includes('UNLISTED_PRIVATE_COMMAND'));
            assert.ok(!cache.includes(command));
            assert.ok(!cache.includes('registrationHash'));
        }
        await assert.rejects(access(marker));
        const methods = (await readFile(native.calls, 'utf8')).trim().split('\n').map(l => JSON.parse(l).method);
        assert.ok(methods.every(m => ['initialize', 'hooks/list', 'skills/list', 'config/read', 'account/read', 'account/rateLimits/read'].includes(m)), JSON.stringify(methods));
        assert.ok(!methods.some(m => m.startsWith('thread/')), 'preview and registry reads must not create or run a task');
    }
    finally {
        service.kill('SIGTERM');
        await once(service, 'exit').catch(() => { });
        await native.waitForExit();
        for (const [k, v] of Object.entries(old)) {
            if (v === undefined)
                delete process.env[k];
            else
                process.env[k] = v;
        }
        await rm(dir, { recursive: true, force: true });
    }
});
