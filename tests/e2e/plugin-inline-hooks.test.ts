import { projectTestClient } from '../helpers/project-client.js';
import type { UsageClient } from '@wombat/client';
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { realpathSync } from 'node:fs';
import { mkdtemp, mkdir, writeFile, readFile, readdir, rm, access } from 'node:fs/promises';
import { spawn, spawnSync } from 'node:child_process';
import { once } from 'node:events';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { createNodeClient } from '@wombat/client/node';
import { nativeCodexFixture } from '../helpers/native-codex.js';
for (const array of [false, true])
    test(`inline plugin Hook ${array ? 'array' : 'object'} uses native-expanded project commands and current evidence`, { timeout: 60000, skip: process.platform === 'win32' }, async () => {
        const dir = realpathSync.native(await mkdtemp(path.join(tmpdir(), 'wombat-plugin-inline-')));
        const root = path.join(dir, 'source'), a = path.join(dir, 'a'), b = path.join(dir, 'b');
        const file = path.join(root, 'arbitrary-package', '.codex-plugin', 'plugin.json'), marker = path.join(dir, 'must-not-exist');
        await mkdir(path.join(root, 'sessions'), { recursive: true });
        await mkdir(path.dirname(file), { recursive: true });
        for (const project of [a, b])
            await mkdir(project);
        await writeFile(path.join(root, 'config.toml'), '[plugins."sample@test"]\nenabled=true\n');
        const command = 'python3 "${PLUGIN_ROOT}/工具 script.py" --private PRIVATE_ARG';
        const doc = { hooks: { SessionStart: [{ hooks: [{ type: 'command', command }, { type: 'command', command: 'python3 "${UNRESOLVED}/secret.py"' }, { type: 'command', command: 'UNLISTED_PRIVATE' }] }] } };
        const text = JSON.stringify({ name: 'sample', hooks: array ? [{ hooks: { Stop: [] } }, doc, doc] : doc });
        await writeFile(file, text);
        await writeFile(path.join(b, '工具 script.py'), `from pathlib import Path\nPath(${JSON.stringify(marker)}).touch()\n`);
        const native = await nativeCodexFixture(dir);
        const base = { pluginId: 'sample@test', eventName: 'sessionStart', sourcePath: file, source: 'plugin', enabled: true, isManaged: false, trustStatus: 'trusted', handlerType: 'command', timeoutSec: 600 };
        const key = (n: number) => `sample@test:plugin.json#hooks[${array ? 1 : 0}]:session_start:0:${n}`;
        const hooks = { data: [a, b].map(cwd => ({ cwd, hooks: [
                    { ...base, key: key(0), currentHash: `sha256:synthetic-${cwd === a ? 'a' : 'b'}`, command: `python3 "${path.join(cwd, '工具 script.py')}" --private PRIVATE_ARG`, displayOrder: 0 },
                    { ...base, key: key(1), currentHash: 'sha256:unresolved', command: 'python3 "${UNRESOLVED}/secret.py"', displayOrder: 1 },
                ], warnings: [], errors: [] })) };
        const setMode = (extra: object = {}) => writeFile(native.mode, JSON.stringify({ hooks, ...extra }));
        await setMode();
        const old = { WOMBAT_DATA_HOME: process.env.WOMBAT_DATA_HOME, CODEX_HOME: process.env.CODEX_HOME, WOMBAT_AUTO_PRICES: process.env.WOMBAT_AUTO_PRICES };
        Object.assign(process.env, { WOMBAT_DATA_HOME: path.join(dir, 'data'), CODEX_HOME: root, WOMBAT_AUTO_PRICES: '0' });
        const binary = path.resolve('dist/wombat-core'), service = spawn(binary, ['--serve-usage'], { stdio: 'ignore', env: process.env });
        await once(service, 'spawn');
        const client = createNodeClient({ binaryPath: binary, codexBinaryPath: native.binary, automaticPrices: false });
        const nativeEntry = projectTestClient(client, [root], [a, b]);
        try {

            const nativeTransport = nativeEntry;
            const inventory = await nativeTransport.config!({ kind: 'hook' });
            assert.equal(inventory.hookRegistry.status, 'observed');
            assert.equal(inventory.items.length, 2);
            const pointer = `${array ? '/hooks/1/hooks' : '/hooks/hooks'}/SessionStart/0/hooks/0`;
            const item = inventory.items.find(i => i.nativeKey === pointer)!;
            assert.ok(item);
            assert.equal(item.bytes, Buffer.byteLength(JSON.stringify(doc.hooks.SessionStart[0].hooks[0])));
            assert.deepEqual(item.authorizedProjects, [a, b]);
            assert.equal(item.usageCount, null);
            assert.ok(inventory.hookRegistry.contexts.every(c => c.registrations.length === 2 && c.registrations.every(r => r.pluginId === 'sample@test')));
            for (const privateText of ['PRIVATE_ARG', 'UNLISTED_PRIVATE', 'UNRESOLVED'])
                assert.ok(!JSON.stringify(inventory).includes(privateText));
            const cli = spawnSync(process.execPath, [path.resolve('dist/wombat.js'), 'optimize', 'inventory', '--root', root, ...[a, b].flatMap(p => ['--project-root', p]), '--kind', 'hook', '--read-view', inventory.readView!, '--json'], { env: process.env, encoding: 'utf8', timeout: 15000 });
            assert.ok(cli.status === 0 || cli.status === 2, cli.stderr);
            assert.deepEqual(JSON.parse(cli.stdout).hookRegistry, inventory.hookRegistry);
            const suggestions = await nativeTransport.optimize!({ action: 'list', project: a });
            const suggestion = suggestions.suggestions.find(s => s.item.id === item.id)!;
            assert.ok(suggestion);
            const finding = suggestion.findings.find(f => f.rule === 'hookTarget')!;
            assert.ok(finding);
            assert.equal(finding.evidence?.hook?.target, path.join(a, '工具 script.py'));
            assert.equal(finding.evidence?.hook?.nativeKey, key(0));
            assert.ok(!(await nativeTransport.optimize!({ action: 'list', project: b })).suggestions.some(s => s.findings.some(f => f.rule === 'hookTarget')));
            await nativeTransport.optimize!({ action: 'keep', suggestionId: suggestion.id, decisionReason: 'necessary', readView: suggestions.readView, decisionRevision: suggestions.decisionRevision, project: a });
            await writeFile(path.join(a, '工具 script.py'), `from pathlib import Path\nPath(${JSON.stringify(marker)}).touch()\n`);
            const resolved = await nativeTransport.optimize!({ action: 'recheck', readView: suggestions.readView, baselineId: suggestion.baselineId, suggestionId: suggestion.id, project: a });
            assert.ok(resolved.checks.some(c=>c.rule==='hookTarget'&&c.outcome==='miss'));
            await setMode({ hooks: { data: hooks.data.map(c => ({ ...c, hooks: c.hooks.map(h => ({ ...h, command: 'python3 "${UNKNOWN}/tool.py"' })) })) } });
            const uncertain = await nativeTransport.optimize!({ action: 'recheck', readView: suggestions.readView, baselineId: suggestion.baselineId, suggestionId: suggestion.id, project: a });
            assert.ok(uncertain.checks.some(c=>c.rule==='hookTarget'&&c.outcome==='insufficient'));
            await setMode({ hooksAfter: { data: hooks.data.map(c => ({ ...c, hooks: c.hooks.map(h => ({ ...h, command: 'python3 changed.py' })) })) } });
            assert.equal((await nativeTransport.config!({ kind: 'hook' })).hookRegistry.status, 'unavailable');
            for (const entry of await readdir(path.join(dir, 'data/config-v2'))) {
                const cache = await readFile(path.join(dir, 'data/config-v2', entry), 'utf8');
                for (const privateText of ['PRIVATE_ARG', 'UNLISTED_PRIVATE', 'UNRESOLVED', 'registrationHash'])
                    assert.ok(!cache.includes(privateText));
            }
            await assert.rejects(access(marker));
            const methods = (await readFile(native.calls, 'utf8')).trim().split('\n').map(l => JSON.parse(l).method);
            assert.ok(methods.every(m => ['initialize', 'hooks/list', 'skills/list', 'config/read', 'account/read', 'account/rateLimits/read'].includes(m)));
            assert.ok(!methods.some(m => m.startsWith('thread/')));
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
