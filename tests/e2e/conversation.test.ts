import assert from 'node:assert/strict';
import { test } from 'node:test';
import { readFile, rename, rm, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { performance } from 'node:perf_hooks';
import type { ConversationRequest, ConversationResult } from '@wombat/client';
import { withLocalProduct, row } from '../helpers/local-product.js';

test('real conversation chain covers no-usage activity, CLI, Web, continuation and source changes', { timeout: 120_000 }, async () => {
  await withLocalProduct(async f => {
    const text = 'PRIVATE_SYNTHETIC_BODY 中文🙂\n' + 'x'.repeat(2 * 1024 * 1024);
    const rows = [row('session_meta', { id: 'task', cwd: f.project }, '2026-10-14T00:00:00Z'), row('turn_context', { turn_id: 'turn', cwd: f.project }, '2026-10-14T00:00:01Z'), row('event_msg', { type: 'agent_message', phase: 'final_answer', message: text, turn_id: 'turn' }, '2026-10-14T00:00:02Z')];
    await f.write('task', rows);
    const prepare = () => f.client.live!({ query: { action: 'usage', roots: [f.root], scope: { allTime: true } }, mode: 'fresh' });
    const view = await prepare(); assert.equal(view.freshness.status, 'current');
    const id = view.result.snapshotRef.snapshotId;
    const base: ConversationRequest = { action: 'locate', snapshotId: id, selection: { projects: [f.project], since: '2026-10-14', until: '2026-10-15', timezone: 'Asia/Shanghai' } };
    const located = await f.client.conversation!(base); assert.equal(located.items.length, 1); assert.equal(located.sourceBytesRead, 0); assert(located.scanComplete);
    assert(!JSON.stringify(located).includes('PRIVATE_SYNTHETIC_BODY'));
    const target = located.items[0].evidence;
    const activity = await f.client.conversation!({ ...base, action: 'activity' }); assert(activity.items.some(i => i.kind === 'message'));
    const methods = JSON.parse(f.cli(['api'])); assert(methods.methods.includes('conversation'));
    const cliLocated: ConversationResult = JSON.parse(f.cli(['call'], JSON.stringify({ method: 'conversation', params: base }))); assert.equal(cliLocated.items[0].evidence.eventId, target.eventId);
    const read: ConversationRequest = { ...base, action: 'read', targets: [{ evidence: target, maxBytes: 256 * 1024 }] };
    let result = await f.client.conversation!(read), rebuilt = result.fragments[0].text, bytes = result.sourceBytesRead;
    while (result.fragments[0].nextCursor) {
      read.targets![0].cursor = result.fragments[0].nextCursor;
      result = await f.client.conversation!(read); rebuilt += result.fragments[0].text; bytes += result.sourceBytesRead;
    }
    assert.equal(rebuilt, text); assert(bytes < Buffer.byteLength(text) + 32768, 'Large-message continuation must not replay whole records');
    await assert.rejects(f.client.conversation!({ ...base, action: 'read', targets: [{ evidence: target }], selection: { projects: [path.join(f.dir, 'unrelated')] } }), { code: 'NOT_FOUND' });
    const browser = await f.browser();
    await assert.rejects(browser.conversation!({ ...base, snapshotId: 'foreign' }), { code: 'INVALID_ARGUMENT' });
    await assert.rejects(browser.conversation!({ ...base, roots: [f.root] }), { code: 'INVALID_ARGUMENT' });
    await browser.live!({ query: { action: 'usage', scope: { allTime: true } }, mode: 'cached' });
    const browserLocated = await browser.conversation!(base); assert.equal(browserLocated.items[0].evidence.eventId, target.eventId);
    const browserRead = await browser.conversation!({ ...base, action: 'read', targets: [{ evidence: target, maxBytes: 64 }] }); assert(browserRead.fragments[0].text.startsWith('PRIVATE_SYNTHETIC_BODY 中文🙂'));
    const cross = await f.client.conversation!({ ...base, action: 'search', query: '中文🙂' }); assert.equal(cross.items.length, 1);
    await f.append('task', [row('event_msg', { type: 'user_message', message: 'next', turn_id: 'turn' }, '2026-10-14T01:00:00Z')]);
    const later = await prepare();
    const changes = await f.client.conversation!({ ...base, action: 'changes', snapshotId: later.result.snapshotRef.snapshotId, baselineSnapshotId: id }); assert.equal(changes.items.filter(i => i.kind === 'message').length, 1); assert(changes.items.every(i => i.change === 'appended'));
    const source = path.join(f.root, 'sessions/task.jsonl');
    const replacement = source + '.replacement'; await writeFile(replacement, await readFile(source)); await rename(replacement, source);
    await assert.rejects(f.client.conversation!({ ...base, action: 'read', targets: [{ evidence: target }] }), { code: 'SOURCE_CHANGED' });
    await rm(source);
    await assert.rejects(f.client.conversation!({ ...base, action: 'read', targets: [{ evidence: target }] }), { code: 'SOURCE_UNREADABLE' });
  });
});

test('real RPC navigation remains bounded across many messages and service restart', { timeout: 120_000 }, async () => {
  await withLocalProduct(async f => {
    const rows = [row('session_meta', { id: 'task', cwd: f.project }, '2026-10-14T00:00:00Z'), row('turn_context', { turn_id: 'turn', cwd: f.project }, '2026-10-14T00:00:01Z')];
    for (let i = 0; i < 3000; i++) rows.push(row('event_msg', { type: 'agent_message', message: `message ${i} ` + 'x'.repeat(1024), phase: 'final_answer', turn_id: 'turn' }, '2026-10-14T00:00:02Z'));
    await f.write('many', rows);
    let view = await f.client.live!({ query: { action: 'usage', roots: [f.root], scope: { allTime: true } }, mode: 'fresh' });
    const times: number[] = [];
    for (let i = 0; i < 30; i++) {
      const start = performance.now();
      const result = await f.client.conversation!({ action: 'locate', snapshotId: view.result.snapshotRef.snapshotId, selection: { projects: [f.project] }, limit: 3 });
      assert.equal(result.items.length, 3, JSON.stringify({limits:result.limits,complete:result.scanComplete,partial:result.partial,scanned:result.scannedEvents,facets:view.result.facets,quality:view.result.quality})); assert.equal(result.sourceBytesRead, 0); times.push(performance.now() - start);
    }
    if (process.env.WOMBAT_CONVERSATION_BENCHMARK_OUTPUT) await writeFile(process.env.WOMBAT_CONVERSATION_BENCHMARK_OUTPUT, JSON.stringify({ source: '3000 synthetic messages, about 3 MiB; fresh product-data directory', platform: `${process.platform}/${process.arch}`, method: 'real Node client to release Rust service; first and repeated RPCs; uncontrolled OS cache', firstMs: times[0], p95Ms: [...times.slice(1)].sort((a,b)=>a-b)[Math.ceil((times.length-1)*.95)-1], maximumMs: Math.max(...times), samplesMs: times }, null, 2));
    await f.stop(); await f.start();
    view = await f.client.live!({ query: { action: 'usage', roots: [f.root], scope: { allTime: true } }, mode: 'fresh' });
    const request: ConversationRequest = { action: 'locate', snapshotId: view.result.snapshotRef.snapshotId, selection: { projects: [f.project] }, limit: 2 };
    const first = await f.client.conversation!(request); assert(first.nextCursor);
    const second = await f.client.conversation!({ ...request, cursor: first.nextCursor }); assert.equal(second.items.length, 2); assert.notEqual(second.items[0].evidence.eventId, first.items[0].evidence.eventId);
    await assert.rejects(f.client.conversation!({ ...request, cursor: first.nextCursor, selection: { projects: [f.project], phases: ['commentary'] } }), { code: 'CURSOR_MISMATCH' });
    const restored = await f.client.conversation!({ ...request, action: 'read', limit: undefined, targets: [{ evidence: second.items[0].evidence, maxBytes: 64 }] }); assert(restored.fragments[0].text.startsWith('message '));
  });
});
