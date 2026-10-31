import { test } from 'node:test';
import assert from 'node:assert/strict';
import { withLocalProduct, row, measurement } from '../helpers/local-product.ts';
import type { UsageRequest } from '@wombat/client';

test('statistics keep zero, missing totals, ties, unassigned facts and empty windows distinct', { timeout: 40_000 }, async () => {
  await withLocalProduct(async f => {
    const at = '2026-10-01T16:30:00Z';
    for (const [id, total, model] of [['zero', 0, 'gpt-5.4'], ['tie-a', 100, 'gpt-5.4'], ['tie-b', 100, 'gpt-6-sol'], ['partial', null, 'gpt-6-sol']] as const) {
      await f.write(id, [row('session_meta', { id, cwd: f.project }, at), row('turn_context', { turn_id: 'turn', model }, at), measurement(id, id, total, at)]);
    }
    // No session identity: these 50 tokens belong to the ledger, not a fabricated task.
    await f.write('unassigned', [measurement(null, 'unassigned', 50, at)]);
    const nativeApi = await f.native();
    const live = await nativeApi.live!({ query: { action: 'threads', scope: { allTime: true }, limit: 10 }, mode: 'fresh' });
    const snapshotId = live.result.snapshotRef.snapshotId;
    const query: UsageRequest = { action: 'statistics', snapshotId, scope: { allTime: true }, presentation: 'models', limit: 1 };
    const result = (await nativeApi.live!({ query, mode: 'cached' })).result;
    const stats = result.statistics!;
    assert.deepEqual([stats.population.measuredTasks, stats.population.completeTasks, stats.population.incompleteTasks, stats.population.completeTaskTokens], [4, 3, 1, 200]);
    assert.deepEqual([stats.population.meanTokens, stats.population.medianTokens, stats.population.p90Tokens], [200 / 3, 100, 100]);
    assert.equal(stats.population.unassignedUsage.tokens.total, 50);
    assert.equal(stats.groups.length, 1); assert.equal(result.page.total, 3);
    const second = (await nativeApi.live!({ query: { ...query, offset: 1 }, mode: 'cached' })).result;
    assert.deepEqual(second.statistics!.population, stats.population, 'Pagination must not change population facts');
    assert.notEqual(second.statistics!.groups[0].key, stats.groups[0].key);
    const tie = live.result.items.find(item => item.kind === 'thread' && item.threadUsage.tokens.total === 100)!;
    assert.ok(tie && tie.kind === 'thread');
    const typical = (await nativeApi.live!({ query: { ...query, threadId: tie.id }, mode: 'cached' })).result.statistics!.selectedTask!;
    assert.equal(typical.percentileRank, 2 / 3); assert.equal(typical.aboveP90, false);
    const zero = live.result.items.find(item => item.kind === 'thread' && item.threadUsage.tokens.total === 0);
    assert.ok(zero && zero.kind === 'thread');
    const zeroRank = (await nativeApi.live!({ query: { ...query, threadId: zero.id }, mode: 'cached' })).result.statistics!.selectedTask!;
    assert.equal(zeroRank.tokens, 0); assert.equal(zeroRank.percentileRank, 1 / 6); assert.equal(zeroRank.aboveP90, false);
    const partial = live.result.items.find(item => item.kind === 'thread' && item.threadUsage.tokens.total === null)!;
    assert.ok(partial && partial.kind === 'thread');
    const unknown = (await nativeApi.live!({ query: { ...query, threadId: partial.id }, mode: 'cached' })).result.statistics!.selectedTask!;
    assert.equal(unknown.tokens, null); assert.equal(unknown.percentileRank, null); assert.equal(unknown.aboveP90, null);
    const empty = (await nativeApi.live!({ query: { ...query, scope: { since: '2026-10-02', until: '2026-10-03', timezone: 'UTC' } }, mode: 'cached' })).result.statistics!;
    assert.deepEqual([empty.population.measuredTasks, empty.population.completeTaskTokens, empty.population.meanTokens, empty.population.medianTokens, empty.population.p90Tokens], [0, 0, null, null, null]);
    const localDay = (await nativeApi.live!({ query: { ...query, scope: { since: '2026-10-02', until: '2026-10-03', timezone: 'Asia/Shanghai' } }, mode: 'cached' })).result.statistics!;
    assert.deepEqual(localDay.population, stats.population, 'The UTC evening belongs to the following Shanghai calendar day');
    const growth = (await nativeApi.live!({ query: { ...query, scope: { since: '2026-10-01', until: '2026-10-02', timezone: 'UTC' }, comparison: { kind: 'periods', baselineSince: '2026-09-30', baselineUntil: '2026-10-01', dimension: 'thread' } }, mode: 'cached' })).result.statistics!.growth!;
    assert.equal(growth.taskCountDelta, 4); assert.equal(growth.taskCountContribution, null); assert.equal(growth.perTaskContribution, null);
    assert.equal(growth.attributedTokenDelta, null, 'Incomplete populations cannot claim an exact decomposition');
    await assert.rejects(nativeApi.live!({ query: { ...query, threadId: 'absent' }, mode: 'cached' }), { code: 'NOT_FOUND' });
    await assert.rejects(nativeApi.live!({ query: { ...query, scope: { allTime: true, threadId: tie.id }, threadId: tie.id }, mode: 'cached' }), { code: 'INVALID_ARGUMENT' });
    const invalid = JSON.parse(f.cli(['call'], JSON.stringify({ method: 'usage', params: { mode: 'cached', query: { ...query, presentation: 'details' } } }), 1));
    assert.equal(invalid.error.code, 'INVALID_ARGUMENT');
    await f.append('tie-a', [measurement('tie-a', 'added', 50, at)]);
    const fresh = (await nativeApi.live!({ query: { ...query, snapshotId: undefined }, mode: 'fresh' })).result;
    assert.equal(fresh.statistics!.population.completeTaskTokens, 250);
    const pinned = (await nativeApi.live!({ query, mode: 'cached' })).result;
    assert.deepEqual(pinned.statistics, stats, 'A later scan cannot rewrite an existing snapshot');
    const cli = JSON.parse(f.cli(['statistics', '--snapshot', snapshotId, '--all-time', '--presentation', 'models', '--limit', '1', '--json']));
    assert.deepEqual(cli.statistics, stats, 'CLI retains missing measurement fields in a successfully read source');
  });
});
