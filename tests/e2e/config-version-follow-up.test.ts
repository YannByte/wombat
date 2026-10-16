import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdir, writeFile, readdir, readFile } from 'node:fs/promises';
import { setTimeout as delay } from 'node:timers/promises';
import path from 'node:path';
import { withLocalProduct, row, measurement } from '../helpers/local-product.ts';
import { rfc3339Nanos } from '../helpers/rfc3339.ts';

test('follow-up confirms exact read and loaded versions while preserving review history and private content', { timeout: 40_000 }, async () => {
  await withLocalProduct(async f => {
    const file = path.join(f.project, 'AGENTS.md'), initial = new Date().toISOString();
    await writeFile(file, 'Synthetic oversized instructions. '.repeat(1_000));
    await writeFile(path.join(f.root, 'AGENTS.md'), 'Synthetic global instructions.');
    await f.write('task', [row('session_meta', { id: 'task', cwd: f.project }, initial), row('turn_context', { turn_id: 'turn', model: 'gpt-5.4' }, initial)]);
    const browser = await f.browser();
    const listed = await browser.optimize!({ project: f.project });
    const suggestion = listed.suggestions.find(s => s.item.path === file)!; assert.ok(suggestion);
    const checkedBody = 'SYNTHETIC_PRIVATE_CHECKED_RULE_VERSION';
    await writeFile(file, checkedBody);
    await browser.optimize!({ action: 'recheck', project: f.project, suggestionId: suggestion.id });
    const history = async () => {
      // Automatic reads can return a syncing preview. Final follow-up assertions
      // need a completed usage revision and the configuration view bound to it.
      const usage = await browser.live!({ query: { action: 'usage', scope: { allTime: true } }, mode: 'fresh' });
      assert.equal(usage.freshness.status, 'current');
      assert.equal(usage.freshness.initialScan, false);
      const config = await browser.config!({ action: 'list', snapshotId: usage.result.snapshotRef.snapshotId });
      assert.ok(config.readView);
      assert.equal(config.usageRevision, usage.result.snapshotRef.snapshotId);
      return browser.optimize!({ group: 'history', project: f.project, suggestionId: suggestion.id, readView: config.readView });
    };
    const baseline = await history();
    const observation = baseline.followUps[0]; assert.equal(observation.status, 'no_observed_records');
    const before = new Date(Date.parse(observation.after) - 1).toISOString();
    await delay(10); const readAt = new Date().toISOString();
    const read = [row('response_item', { type: 'function_call', call_id: 'read', name: 'read_file', arguments: JSON.stringify({ path: file }) }, readAt), row('response_item', { type: 'function_call_output', call_id: 'read', output: checkedBody }, readAt)];
    await f.append('task', [measurement('task', 'before', 30, before), measurement('task', 'after', 80, readAt), ...read, ...read]);
    // A different project in the same source cannot enter this follow-up comparison.
    await f.write('other', [row('session_meta', { id: 'other', cwd: f.dir }, initial), row('turn_context', { turn_id: 'turn', model: 'gpt-5.4' }, initial), measurement('other', 'foreign-project', 999, readAt)]);
    const readResult = await history();
    assert.equal(readResult.followUps[0].status, 'read_content_matched');
    assert.equal(readResult.followUps[0].matchingReadRecords, 1, 'Replayed native reads count once');
    assert.equal(readResult.followUps[0].matchingLoadRecords, 0);
    const compared = readResult.followUps[0].usageComparison!; assert.ok(compared);
    assert.equal(compared.baselineUsage.tokens.total, 30); assert.equal(compared.currentUsage.tokens.total, 80);
    // Core cutoffs can retain nanoseconds; truncating to microseconds changes interval lengths.
    assert.equal(rfc3339Nanos(compared.changeAt) - rfc3339Nanos(compared.baselineStart), rfc3339Nanos(compared.observedThrough) - rfc3339Nanos(compared.changeAt));
    assert.equal(compared.partial, false);
    await writeFile(file, 'Synthetic later current version.');
    assert.equal((await history()).followUps[0].status, 'read_content_matched', 'Follow-up binds the saved recheck version even after another edit');
    await delay(5); const loadedAt = new Date().toISOString();
    const load = row('response_item', { type: 'message', id: 'loaded', role: 'user', content: [{ type: 'input_text', text: `# AGENTS.md instructions for ${f.project}\n\n<INSTRUCTIONS>${checkedBody}</INSTRUCTIONS>` }], internal_chat_message_metadata_passthrough: { turn_id: 'turn', content_item_kinds: ['agents_md.instructions'] } }, loadedAt);
    await f.append('task', [load, load]);
    const loaded = await history();
    assert.equal(loaded.followUps[0].status, 'loaded_content_matched'); assert.equal(loaded.followUps[0].matchingLoadRecords, 1); assert.equal(loaded.followUps[0].matchingReadRecords, 1);
    assert.equal(loaded.followUps[0].after, observation.after); assert.equal(loaded.followUps[0].recordId, observation.recordId);
    assert.equal(loaded.decisionRevision, baseline.decisionRevision); assert.equal(loaded.history, baseline.history);
    const pinned = await browser.optimize!({ group: 'history', project: f.project, suggestionId: suggestion.id, readView: baseline.readView });
    assert.equal(pinned.followUps[0].status, 'no_observed_records');
    await f.stop(); await f.start();
    const restored = await history(); assert.equal(restored.followUps[0].status, 'loaded_content_matched');
    assert.equal(restored.followUps[0].recordId, observation.recordId);
    const cli = JSON.parse(f.cli(['optimize', 'history', '--root', f.root, '--project-root', f.project, '--project', f.project, '--suggestion', suggestion.id, '--read-view', restored.readView!, '--json'], '', 2));
    assert.equal(cli.followUps[0].status, 'loaded_content_matched'); assert.equal(cli.followUps[0].matchingReadRecords, 1); assert.equal(cli.followUps[0].matchingLoadRecords, 1);
    assert.ok(!JSON.stringify([loaded, cli]).includes(checkedBody));
    // Synthetic source retains its literal text; derived stores must retain only safe observations.
    const privateStores = async (dir: string): Promise<void> => {
      for (const entry of await readdir(dir, { withFileTypes: true })) {
        const full = path.join(dir, entry.name);
        if (entry.isDirectory()) await privateStores(full);
        else assert.ok(!(await readFile(full)).includes(Buffer.from(checkedBody)), `Private body leaked into ${entry.name}`);
      }
    };
    await f.stop(); await privateStores(f.data);
  });
});

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
    const browser = await f.browser();
    const scope = { project: f.project, since: day(-4), until: day(1), timezone: 'UTC' };
    // Automatic first reads may return a syncing preview; final counts need a completed view.
    const usage = await browser.live!({ query: { action: 'usage', scope }, mode: 'fresh' });
    assert.equal(usage.freshness.status, 'current');
    assert.equal(usage.freshness.initialScan, false);
    const full = await browser.config!({ action: 'list', snapshotId: usage.result.snapshotRef.snapshotId, scope, kinds: ['skill', 'mcp'], sort: 'name' });
    assert.equal(full.items.length, 3);
    assert.deepEqual([full.extensionActivity!.observedUse, full.extensionActivity!.noObservedUse, full.extensionActivity!.unavailable], [1, 2, 0]);
    const used = full.items.find(item => item.name === 'used')!, idle = full.items.find(item => item.name === 'idle')!;
    const usedActivity = full.extensionActivity!.items.find(item => item.itemId === used.id)!;
    const idleActivity = full.extensionActivity!.items.find(item => item.itemId === idle.id)!;
    assert.equal(usedActivity.observedRecords, 1); assert.equal(usedActivity.noObservedUseDays, 3);
    assert.equal(idleActivity.observedRecords, 0); assert.equal(idleActivity.noObservedUseDays, 4);
    for (const item of full.extensionActivity!.items) assert.equal(item.absenceObservable, false);
    const request = { action: 'list' as const, readView: full.readView, scope, kinds: ['skill' as const, 'mcp' as const], sort: 'name' as const, limit: 1, offset: 1 };
    const page = await browser.config!(request);
    assert.equal(page.items.length, 1); assert.equal(page.extensionActivity!.items.length, 1);
    assert.deepEqual([page.extensionActivity!.observedUse, page.extensionActivity!.noObservedUse, page.extensionActivity!.unavailable], [1, 2, 0]);
    const mismatched = JSON.parse(f.cli(['call'], JSON.stringify({ method: 'config', params: request }), 1));
    assert.equal(mismatched.error.code, 'INVALID_ARGUMENT', 'A pinned read view requires the same explicit source and project roots');
    const cli = JSON.parse(f.cli(['call'], JSON.stringify({ method: 'config', params: { ...request, roots: [f.root], projectRoots: [f.project] } }), 2));
    assert.equal(cli.coverage.status, 'partial', 'Configured native integration remains unknown in this isolated fixture');
    assert.deepEqual(cli.extensionActivity, page.extensionActivity);
    const allTime = await browser.config!({ action: 'list', readView: full.readView, scope: { project: f.project, allTime: true }, kinds: ['skill'] });
    const unknownStart = allTime.extensionActivity!.items.find(item => item.itemId === idle.id)!;
    assert.equal(unknownStart.observedRecords, 0); assert.equal(unknownStart.noObservedUseDays, null, 'No all-time window start can be invented');
  });
});
