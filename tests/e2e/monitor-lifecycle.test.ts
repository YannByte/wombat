import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFile, rm } from 'node:fs/promises';
import { DatabaseSync } from 'node:sqlite';
import { spawn } from 'node:child_process';
import { once } from 'node:events';
import { setTimeout as delay } from 'node:timers/promises';
import path from 'node:path';
import { withLocalProduct, row, measurement } from '../helpers/local-product.ts';

test('monitor thresholds, late reviews and acknowledgement survive restarts and derived-index rebuilding', { timeout: 60_000 }, async () => {
  await withLocalProduct(async f => {
    const day = (offset: number) => new Date(Date.now() + offset * 86_400_000).toISOString().slice(0, 10);
    const today = day(0) + 'T00:01:00Z', yesterday = day(-1) + 'T00:01:00Z';
    await f.write('task', [row('session_meta', { id: 'task', cwd: f.project }, today), row('turn_context', { turn_id: 'turn', model: 'gpt-5.4' }, today), measurement('task', 'past', 10, yesterday), measurement('task', 'initial', 79, today)]);
    const nativeApi = await f.native();
    const snapshot = async () => (await nativeApi.live!({ query: { action: 'usage', scope: { allTime: true } }, mode: 'fresh' })).result.snapshotRef.snapshotId;
    const check = async (id = 'budget') => nativeApi.monitor!({ action: 'check', snapshotId: await snapshot(), ids: [id] });
    const plan = { id: 'budget', enabled: true, period: 'day' as const, tokenLimit: 100, warningRatio: 0.8, review: false, scope: { project: f.project, timezone: 'UTC' } };
    await nativeApi.monitor!({ action: 'upsert', plan });
    assert.equal((await check()).notifications.length, 0);
    await f.append('task', [measurement('task', 'warning', 1, today)]);
    const warning = (await check()).notifications;
    assert.equal(warning.length, 1); assert.equal(warning[0].kind, 'budget_warning'); assert.equal(warning[0].summary.tokens.total, 80);
    assert.equal((await check()).notifications.length, 0);
    await nativeApi.monitor!({ action: 'acknowledge', notificationId: warning[0].id });
    await nativeApi.monitor!({ action: 'upsert', plan: { ...plan, enabled: false } });
    await f.append('task', [measurement('task', 'exceeded', 20, today)]);
    assert.equal((await check()).notifications.length, 0, 'Paused plans retain facts without emitting new notices');
    await nativeApi.monitor!({ action: 'upsert', plan });
    const exceeded = (await check()).notifications;
    assert.equal(exceeded.length, 1); assert.equal(exceeded[0].kind, 'budget_exceeded'); assert.equal(exceeded[0].summary.tokens.total, 100);
    assert.notEqual(exceeded[0].id, warning[0].id);
    await nativeApi.monitor!({ action: 'upsert', plan: { ...plan, id: 'review', tokenLimit: undefined, review: true } });
    const review = (await check('review')).notifications;
    assert.equal(review.length, 1); assert.equal(review[0].kind, 'period_review'); assert.equal(review[0].summary.tokens.total, 10);
    assert.equal((await check('review')).notifications.length, 0);
    await f.append('task', [measurement('task', 'late-past', 15, yesterday)]);
    const amended = (await check('review')).notifications;
    assert.equal(amended.length, 1); assert.equal(amended[0].summary.tokens.total, 25); assert.notEqual(amended[0].id, review[0].id);
    const before = await nativeApi.monitor!({ action: 'list' });
    assert.equal(before.notifications.length, 4); assert.equal(before.notifications.find(n => n.id === warning[0].id)?.acknowledged, true);
    await f.stop();
    await rm(path.join(f.data, 'live-v2'), { recursive: true, force: true });
    await f.start(); await snapshot();
    const restored = JSON.parse(f.cli({method:'monitor',params:{action:'list'}}));
    assert.deepEqual(restored.plans, before.plans); assert.deepEqual(restored.notifications, before.notifications);
    assert.equal((await check()).notifications.length, 0); assert.equal((await check('review')).notifications.length, 0);
    await nativeApi.monitor!({ action: 'remove', id: 'budget' });
    assert.equal((await nativeApi.monitor!({ action: 'list' })).notifications.length, 4, 'Removing the plan preserves notification history');
    await assert.rejects(check('budget'), { code: 'NOT_FOUND' });
    await assert.rejects(nativeApi.monitor!({ action: 'upsert', plan: { ...plan, scope: { ...plan.scope, allTime: true } } }), { code: 'INVALID_ARGUMENT' });
    await assert.rejects(nativeApi.monitor!({ action: 'upsert', plan: { ...plan, warningRatio: 0 } }), { code: 'INVALID_ARGUMENT' });
    assert.equal((await nativeApi.monitor!({ action: 'list' })).plans.length, 1, 'Rejected writes cannot create plans');
    await f.stop();
    const file = path.join(f.data, 'user-v1/monitor.sqlite');
    const db = new DatabaseSync(file); db.exec('PRAGMA user_version=99'); db.close();
    const unknown = await readFile(file);
    await f.start();
    await assert.rejects(nativeApi.monitor!({ action: 'list' }), { code: 'UNSUPPORTED_VERSION' });
    await f.stop();
    assert.deepEqual(await readFile(file), unknown, 'An unsupported user-store version must remain byte-for-byte intact');
  });
});

