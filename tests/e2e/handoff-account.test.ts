import { projectTestClient } from '../helpers/project-client.js';
import type { UsageClient } from '@wombat/client';
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { realpathSync } from 'node:fs';
import { mkdtemp, mkdir, writeFile, rm } from 'node:fs/promises';
import path from 'node:path';
import { tmpdir } from 'node:os';
import { spawn } from 'node:child_process';
import { once } from 'node:events';
import { createNodeClient } from '@wombat/client/node';
const binary = path.resolve('dist', process.platform === 'win32' ? 'wombat-core.exe' : 'wombat-core');
test('handoff reviews all pending files, merges shared targets and refuses changed content before Codex starts', { timeout: 45000 }, async () => {
    const dir = realpathSync.native(await mkdtemp(path.join(tmpdir(), 'wombat-handoff-'))), root = path.join(dir, 'source'), a = path.join(dir, 'a'), b = path.join(dir, 'b');
    const previous = { WOMBAT_DATA_HOME: process.env.WOMBAT_DATA_HOME, CODEX_HOME: process.env.CODEX_HOME, WOMBAT_AUTO_PRICES: process.env.WOMBAT_AUTO_PRICES };
    Object.assign(process.env, { WOMBAT_DATA_HOME: path.join(dir, 'data'), CODEX_HOME: root, WOMBAT_AUTO_PRICES: '0' });
    let service: ReturnType<typeof spawn> | undefined, nativeEntry: UsageClient | undefined;
    try {
        await mkdir(path.join(root, 'sessions'), { recursive: true });
        await writeFile(path.join(root, 'AGENTS.md'), 'x'.repeat(16385));
        for (const project of [a, b]) {
            await mkdir(project);
            await writeFile(path.join(project, 'AGENTS.md'), 'Synthetic project instructions.\n');
            for (let i = 0; i < 40; i++) {
                const skill = path.join(project, `.agents/skills/test-${i}`);
                await mkdir(skill, { recursive: true });
                await writeFile(path.join(skill, 'SKILL.md'), '---\ndescription: Synthetic missing-name skill\n---\nBody.\n');
            }
        }
        service = spawn(binary, ['--serve-usage'], { stdio: 'ignore' });
        await once(service, 'spawn');
        nativeEntry = projectTestClient(createNodeClient({ binaryPath: binary, codexBinaryPath: path.join(dir, 'absent-codex'), automaticPrices: false }), [root], [a, b]);

        const nativeApi = nativeEntry;
        const displayed = await nativeApi.optimize!({ action: 'list', category: 'repair', limit: 3 });
        assert.equal(displayed.suggestions.length, 3);
        const preview = await nativeApi.handoff!({ action: 'preview', readView: displayed.readView, decisionRevision: displayed.decisionRevision });
        assert.equal(preview.projects.length, 2);
        const targets = preview.projects.flatMap(p => p.targets);
        assert.equal(targets.length, 81);
        const shared = targets.filter(t => t.path === path.join(root, 'AGENTS.md'));
        assert.equal(shared.length, 1);
        assert.deepEqual(shared[0].sharedProjects, [a, b]);
        // Selection identities belong to their displayed scope. Changing projects requires a fresh review.
        await assert.rejects(nativeApi.handoff!({ action: 'preview', readView: preview.readView, decisionRevision: preview.decisionRevision, suggestionIds: [shared[0].suggestionIds[0]], project: b }), { code: 'VIEW_EXPIRED' });
        const scoped = await nativeApi.handoff!({ action: 'preview', project: b });
        const scopedShared = scoped.projects.flatMap(p => p.targets).find(t => t.path === path.join(root, 'AGENTS.md'))!;
        const single = await nativeApi.handoff!({ action: 'preview', readView: scoped.readView, decisionRevision: scoped.decisionRevision, suggestionIds: [scopedShared.suggestionIds[0]], project: b });
        assert.equal(single.projects.length, 1);
        assert.equal(single.projects[0].cwd, b);
        assert.equal(single.projects[0].targets.length, 1);
        await assert.rejects(nativeApi.handoff!({ action: 'send', project: b, readView: single.readView, decisionRevision: single.decisionRevision, suggestionIds: [scopedShared.suggestionIds[0]], selectionVersion: single.selectionVersion }), { code: 'CODEX_UNAVAILABLE' });
        assert.equal((await nativeApi.optimize!({ action: 'list', project: b })).pending, (await nativeApi.optimize!({ action: 'list', project: b, readView: single.readView })).pending);
        await assert.rejects(nativeApi.handoff!({ action: 'preview', suggestionIds: [] }), { code: 'INVALID_ARGUMENT' });
        await writeFile(path.join(root, 'AGENTS.md'), 'x'.repeat(16386));
        await assert.rejects(nativeApi.handoff!({ action: 'send', readView: preview.readView, decisionRevision: preview.decisionRevision, selectionVersion: preview.selectionVersion }), { code: 'VIEW_EXPIRED' });
    }
    finally {
        if (service && service.exitCode === null && service.signalCode === null) {
            const ended = once(service, 'exit');
            service.kill();
            await ended;
        }
        for (const [k, v] of Object.entries(previous)) {
            if (v === undefined)
                delete process.env[k];
            else
                process.env[k] = v;
        }
        await rm(dir, { recursive: true, force: true });
    }
});
test('account failures retain old section timestamps and a changed account clears previous windows', { timeout: 20000 }, async () => {
    const dir = await mkdtemp(path.join(tmpdir(), 'wombat-account-')), fixture = path.join(dir, 'codex.cjs'), mode = path.join(dir, 'mode.json');
    const previousDataHome = process.env.WOMBAT_DATA_HOME;
    process.env.WOMBAT_DATA_HOME = path.join(dir, 'data');
    try {
        await writeFile(fixture, `const {readFileSync}=require('node:fs'); const mode=JSON.parse(readFileSync(${JSON.stringify(mode)},'utf8')); if(process.argv.includes('--version')){console.log('codex-cli 0.160.0');process.exit(0);} let buffer=''; process.stdin.on('data',chunk=>{buffer+=chunk;let end;while((end=buffer.indexOf('\\n'))>=0){const text=buffer.slice(0,end);buffer=buffer.slice(end+1);const m=JSON.parse(text);if(m.id==null)continue;let result={};let error=false;if(m.method==='account/read') result={requiresOpenaiAuth:true,account:{type:'chatgpt',email:'synthetic@example.invalid',planType:'pro'},workspaceRouting:{chatgptAccountId:mode.account}};if(m.method==='account/rateLimits/read'){error=mode.fail;result={accountId:mode.account,rateLimitsByLimitId:{synthetic:{primary:{usedPercent:12,windowDurationMins:37},credits:{balance:'12.500000001',hasCredits:true,unlimited:false},individualLimit:{limit:'20',used:'0',remainingPercent:100,resetsAt:1800000000}}},rateLimitResetCredits:{availableCount:2,credits:null},ordinaryUsageAllowed:true};}if(m.method==='account/usage/read'){error=mode.fail;result={summary:{lifetimeTokens:0,longestRunningTurnSec:123}};}console.log(JSON.stringify(error?{id:m.id,error:{code:-1,message:'Synthetic private error'}}:{id:m.id,result}));}});`);
        await writeFile(mode, JSON.stringify({ account: 'account-a', fail: false }));
        const client = createNodeClient({ binaryPath: binary, codexBinaryPath: fixture, automaticPrices: false });
        const first = await client.account!({ action: 'read' });
        assert.equal(first.windows.length, 1);
        assert.equal(first.windows[0].durationMinutes, 37);
        assert.equal(first.summary?.longestRunningTurnSeconds, 123);
        assert.equal(first.summary?.lifetimeTokens, 0);
        assert.equal(first.identity?.maskedEmail, 'sy***@example.invalid');
        assert.equal(first.buckets[0].credits?.balance, '12.500000001');
        assert.equal(first.resetCredits?.availableCount, 2);
        const stored = await client.account!({ action: 'history' });
        assert.equal(stored.history?.totalObservations, 1);
        assert.equal(stored.history?.observations[0].accountId, first.identity?.id);
        assert.equal(stored.history?.observations[0].windows[0].usedPercent, 12);
        await writeFile(mode, JSON.stringify({ account: 'account-a', fail: true }));
        const stale = await client.account!({ action: 'refresh' });
        assert.equal(stale.allowance.status, 'stale');
        assert.equal(stale.allowance.checkedAt, first.allowance.checkedAt);
        assert.equal(stale.activity.checkedAt, first.activity.checkedAt);
        assert.equal(stale.windows.length, 1);
        assert.equal(stale.ordinaryUsageAllowed, null);
        assert.equal(stale.buckets[0].credits?.balance, '12.500000001');
        assert.equal(stale.buckets[0].status, 'stale');
        assert.equal(stale.buckets[0].individualLimit?.status, 'stale');
        assert.equal(stale.resetCredits?.availableCount, 2);
        assert.ok(!JSON.stringify(stale).includes('Synthetic private error'));
        await writeFile(mode, JSON.stringify({ account: 'account-b', fail: true }));
        const switched = await client.account!({ action: 'refresh' });
        assert.notEqual(switched.identity?.id, first.identity?.id);
        assert.equal(switched.windows.length, 0);
        assert.equal(switched.buckets.length, 0);
        assert.equal(switched.resetCredits, null);
        assert.equal(switched.summary, null);
        assert.equal(switched.allowance.status, 'unavailable');
        const history = await client.account!({ action: 'history' });
        assert.equal(history.history?.totalObservations, 3);
        assert.equal(history.history?.observations[0].accountId, switched.identity?.id);
        assert.equal(history.history?.observations[1].status, 'unavailable');
        assert.equal(history.history?.observations[1].windows.length, 0);
    }
    finally {
        if (previousDataHome === undefined)
            delete process.env.WOMBAT_DATA_HOME;
        else
            process.env.WOMBAT_DATA_HOME = previousDataHome;
        await rm(dir, { recursive: true, force: true });
    }
});
