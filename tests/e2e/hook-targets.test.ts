import {comparable} from '../helpers/comparable.js';
import {spawnProductCli} from '../../scripts/cli-invocation.js';
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
test('Hook script findings, project scope, independent rechecks share version-bound evidence', { timeout: 60000, skip: process.platform === 'win32' }, async () => {
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
    const old = { WOMBAT_CODEX_BIN:process.env.WOMBAT_CODEX_BIN, WOMBAT_DATA_HOME: process.env.WOMBAT_DATA_HOME, CODEX_HOME: process.env.CODEX_HOME, WOMBAT_AUTO_PRICES: process.env.WOMBAT_AUTO_PRICES };
    Object.assign(process.env, { WOMBAT_DATA_HOME: path.join(dir, 'data'), CODEX_HOME: root, WOMBAT_AUTO_PRICES: '0', WOMBAT_CODEX_BIN:native.binary });
    const binary = path.resolve('dist', process.platform === 'win32' ? 'wombat-core.exe' : 'wombat-core');


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
        const checksA = await nativeTransport.optimize!({ action: 'checks', itemId: original.item.id, project: a });
        assert.equal(checksA.checks[0].outcome, 'hit');
        const checksB = await nativeTransport.optimize!({ action: 'checks', itemId: original.item.id, project: b });
        assert.equal(checksB.checks[0].outcome, 'miss');
        assert.equal((await nativeTransport.optimize!({ action: 'list', project: b })).pending, 0);
        const cli = spawnProductCli(path.resolve('dist/wombat.js'), {method:'optimize',params:{action:'list',roots:[root],projectRoots:[a,b]}}, { env: process.env, encoding: 'utf8', timeout: 15000 });
        assert.equal(cli.status, 2, cli.stderr);
        assert.deepEqual(comparable(JSON.parse(cli.stdout).suggestions), comparable(first.suggestions));
        await nativeTransport.optimize!({ action: 'keep', suggestionId: original.id, decisionReason: 'necessary', decisionRevision: first.decisionRevision });
        await writeFile(path.join(a, filename), script);
        await rm(path.join(b, filename));
        const rechecked = await nativeTransport.optimize!({ action: 'recheck', baseline:original,baselineId: original.baselineId });
        assert.ok(rechecked.checks.some(c=>c.rule==='hookTarget'&&c.outcome==='miss'&&c.comparison.status==='comparable'));
        const current = await nativeTransport.optimize!({ action: 'list' });
        assert.equal(current.pending, 1);
        assert.equal(current.suggestions[0].findings[0].evidence?.hook?.project, b);
        const second = current.suggestions[0];
        assert.notEqual(second.id, original.id);
        omitted = true;
        await update();
        const unknown = await nativeTransport.optimize!({ action: 'recheck', baseline:second,baselineId: second.baselineId, suggestionId: second.id });
        assert.ok(unknown.checks.some(c=>c.rule==='hookTarget'&&c.outcome==='insufficient'));
        omitted = false;
        enabled = false;
        await update();
        const disabled = await nativeTransport.optimize!({ action: 'recheck', baseline:second,baselineId: second.baselineId, suggestionId: second.id });
        assert.ok(disabled.checks.some(c=>c.rule==='hookTarget'&&c.outcome==='miss'));
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


        for (const [k, v] of Object.entries(old)) {
            if (v === undefined)
                delete process.env[k];
            else
                process.env[k] = v;
        }
        await rm(dir, { recursive: true, force: true });
    }
});
test('native project inheritance is observed per view and unavailable registration retains scoped evidence', { timeout: 45000, skip: process.platform === 'win32' }, async () => {
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
    const old = { WOMBAT_CODEX_BIN:process.env.WOMBAT_CODEX_BIN, WOMBAT_DATA_HOME: process.env.WOMBAT_DATA_HOME, CODEX_HOME: process.env.CODEX_HOME, WOMBAT_AUTO_PRICES: process.env.WOMBAT_AUTO_PRICES };
    Object.assign(process.env, { WOMBAT_DATA_HOME: path.join(dir, 'data'), CODEX_HOME: root, WOMBAT_AUTO_PRICES: '0', WOMBAT_CODEX_BIN:native.binary });
    const binary = path.resolve('dist', process.platform === 'win32' ? 'wombat-core.exe' : 'wombat-core');

    const client = createNodeClient({ binaryPath: binary, codexBinaryPath: native.binary, automaticPrices: false }), scope = { roots: [root], projectRoots: [parent, child], project: child };
    try {
        const first = await client.optimize!({ ...scope, action: 'list' }), finding = first.suggestions.find(s => s.findings.some(f => f.rule === 'hookTarget'))!;
        assert.ok(finding);
        assert.equal(finding.findings[0].evidence?.hook?.project, child);
        const inventory = await client.config!({ roots: [root], projectRoots: [parent, child], kind: 'hook', scope: { project: child } });
        assert.equal(inventory.items[0].id, finding.item.id);
        assert.equal(finding.item.measurementStatus, 'declarationUnavailable');
        await client.optimize!({ ...scope, action: 'keep', suggestionId: finding.id, decisionReason: 'necessary', decisionRevision: first.decisionRevision });
        await writeFile(native.mode, JSON.stringify({ hooksError: true }));
        const unavailable = await client.optimize!({ ...scope, action: 'recheck', baseline:finding,baselineId: finding.baselineId, suggestionId: finding.id });
        assert.ok(unavailable.checks.some(c=>c.rule==='hookTarget'&&c.outcome==='insufficient'));
        await writeFile(path.join(child, 'local.py'), 'MUST_NOT_EXECUTE');
        await writeFile(native.mode, JSON.stringify(good));
        const repaired = await client.optimize!({ ...scope, action: 'recheck', baseline:finding,baselineId: finding.baselineId, suggestionId: finding.id });
        assert.ok(repaired.checks.some(c=>c.rule==='hookTarget'&&c.outcome==='miss'&&c.comparison.status==='comparable'));
    }
    finally {


        for (const [k, v] of Object.entries(old)) {
            if (v === undefined)
                delete process.env[k];
            else
                process.env[k] = v;
        }
        await rm(dir, { recursive: true, force: true });
    }
});
