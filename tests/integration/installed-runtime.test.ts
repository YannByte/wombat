import { queryInvocation, spawnProductCli, type CliInput } from '../../scripts/cli-invocation.js';
import assert from 'node:assert/strict';
import { spawn, spawnSync, type ChildProcess } from 'node:child_process';
import { accessSync, constants, cpSync, mkdirSync, mkdtempSync, readdirSync, realpathSync, rmSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { test } from 'node:test';

function canonicalPath(file: string): string {
  const resolved = realpathSync.native(file);
  return process.platform === 'win32' ? resolved.toLowerCase() : resolved;
}

async function stopService(child: ChildProcess): Promise<void> {
  const stopped = () => child.exitCode !== null || child.signalCode !== null;
  if (stopped()) return;
  const exited = new Promise<void>(resolve => {
    const finished = () => { clearTimeout(timer); resolve(); };
    const timer = setTimeout(() => { child.off('exit', finished); resolve(); }, 5_000);
    child.once('exit', finished);
  });
  if (process.platform === 'win32' && child.pid) {
    const result = spawnSync('taskkill', ['/PID', String(child.pid), '/T', '/F'], { encoding: 'utf8', windowsHide: true });
    if (result.error) throw result.error;
  } else child.kill('SIGKILL');
  await exited;
  assert.equal(stopped(), true, 'installed runtime service did not stop');
}

test('installed runtime queries synthetic usage, fixed drill-down and configuration outside the repository', { timeout: 45000 }, async () => {
  const temp = realpathSync.native(mkdtempSync(path.join(os.tmpdir(), 'wombat-skill-flow-')));
  let service: ChildProcess | undefined;
  try {
    const installed = path.join(temp, 'runtime');
    mkdirSync(installed);
    const dist = path.resolve('dist');
    for (const name of readdirSync(dist).filter(name => /^wombat(?:-.*)?\.js$/.test(name))) cpSync(path.join(dist, name), path.join(installed, name));
    const coreName = process.platform === 'win32' ? 'wombat-core.exe' : 'wombat-core';
    cpSync(path.join(dist, coreName), path.join(installed, coreName));
    cpSync(path.join(dist, 'skill'), path.join(installed, 'skill'), {recursive:true});
    accessSync(path.join(installed, coreName), constants.X_OK);
    const source = path.join(temp, 'source'), project = path.join(temp, 'project');
    mkdirSync(path.join(source, 'sessions'), { recursive: true }); mkdirSync(project);
    writeFileSync(path.join(project, 'AGENTS.md'), '# Project\nSynthetic instructions.\n');
    const skill = path.join(project, '.agents/skills/sample/SKILL.md'); mkdirSync(path.dirname(skill), { recursive: true });
    writeFileSync(skill, `---\nname: sample\ndescription: ${'x'.repeat(501)}\n---\nSynthetic skill.\n`);
    const canonicalSkill = canonicalPath(skill);
    writeFileSync(path.join(source, 'AGENTS.md'), 'Synthetic source rules.\n');
    const row = (type: string, payload: object) => ({ timestamp: '2026-10-02T17:00:00Z', type, payload });
    for (const [id, input] of [['high', 200], ['low', 100]] as const) {
      const rows = [row('session_meta', { id, cwd: project }), row('turn_context', { turn_id: id + '-turn', cwd: project, model: 'gpt-5.4' }), row('event_msg', { type: 'task_started', turn_id: id + '-turn' }), row('event_msg', { type: 'token_usage_record', thread_id: id, turn_id: id + '-turn', response_id: id + '-response', usage: { input_tokens: input, cached_input_tokens: 0, output_tokens: input / 10, total_tokens: input + input / 10 } })];
      writeFileSync(path.join(source, 'sessions', id + '.jsonl'), rows.map(row => JSON.stringify(row)).join('\n') + '\n');
    }
    const env = { ...process.env, WOMBAT_DATA_HOME: path.join(temp, 'data'), CODEX_HOME: source, WOMBAT_AUTO_PRICES: '0', WOMBAT_CORE_BIN: '' };
    const core = path.join(installed, coreName);
    service = spawn(core, ['--serve-usage'], { cwd: temp, env, windowsHide: true, stdio: 'ignore' });
    await new Promise<void>((resolve, reject) => {
      const ready = () => { service!.off('error', failed); resolve(); };
      const failed = (error: Error) => { service!.off('spawn', ready); reject(error); };
      service!.once('spawn', ready);
      service!.once('error', failed);
    });
    await delay(100);
    assert.equal(service.exitCode, null, 'installed runtime service exited during startup');
    const invoke = (args: CliInput): any => {
      const result = spawnProductCli(path.join(installed, 'wombat.js'), Array.isArray(args) ? [...args, '--json'] : args, {
        cwd: temp, env,
        encoding: 'utf8', timeout: 15000, maxBuffer: 4 * 1024 * 1024,
      });
      assert.ok(result.status !== null && [0, 2].includes(result.status), result.stderr + result.stdout);
      const value = JSON.parse(result.stdout); assert.ok(!value.error, result.stdout); return value;
    };
    const fixed = invoke({"method":"usage","params":{"query":{"action":"refresh"},"mode":"auto"}});
    const scope = ['--snapshot', fixed.snapshotRef.snapshotId, '--since', '2026-10-03', '--until', '2026-10-04', '--timezone', 'Asia/Shanghai'];
    assert.equal(invoke(['usage', ...scope]).summary.tokens.total, 330);
    const threads = invoke(queryInvocation({"action":"threads","sort":"tokens","limit":1,"snapshotId":fixed.snapshotRef.snapshotId,"scope":{"since":"2026-10-03","until":"2026-10-04","timezone":"Asia/Shanghai"}}, {mode:"auto"}));
    assert.equal(threads.page.total, 2); assert.equal(threads.summary.tokens.total, 330); assert.equal(threads.items[0].matchedUsage.tokens.total, 220);
    const turns = invoke(queryInvocation({"action":"turns","matchedOnly":true,"threadId":threads.items[0].id,"sort":"tokens","snapshotId":fixed.snapshotRef.snapshotId,"scope":{"since":"2026-10-03","until":"2026-10-04","timezone":"Asia/Shanghai"}}, {mode:"auto"}));
    assert.equal(turns.items[0].usage.tokens.total, 220);
    const steps = invoke(queryInvocation({"action":"steps","threadId":threads.items[0].id,"turnId":turns.items[0].id,"snapshotId":fixed.snapshotRef.snapshotId,"scope":{"since":"2026-10-03","until":"2026-10-04","timezone":"Asia/Shanghai"}}, {mode:"auto"}));
    assert.equal(steps.items.filter((item: { kind: string }) => item.kind === 'measurement').length, 1);
    const inventory = invoke({method:'config',params:{action:'list',roots:[source],projectRoots:[project]}});
    assert.ok(
      inventory.items.some((item: { path: string }) => canonicalPath(item.path) === canonicalSkill),
      `installed inventory did not include ${skill}: ${JSON.stringify(inventory.items.map((item: { path: string }) => item.path))}`,
    );
    const suggestions = invoke({method:'optimize',params:{action:'list',roots:[source],projectRoots:[project],readView:inventory.readView}});
    assert.ok(suggestions.suggestions.some((item: { item: { path: string }; findings: { rule: string }[] }) => canonicalPath(item.item.path) === canonicalSkill && item.findings.some(finding => finding.rule === 'descriptionSize')));
    await stopService(service); service = undefined;
    assert.ok(invoke(['--help']).commands.includes('check'));
  } finally {
    if (service) await stopService(service);
    rmSync(temp, { recursive: true, force: true, maxRetries: 10, retryDelay: 200 });
  }
});
