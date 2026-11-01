import assert from 'node:assert/strict';
import { spawn, spawnSync, type ChildProcess } from 'node:child_process';
import { mkdtempSync, rmSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { fileURLToPath } from 'node:url';
import {spawnProductCli} from './cli-invocation.ts';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const temporary = mkdtempSync(path.join(os.tmpdir(), 'wombat-release-probe-'));
const env: NodeJS.ProcessEnv = {
  ...process.env,
  WOMBAT_AUTO_PRICES: '0',
  WOMBAT_SERVICE_TRACE: '1',
  WOMBAT_DATA_HOME: path.join(temporary, 'data'),
  CODEX_HOME: path.join(temporary, 'missing-source-must-not-be-scanned'),
};
let service: ChildProcess | undefined;
let diagnostics = '';

async function stop(child: ChildProcess): Promise<void> {
  const stopped = () => child.exitCode !== null || child.signalCode !== null;
  const waitForExit = (milliseconds: number) => new Promise<void>(resolve => {
    const finished = () => { clearTimeout(timer); resolve(); };
    const timer = setTimeout(() => { child.off('exit', finished); resolve(); }, milliseconds);
    child.once('exit', finished);
  });
  if (stopped()) return;
  const exited = waitForExit(5_000);
  if (process.platform === 'win32' && child.pid) {
    const killed = spawnSync('taskkill', ['/PID', String(child.pid), '/T', '/F'], {
      encoding: 'utf8',
      windowsHide: true,
    });
    if (killed.error) throw killed.error;
  } else {
    child.kill('SIGKILL');
  }
  await exited;
  assert.equal(stopped(), true, 'shared service did not stop after the probe');
}

try {
  const core = path.join(root, 'dist', process.platform === 'win32' ? 'wombat-core.exe' : 'wombat-core');
  service = spawn(core, ['--serve-usage'], { env, windowsHide: true, stdio: ['ignore', 'ignore', 'pipe'] });
  service.stderr?.on('data', chunk => { diagnostics = (diagnostics + String(chunk)).slice(-8_000); });
  await new Promise<void>((resolve, reject) => {
    const ready = () => { service!.off('error', failed); resolve(); };
    const failed = (error: Error) => { service!.off('spawn', ready); reject(error); };
    service!.once('spawn', ready);
    service!.once('error', failed);
  });
  await delay(100);
  assert.equal(service.exitCode, null, diagnostics || 'shared service exited before the probe');
  assert.equal(service.signalCode, null, diagnostics || 'shared service exited before the probe');
  const result = spawnProductCli(path.join(root, 'dist', 'wombat.js'), {method:'optimize',params:{action:'capabilities'}}, {
    cwd: temporary,
    env,
    encoding: 'utf8',
    timeout: 15_000,
    maxBuffer: 8 * 1024 * 1024,
  });
  // spawnSync blocks delivery of the service's piped stderr events. Yield once so a failure
  // includes the complete connection-stage diagnostics instead of only the startup message.
  await delay(25);
  assert.ifError(result.error);
  assert.equal(result.status, 0, result.stdout + result.stderr + diagnostics);
  assert.equal(result.stderr, '');
  assert.equal(result.stdout.trim().split('\n').length, 1, 'startup probe must print one JSON object');
  const value = JSON.parse(result.stdout);
  assert.equal(value.action, 'capabilities');
  assert.equal(value.resultStatus, 'complete');
  assert.equal(value.readView, null);
  assert.equal(service.exitCode, null, diagnostics || 'shared service exited during the probe');
  assert.equal(service.signalCode, null, diagnostics || 'shared service exited during the probe');
  console.log(`Shared-service startup probe passed on ${process.platform}/${process.arch} without scanning sources.`);
} finally {
  if (service) await stop(service);
  rmSync(temporary, { recursive: true, force: true, maxRetries: 5, retryDelay: 500 });
}
