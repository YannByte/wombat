import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, writeFileSync, rmSync, symlinkSync } from 'node:fs';
import path from 'node:path';
import os from 'node:os';
import { test } from 'node:test';
import {
  installedOnboardingCore,
  assertOnboardingCoreOwner,
  cleanupOnboardingCores,
} from './onboarding-cleanup.ts';

test('failed startup can resolve the installed core from its exact installation identity', async () => {
  const prefix = mkdtempSync(path.join(os.tmpdir(), 'wombat-cleanup-'));
  try {
    const root = path.join(prefix, 'lib/wombat'),
      id = '0.3.0-synthetic',
      core = path.join(root, 'versions', id, 'lib/wombat-core');
    mkdirSync(path.dirname(core), { recursive: true });
    writeFileSync(core, 'synthetic');
    writeFileSync(path.join(root, 'current.txt'), id + '\n');
    assert.equal(installedOnboardingCore(prefix), core);
    assertOnboardingCoreOwner(core, core);
    if(process.platform!=='win32') {
      const alias=path.join(prefix,'alias');symlinkSync(root,alias,'dir');
      assertOnboardingCoreOwner(core,path.join(alias,'versions',id,'lib/wombat-core'));
    }
    for (const command of ['/other/wombat-core --serve-usage', core + ' --other', ' --serve-usage'])
      assert.throws(() => assertOnboardingCoreOwner(command, core));
    writeFileSync(path.join(root, 'current.txt'), '../../outside');
    assert.throws(() => installedOnboardingCore(prefix));
    writeFileSync(path.join(root, 'current.txt'), id);
    if (process.platform !== 'win32') {
      rmSync(core);
      symlinkSync(process.execPath, core);
      assert.throws(() => installedOnboardingCore(prefix));
    }
    // A failure before any service exists needs no installed pointer.
    await cleanupOnboardingCores(path.join(prefix, 'absent'), [path.join(prefix, 'unused-data')]);
  } finally {
    rmSync(prefix, { recursive: true, force: true });
  }
});
