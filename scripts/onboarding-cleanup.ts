import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { existsSync, lstatSync, readFileSync, realpathSync } from 'node:fs';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';

/** Resolve the exact core executable in an isolated installation. */
export function installedOnboardingCore(prefix: string): string {
  const root = path.join(prefix, 'lib/wombat'),
    pointer = path.join(root, 'current.txt');
  assert.ok(
    !lstatSync(pointer).isSymbolicLink(),
    'Test installation pointer must be a regular file',
  );
  const id = readFileSync(pointer, 'utf8').trim();
  assert.match(id, /^[0-9A-Za-z][0-9A-Za-z._-]{0,127}$/);
  const payload = path.join(root, 'versions', id),
    core = path.join(payload, 'lib/wombat-core');
  assert.ok(
    !lstatSync(payload).isSymbolicLink() && !lstatSync(core).isSymbolicLink(),
    'Test runtime must not redirect ownership',
  );
  assert.equal(
    realpathSync(core),
    path.join(realpathSync(root), 'versions', id, 'lib/wombat-core'),
  );
  return core;
}

export function assertOnboardingCoreOwner(command: string, core: string): void {
  assert.equal(realpathSync(command),realpathSync(core),'Refusing to stop a process outside the test installation');
}

function absent(error: unknown): boolean {
  return error != null && typeof error === 'object' && 'code' in error && error.code === 'ESRCH';
}

function holders(lock: string): string[] {
  try {
    return execFileSync(process.platform === 'darwin' ? '/usr/sbin/lsof' : 'lsof', ['-t', lock], {
      encoding: 'utf8',
      timeout: 5000,
      maxBuffer: 65536,
    })
      .trim()
      .split('\n')
      .filter(Boolean);
  } catch (error) {
    if (error != null && typeof error === 'object' && 'status' in error && error.status === 1)
      return [];
    throw error;
  }
}

/** Locks in isolated data directories plus the installed command establish ownership. */
export async function cleanupOnboardingCores(
  prefix: string,
  directories: readonly string[],
): Promise<void> {
  let core: string | undefined;
  for (const directory of directories) {
    const lock = path.join(directory, 'live-v2/request.lock');
    if (!existsSync(lock)) continue;
    for (const value of holders(lock)) {
      assert.match(value, /^\d+$/);
      const pid = Number(value);
      let command: string;
      try {
        command = execFileSync('/bin/ps', ['-p', value, '-o', 'command='], {
          encoding: 'utf8',
          timeout: 5000,
          maxBuffer: 65536,
        }).trim();
      } catch (error) {
        if (error != null && typeof error === 'object' && 'status' in error && error.status === 1)
          continue;
        throw error;
      }
      core ??= installedOnboardingCore(prefix);
      assertOnboardingCoreOwner(command, core);
      try {
        process.kill(pid, 'SIGTERM');
      } catch (error) {
        if (absent(error)) continue;
        throw error;
      }
      const until = Date.now() + 5000;
      for (;;) {
        try {
          process.kill(pid, 0);
        } catch (error) {
          if (absent(error)) break;
          throw error;
        }
        if (Date.now() > until) throw new Error('Owned foreground core did not stop');
        await delay(50);
      }
    }
  }
}
