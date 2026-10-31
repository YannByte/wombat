import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdir, writeFile, readdir, readFile } from 'node:fs/promises';
import { setTimeout as delay } from 'node:timers/promises';
import path from 'node:path';
import { withLocalProduct, row, measurement } from '../helpers/local-product.ts';
import { rfc3339Nanos } from '../helpers/rfc3339.ts';

test('extension activity reports full filtered counts before pagination and separates unknown all-time duration', { timeout: 40_000 }, async () => {
  await withLocalProduct(async f => {
    const day = (offset: number) => new Date(Date.now() + offset * 86_400_000).toISOString().slice(0, 10);
    const at = day(-3) + 'T00:00:00Z';
    const skill = (name: string) => path.join(f.project, '.agents/skills', name, 'SKILL.md');
    for (const name of ['used', 'idle']) {
      await mkdir(path.dirname(skill(name)), { recursive: true });
      await writeFile(skill(name), `---\nname: ${name}\ndescription: Synthetic extension activity fixture.\n---\nSynthetic instructions.\n`);
    }
    await writeFile(path.join(f.root, 'config.toml'), '[mcp_servers.docs]\ncommand="DO_NOT_EXECUTE"\n');
    await f.write('task', [row('session_meta', { id: 'task', cwd: f.project }, at), row('turn_context', { turn_id: 'turn', model: 'gpt-5.4' }, at), row('response_item', { type: 'function_call', call_id: 'used', name: 'read_file', arguments: JSON.stringify({ path: skill('used') }) }, at), row('response_item', { type: 'function_call_output', call_id: 'used', output: 'Synthetic instructions.' }, at)]);
    const nativeApi = await f.native();
    const scope = { project: f.project, since: day(-4), until: day(1), timezone: 'UTC' };
    // Automatic first reads may return a syncing preview; final counts need a completed view.
    const usage = await nativeApi.live!({ query: { action: 'usage', scope }, mode: 'fresh' });
    assert.ok(['current','stale'].includes(usage.freshness.status));
    assert.equal(usage.freshness.initialScan, false);
    const full = await nativeApi.config!({ action: 'list', snapshotId: usage.result.snapshotRef.snapshotId, scope, kinds: ['skill', 'mcp'], sort: 'name' });
    assert.equal(full.items.length, 3);
    assert.deepEqual([full.extensionActivity!.observedUse, full.extensionActivity!.noObservedUse, full.extensionActivity!.unavailable], [1, 2, 0]);
    const used = full.items.find(item => item.name === 'used')!, idle = full.items.find(item => item.name === 'idle')!;
    const usedActivity = full.extensionActivity!.items.find(item => item.itemId === used.id)!;
    const idleActivity = full.extensionActivity!.items.find(item => item.itemId === idle.id)!;
    assert.equal(usedActivity.observedRecords, 1); assert.equal(usedActivity.noObservedUseDays, 3);
    assert.equal(idleActivity.observedRecords, 0); assert.equal(idleActivity.noObservedUseDays, 4);
    for (const item of full.extensionActivity!.items) assert.equal(item.absenceObservable, false);
    const request = { action: 'list' as const, readView: full.readView, scope, kinds: ['skill' as const, 'mcp' as const], sort: 'name' as const, limit: 1, offset: 1 };
    const page = await nativeApi.config!(request);
    assert.equal(page.items.length, 1); assert.equal(page.extensionActivity!.items.length, 1);
    assert.deepEqual([page.extensionActivity!.observedUse, page.extensionActivity!.noObservedUse, page.extensionActivity!.unavailable], [1, 2, 0]);
    const mismatched = JSON.parse(f.cli(['call'], JSON.stringify({ method: 'config', params: request }), 1));
    assert.equal(mismatched.error.code, 'INVALID_ARGUMENT', 'A pinned read view requires the same explicit source and project roots');
    const cli = JSON.parse(f.cli(['call'], JSON.stringify({ method: 'config', params: { ...request, roots: [f.root], projectRoots: [f.project] } }), 2));
    assert.equal(cli.coverage.status, 'partial', 'Configured native integration remains unknown in this isolated fixture');
    assert.deepEqual(cli.extensionActivity, page.extensionActivity);
    const allTime = await nativeApi.config!({ action: 'list', readView: full.readView, scope: { project: f.project, allTime: true }, kinds: ['skill'] });
    const unknownStart = allTime.extensionActivity!.items.find(item => item.itemId === idle.id)!;
    assert.equal(unknownStart.observedRecords, 0); assert.equal(unknownStart.noObservedUseDays, null, 'No all-time window start can be invented');
  });
});
