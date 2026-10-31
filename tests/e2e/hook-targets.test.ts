import { projectTestClient } from '../helpers/project-client.js';
import type { UsageClient } from '@wombat/client';
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { realpathSync } from 'node:fs';
import { mkdtemp, mkdir, writeFile, readFile, rm, access } from 'node:fs/promises';
import { spawn, spawnSync } from 'node:child_process';
import { once } from 'node:events';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { createNodeClient } from '@wombat/client/node';
import { nativeCodexFixture } from '../helpers/native-codex.js';
test('Hook script findings, project scope, independent rechecks and handoff share version-bound evidence', { timeout: 60000, skip: process.platform === 'win32' }, async () => {
    const dir = realpathSync.native(await mkdtemp(path.join(tmpdir(), 'wombat-hook-target-'))), root = path.join(dir, 'source'), a = path.join(dir, 'a'), b = path.join(dir, 'b');
    await mkdir(path.join(root, 'sessions'), { recursive: true });
    await mkdir(a);
    await mkdir(b);
    const config = path.join(root, 'config.toml'), marker = path.join(dir, 'must-not-exist'), filename = '工具 script.py';
    const script = `from pathlib import Path\nPath(${JSON.stringify(marker)}).write_text('executed')\n`;
    await writeFile(path.join(root, 'AGENTS.md'), 'Synthetic instruction.');
    await writeFile(path.join(a, 'AGENTS.md'), 'Synthetic instruction.');
    await writeFile(path.join(b, 'AGENTS.md'), 'Synthetic instruction.');
    await writeFile(path.join(b, filename), script);
    const native = await nativeCodexFixture(dir);
    let command = `python3 -B './${filename}'`, enabled = true, trust = 'trusted', omitted = false;
    const update = async () => {
        await writeFile(config, `[[hooks.SessionStart]]\n[[hooks.SessionStart.hooks]]\ntype='command'\ncommand=${JSON.stringify(command)}\n`);
        const hook = { key: `${config}:session_start:0:0`, eventName: 'sessionStart', sourcePath: config, source: 'user', enabled, isManaged: false, currentHash: 'sha256:synthetic-current', trustStatus: trust, handlerType: 'command', command, displayOrder: 0, timeoutSec: 600 };
        await writeFile(native.mode, JSON.stringify({ hooks: { data: [a, b].map(cwd => ({ cwd, hooks: omitted ? [] : [hook], warnings: [], errors: [] })) } }));
    };
    await update();
    const old = { WOMBAT_DATA_HOME: process.env.WOMBAT_DATA_HOME, CODEX_HOME: process.env.CODEX_HOME, WOMBAT_AUTO_PRICES: process.env.WOMBAT_AUTO_PRICES };
    Object.assign(process.env, { WOMBAT_DATA_HOME: path.join(dir, 'data'), CODEX_HOME: root, WOMBAT_AUTO_PRICES: '0' });
    const binary = path.resolve('dist', process.platform === 'win32' ? 'wombat-core.exe' : 'wombat-core');
    const service = spawn(binary, ['--serve-usage'], { stdio: 'ignore', env: process.env });
    await once(service, 'spawn');
    const nativeEntry = projectTestClient(createNodeClient({ binaryPath: binary, codexBinaryPath: native.binary, automaticPrices: false }), [root], [a, b]);
    try {

        const nativeTransport = nativeEntry;
        const first = await nativeTransport.optimize!({ action: 'list' });
        assert.equal(first.pending, 1);
        const original = first.suggestions[0];
        assert.equal(original.category, 'repair');
        assert.equal(original.findings[0].rule, 'hookTarget');
        assert.equal(original.findings[0].evidence?.hook?.project, a);
        assert.equal(original.findings[0].evidence?.hook?.target, './' + filename);
        assert.ok(!JSON.stringify(original).includes('Path('));
        const checksA = await nativeTransport.optimize!({ action: 'checks', readView: first.readView, itemId: original.item.id, project: a });
        assert.equal(checksA.checks[0].outcome, 'hit');
        const checksB = await nativeTransport.optimize!({ action: 'checks', readView: first.readView, itemId: original.item.id, project: b });
        assert.equal(checksB.checks[0].outcome, 'miss');
        assert.equal((await nativeTransport.optimize!({ action: 'list', readView: first.readView, project: b })).pending, 0);
        const cli = spawnSync(process.execPath, [path.resolve('dist/wombat.js'), 'optimize', 'list', '--root', root, '--project-root', a, '--project-root', b, '--read-view', first.readView!, '--json'], { env: process.env, encoding: 'utf8', timeout: 15000 });
        assert.equal(cli.status, 2, cli.stderr);
        assert.deepEqual(JSON.parse(cli.stdout).suggestions, first.suggestions);
        const handoff = await nativeTransport.handoff!({ action: 'preview', readView: first.readView });
        assert.equal(handoff.projects[0].cwd, a);
        assert.deepEqual(handoff.projects[0].targets[0].sharedProjects, [a]);
        await nativeTransport.optimize!({ action: 'keep', suggestionId: original.id, decisionReason: 'necessary', readView: first.readView, decisionRevision: handoff.decisionRevision });
        await writeFile(path.join(a, filename), script);
        await rm(path.join(b, filename));
        const rechecked = await nativeTransport.optimize!({ action: 'recheck', group: 'history' });
        const resolved = rechecked.suggestions.find(s => s.id === original.id)!;
        assert.equal(resolved.status, 'verified');
        assert.equal(resolved.decision?.kind, 'keep');
        const current = await nativeTransport.optimize!({ action: 'list' });
        assert.equal(current.pending, 1);
        assert.equal(current.suggestions[0].findings[0].evidence?.hook?.project, b);
        const second = current.suggestions[0];
        assert.notEqual(second.id, original.id);
        omitted = true;
        await update();
        const unknown = await nativeTransport.optimize!({ action: 'recheck', group: 'history', suggestionId: second.id });
        assert.equal(unknown.suggestions.find(s => s.id === second.id)!.status, 'recheckUnavailable');
        omitted = false;
        enabled = false;
        await update();
        const disabled = await nativeTransport.optimize!({ action: 'recheck', group: 'history', suggestionId: second.id });
        assert.equal(disabled.suggestions.find(s => s.id === second.id)!.status, 'verified');
        enabled = true;
        trust = 'untrusted';
        await update();
        let checks = await nativeTransport.optimize!({ action: 'checks', itemId: original.item.id });
        assert.equal(checks.checks[0].outcome, 'insufficient');
        trust = 'trusted';
        command = 'python3 $FILE';
        await update();
        checks = await nativeTransport.optimize!({ action: 'checks', itemId: original.item.id });
        assert.equal(checks.checks[0].outcome, 'insufficient');
        command = 'node ./entry.js';
        await update();
        await writeFile(path.join(a, 'entry.js.js'), 'MUST_NOT_EXECUTE');
        checks = await nativeTransport.optimize!({ action: 'checks', itemId: original.item.id, project: a });
        assert.equal(checks.checks[0].outcome, 'insufficient');
        await assert.rejects(access(marker));
        assert.ok((await readFile(native.calls, 'utf8')).trim().split('\n').every(l => !JSON.parse(l).method.startsWith('thread/')));
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
test('native project inheritance is observed per view and unavailable registration retains scoped history', { timeout: 45000, skip: process.platform === 'win32' }, async () => {
    const dir = realpathSync.native(await mkdtemp(path.join(tmpdir(), 'wombat-hook-inherited-'))), root = path.join(dir, 'source'), parent = path.join(dir, 'parent'), child = path.join(parent, 'child'), config = path.join(parent, '.codex/config.toml');
    await mkdir(path.join(root, 'sessions'), { recursive: true });
    await mkdir(path.dirname(config), { recursive: true });
    await mkdir(child);
    await writeFile(config, "[[hooks.SessionStart]]\n[[hooks.SessionStart.hooks]]\ntype='command'\ncommand='python3 ./local.py'\n[hooks.SessionStart.hooks.extra]\nvalue='synthetic'\n");
    await writeFile(path.join(parent, 'local.py'), 'MUST_NOT_EXECUTE');
    const native = await nativeCodexFixture(dir);
    const metadata = { key: `${config}:session_start:0:0`, eventName: 'sessionStart', sourcePath: config, source: 'project', enabled: true, isManaged: false, currentHash: 'sha256:synthetic', trustStatus: 'trusted', handlerType: 'command', command: 'python3 ./local.py', displayOrder: 0, timeoutSec: 600 };
    const good = { hooks: { data: [parent, child].map(cwd => ({ cwd, hooks: [metadata], warnings: [], errors: [] })) } };
    await writeFile(native.mode, JSON.stringify(good));
    const old = { WOMBAT_DATA_HOME: process.env.WOMBAT_DATA_HOME, CODEX_HOME: process.env.CODEX_HOME, WOMBAT_AUTO_PRICES: process.env.WOMBAT_AUTO_PRICES };
    Object.assign(process.env, { WOMBAT_DATA_HOME: path.join(dir, 'data'), CODEX_HOME: root, WOMBAT_AUTO_PRICES: '0' });
    const binary = path.resolve('dist', process.platform === 'win32' ? 'wombat-core.exe' : 'wombat-core'), service = spawn(binary, ['--serve-usage'], { stdio: 'ignore', env: process.env });
    await once(service, 'spawn');
    const client = createNodeClient({ binaryPath: binary, codexBinaryPath: native.binary, automaticPrices: false }), scope = { roots: [root], projectRoots: [parent, child], project: child };
    try {
        const first = await client.optimize!({ ...scope, action: 'list' }), finding = first.suggestions.find(s => s.findings.some(f => f.rule === 'hookTarget'))!;
        assert.ok(finding);
        assert.equal(finding.findings[0].evidence?.hook?.project, child);
        const inventory = await client.config!({ roots: [root], projectRoots: [parent, child], readView: first.readView, kind: 'hook', scope: { project: child } });
        assert.equal(inventory.items[0].id, finding.item.id);
        assert.equal(finding.item.measurementStatus, 'declarationUnavailable');
        const preview = await client.handoff!({ ...scope, action: 'preview', readView: first.readView });
        assert.equal(preview.projects[0].cwd, child);
        assert.deepEqual(preview.projects[0].targets.find(t => t.itemId === finding.item.id)!.sharedProjects, [child]);
        await client.optimize!({ ...scope, action: 'keep', suggestionId: finding.id, decisionReason: 'necessary', readView: first.readView, decisionRevision: first.decisionRevision });
        await writeFile(native.mode, JSON.stringify({ hooksError: true }));
        const unavailable = await client.optimize!({ ...scope, action: 'recheck', group: 'history', suggestionId: finding.id });
        const oldFinding = unavailable.suggestions.find(s => s.id === finding.id)!;
        assert.equal(oldFinding.status, 'recheckUnavailable');
        assert.equal(oldFinding.decision?.kind, 'keep');
        await writeFile(path.join(child, 'local.py'), 'MUST_NOT_EXECUTE');
        await writeFile(native.mode, JSON.stringify(good));
        const repaired = await client.optimize!({ ...scope, action: 'recheck', group: 'history', suggestionId: finding.id });
        assert.equal(repaired.suggestions.find(s => s.id === finding.id)?.status, 'verified');
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
