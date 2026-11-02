import {spawnProductCli,type CliInput} from '../../scripts/cli-invocation.js';
import { projectTestClient } from '../helpers/project-client.js';
import type { UsageClient } from '@wombat/client';
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { realpathSync } from 'node:fs';
import { mkdtemp, mkdir, writeFile, readFile, rm } from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import { spawn, spawnSync } from 'node:child_process';
import { once } from 'node:events';
import { createNodeClient } from '@wombat/client/node';
test('task statistics and budgets share exact facts across Agent CLI and native calls', { timeout: 60000 }, async () => {
    const dir = realpathSync.native(await mkdtemp(path.join(os.tmpdir(), 'wombat-statistics-'))), source = path.join(dir, 'source'), foreign = path.join(dir, 'foreign'), project = path.join(dir, 'project');
    for (const folder of [source, foreign])
        await mkdir(path.join(folder, 'sessions'), { recursive: true });
    await mkdir(project);
    const date = (offset: number) => new Date(Date.now() + offset * 86400000).toISOString().slice(0, 10), today = date(0), yesterday = date(-1), tomorrow = date(1);
    const write = async (root: string, id: string, amounts: [
        string,
        number
    ][]) => {
        const records = [{ type: 'session_meta', payload: { id, cwd: project } }, { type: 'turn_context', payload: { turn_id: 'turn', model: 'gpt-5.4' } }, ...amounts.map(([day, total], n) => ({ timestamp: day + 'T00:01:00Z', type: 'event_msg', payload: { type: 'token_usage_record', thread_id: id, turn_id: 'turn', response_id: id + n, usage: { input_tokens: total, cached_input_tokens: 0, cache_creation_input_tokens: 0, output_tokens: 0, reasoning_output_tokens: 0, total_tokens: total } } }))];
        const file = path.join(root, 'sessions', id + '.jsonl'), body = records.map(r => JSON.stringify(r)).join('\n') + '\n';
        await writeFile(file, body);
        return { file, body };
    };
    const original = await Promise.all([write(source, 'a', [[yesterday, 100], [today, 100], [today, 50]]), write(source, 'b', [[yesterday, 100], [today, 0]]), write(source, 'c', [[today, 300]]), write(foreign, 'foreign', [[today, 999]])]);
    const old = { WOMBAT_DATA_HOME: process.env.WOMBAT_DATA_HOME, CODEX_HOME: process.env.CODEX_HOME, WOMBAT_AUTO_PRICES: process.env.WOMBAT_AUTO_PRICES, WOMBAT_CORE_BIN: process.env.WOMBAT_CORE_BIN };
    const binary = path.resolve('dist', process.platform === 'win32' ? 'wombat-core.exe' : 'wombat-core'), entry = path.resolve('dist/wombat.js');
    Object.assign(process.env, { WOMBAT_DATA_HOME: path.join(dir, 'data'), CODEX_HOME: source, WOMBAT_AUTO_PRICES: '0', WOMBAT_CORE_BIN: binary });


    const client = createNodeClient({ binaryPath: binary, automaticPrices: false });
    let nativeEntry: UsageClient | undefined;
    const run = (args: CliInput, input = '') => { const result = spawnProductCli(entry, args, { env: process.env, input, encoding: 'utf8', timeout: 15000, maxBuffer: 2 * 1024 * 1024 }); assert.ifError(result.error); assert.equal(result.status, 0, result.stderr); return JSON.parse(result.stdout); };
    try {
        const live = await client.live!({ query: { action: 'threads', roots: [source], scope: { allTime: true }, limit: 3 }, mode: 'fresh' }), snapshotId = live.result.snapshotRef.snapshotId;
        const scope = { project, since: today, until: tomorrow, timezone: 'UTC' }, comparison = { kind: 'periods' as const, baselineSince: yesterday, baselineUntil: today, dimension: 'thread' as const };
        const request = { action: 'statistics' as const, scope, comparison, presentation: 'models' as const, limit: 1 };
        const stats = (await client.live!({ query: request, mode: 'cached' })).result.statistics!;
        assert.deepEqual([stats.population.measuredTasks, stats.population.completeTasks, stats.population.completeTaskTokens, stats.population.meanTokens, stats.population.medianTokens, stats.population.p90Tokens], [3, 3, 450, 150, 150, 270]);
        assert.equal(stats.growth!.taskCountDelta, 1);
        assert.equal(stats.growth!.attributedTokenDelta, 250);
        assert.equal(stats.growth!.taskCountContribution! + stats.growth!.perTaskContribution!, 250);
        const selected = live.result.items.find(i => i.kind === 'thread' && i.threadUsage.tokens.total === 300);
        assert.ok(selected && selected.kind === 'thread');
        const typical = (await client.live!({ query: { ...request, threadId: selected.id }, mode: 'cached' })).result.statistics!.selectedTask!;
        assert.equal(typical.aboveP90, true);
        assert.equal(typical.percentileRank, 5 / 6);
        const agent = run(['system','call'], JSON.stringify({ method: 'usage', params: { mode: 'cached', query: request } }));
        assert.deepEqual(agent.result.statistics, stats);
        const human = run(["usage", "stats", '--project', project, '--since', today, '--until', tomorrow, '--by', 'model', '--limit', '1', '--json']);
        const populationOnly=(await client.live!({query:{...request,comparison:undefined},mode:'cached'})).result.statistics;
        assert.deepEqual(human.statistics, populationOnly);
        run({method:'monitor',params:{action:'upsert',plan:{id:'daily',enabled:true,period:'day',tokenLimit:Number('400'),review:true,scope:{project:project,timezone:'UTC'}}}});
        const first = run(['usage','alerts','--once','--id=daily','--root',source,'--json']);
        assert.equal(first.notifications.length, 2);
        assert.ok(first.notifications.some((n: {
            kind: string;
        }) => n.kind === 'budget_exceeded'));
        assert.equal(run(['usage','alerts','--once','--id','daily','--root',source,'--json']).notifications.length, 0);
        const foreignView = (await client.live!({ query: { action: 'usage', roots: [foreign], scope: { allTime: true } }, mode: 'fresh' })).result;
        await client.monitor!({ action: 'upsert', plan: { id: 'foreign', enabled: true, period: 'day', review: false, tokenLimit: 500, scope: { project, sourceInstanceId: foreignView.quality.sources[0].source.id, timezone: 'UTC' } } });
        await client.monitor!({ action: 'check', roots:[foreign], ids: ['foreign'] });
        nativeEntry = projectTestClient(client, [source], [project]);

        const nativeApi = nativeEntry;
        const webView = await nativeApi.live!({ query: { action: 'usage', scope: { allTime: true }, limit: 1 }, mode: 'fresh' });
        const webStats = await nativeApi.live!({ query: { ...request  }, mode: 'cached' });
        assert.deepEqual(webStats.result.statistics, stats);
        const listed = await nativeApi.monitor!({ action: 'list' });
        assert.equal(listed.plans.length, 2);
        assert.ok(listed.notifications.length >= 2);
        assert.ok(listed.plans.some(p=>p.id==='foreign'));
        await nativeApi.monitor!({ action: 'acknowledge', notificationId: first.notifications[0].id });
        assert.ok((await client.monitor!({ action: 'list' })).notifications.find(n => n.id === first.notifications[0].id)?.acknowledged);
        const plan = listed.plans.find(p=>p.id==='daily')!;
        await nativeApi.monitor!({ action: 'upsert', plan: { ...plan, enabled: false } });
        assert.equal((await nativeApi.monitor!({ action: 'check', ids: ['daily'] })).notifications.length, 0);
        for (const { file, body } of original)
            assert.equal(await readFile(file, 'utf8'), body);
        const extra = await Promise.all([1, 2, 3].map(async (n) => { const root = path.join(dir, 'extra-' + n); await mkdir(path.join(root, 'sessions'), { recursive: true }); await write(root, 'extra-' + n, [[today, 111]]); return root; }));
        const roots = [source, ...extra];
        const full = (await client.live!({ query: { action: 'usage', roots, scope: { allTime: true }, limit: 1 }, mode: 'fresh' })).result;
        const compact = (await client.live!({ query: { action: 'usage', roots, scope: { allTime: true }, compact: true, limit: 1 }, mode: 'cached' })).result;
        assert.equal(compact.quality.sources.length, 3);
        assert.equal(compact.quality.detailSummary?.omittedSources, 1);
        const omitted = full.quality.sources.find(source => !compact.quality.sources.some(retained => retained.source.id === source.source.id))!.source.id;
        await client.monitor!({ action: 'upsert', plan: { id: 'wide', enabled: true, period: 'day', review: false, tokenLimit: 100, scope: { project, timezone: 'UTC' } } });
        const wide = await client.monitor!({ action: 'check', roots, ids: ['wide'] });
        assert.equal(wide.notifications[0].sourceInstanceIds.length, 4, 'Compact display examples cannot become the authorization source set');
        assert.ok((await nativeApi.monitor!({action:'list'})).notifications.some(n=>n.planId==='wide'));
        await client.monitor!({ action: 'upsert', plan: { id: 'fourth', enabled: true, period: 'day', review: false, tokenLimit: 100, scope: { project, sourceInstanceId: omitted, timezone: 'UTC' } } });
        nativeEntry = projectTestClient(client, roots, [project]);

        const wider = nativeEntry;
        const pinned = await wider.live!({ query: { action: 'usage', scope: { allTime: true }, compact: true, limit: 1 }, mode: 'fresh' });
        const fourth = await wider.monitor!({ action: 'check', ids: ['fourth'] });
        assert.equal(fourth.notifications.length, 1);
        assert.deepEqual(fourth.notifications[0].sourceInstanceIds, [omitted]);
        const api = run(['system','api', '--json']);
        assert.ok(api.methods.includes('monitor'));
        assert.equal(run(['system','call'], JSON.stringify({ method: 'monitor', params: { action: 'list' } })).action, 'list');
    }
    finally {

        for (const [key, value] of Object.entries(old)) {
            if (value === undefined)
                delete process.env[key];
            else
                process.env[key] = value;
        }
        await rm(dir, { recursive: true, force: true });
    }
});