test('partial measured usage can prove a budget crossing without claiming a complete task total', { timeout: 40_000 }, async () => {
  await withLocalProduct(async f => {
    const at = new Date().toISOString().slice(0, 10) + 'T00:01:00Z';
    await f.write('task', [row('session_meta', { id: 'task', cwd: f.project }, at), row('turn_context', { turn_id: 'turn', model: 'gpt-5.4' }, at), measurement('task', 'known', 100, at), measurement('task', 'missing', null, at)]);
    f.cli({method:'monitor',params:{action:'upsert',plan:{id:'partial',enabled:true,period:'day',tokenLimit:Number('100'),review:false,scope:{project:f.project,timezone:'UTC'}}}});
    const checked = JSON.parse(f.cli(['usage','alerts','--once','--id','partial','--root',f.root,'--json'], '', 2));
    const notice = checked.notifications[0]; assert.equal(checked.notifications.length, 1);
    assert.equal(notice.kind, 'budget_exceeded'); assert.equal(notice.partial, true);
    assert.equal(notice.summary.tokens.total, null); assert.equal(notice.summary.tokenAnalysis.totalAnalysis.subtotal, 100);
    assert.equal(notice.statistics.population.incompleteTasks, 1); assert.equal(notice.statistics.population.completeTasks, 0); assert.equal(notice.statistics.population.meanTokens, null);
    const nativeApi = await f.native();
    const listed = await nativeApi.monitor!({ action: 'list' }); assert.deepEqual(listed.notifications[0], notice);
    const repeated = JSON.parse(f.cli(['usage','alerts','--once','--id','partial','--root',f.root,'--json']));
    assert.equal(repeated.notifications.length, 0);
  });
});

test('Agent monitor watch detects newly appended usage, stays quiet on repeats and cancels', { timeout: 40_000, skip: process.platform === 'win32' ? 'POSIX signal cancellation; Windows monitor lifecycle is covered separately' : false }, async () => {
  await withLocalProduct(async f => {
    const at = new Date().toISOString().slice(0, 10) + 'T00:01:00Z';
    await f.write('task', [row('session_meta', { id: 'task', cwd: f.project }, at), row('turn_context', { turn_id: 'turn', model: 'gpt-5.4' }, at), measurement('task', 'initial', 79, at)]);
    f.cli({method:'monitor',params:{action:'upsert',plan:{id:'watch',enabled:true,period:'day',tokenLimit:Number('100'),review:false,scope:{project:f.project,timezone:'UTC'}}}});
    const child = spawn(process.execPath, [f.entry, 'usage', 'alerts', '--id', 'watch', '--root', f.root, '--interval', '5', '--json'], { env: process.env, stdio: ['ignore', 'pipe', 'pipe'] });
    const exiting = once(child, 'exit');
    let stdout = '', stderr = '';
    child.stdout.on('data', chunk => { stdout += chunk; if (stdout.length > 262_144) child.kill('SIGKILL'); });
    child.stderr.on('data', chunk => { stderr += chunk; if (stderr.length > 262_144) child.kill('SIGKILL'); });
    const timeout = setTimeout(() => child.kill('SIGKILL'), 30_000);
    try {
      await once(child, 'spawn');
      await delay(6_000); assert.equal(stdout, '', 'Checks below the threshold should not emit empty JSON');
      await f.append('task', [measurement('task', 'crossing', 21, at)]);
      const deadline = Date.now() + 15_000;
      while (!stdout.includes('\n') && Date.now() < deadline && child.exitCode === null && child.signalCode === null) await delay(50);
      assert.ok(stdout.includes('\n'), stderr || 'Watch did not publish the newly crossed threshold');
      const notice = JSON.parse(stdout.trim()); assert.equal(notice.notifications.length, 1); assert.equal(notice.notifications[0].kind, 'budget_exceeded');
      await delay(6_000); assert.equal(stdout.trim().split('\n').length, 1, 'Unchanged checks must not repeat notifications');
      child.kill('SIGTERM'); const [code, signal] = await exiting;
      assert.equal(code, 130, stderr); assert.equal(signal, null);
      const persisted = JSON.parse(f.cli({method:'monitor',params:{action:'list'}}));
      assert.equal(persisted.notifications.length, 1); assert.equal(persisted.notifications[0].id, notice.notifications[0].id);
    } finally {
      clearTimeout(timeout);
      if (child.exitCode === null && child.signalCode === null) { child.kill('SIGKILL'); await exiting; }
    }
  });
});
