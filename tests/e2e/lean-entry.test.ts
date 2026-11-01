import assert from 'node:assert/strict';
import { mkdtempSync, existsSync, rmSync } from 'node:fs';
import { spawnSync } from 'node:child_process';
import path from 'node:path';
import os from 'node:os';
import test from 'node:test';

test('local discovery needs no initialization and removed browser commands cannot start a host', () => {
  const dir = mkdtempSync(path.join(os.tmpdir(), 'wombat-lean-entry-'));
  const data = path.join(dir,'data');
  const env = {...process.env, WOMBAT_DATA_HOME:data, CODEX_HOME:path.join(dir,'absent-source'), WOMBAT_AUTO_PRICES:'0', WOMBAT_CORE_BIN:path.join(dir,'absent-core')};
  try {
    for (const args of [['--help','--json'], ['--version','--json'], ['system','api','--json'], ['system','api','conversation','--json']]) {
      const result=spawnSync(process.execPath,[path.resolve('dist/wombat.js'),...args],{env,encoding:'utf8',timeout:10000,maxBuffer:2*1024*1024});
      assert.ifError(result.error);
      assert.equal(result.status,0,result.stderr+result.stdout);
      const value=JSON.parse(result.stdout);
      if(value.commands) assert.ok(!value.commands.includes('web'));
      assert.equal(existsSync(data),false,'Discovery initialized product state');
    }
    const removed=spawnSync(process.execPath,[path.resolve('dist/wombat.js'),'web','--json'],{env,encoding:'utf8',timeout:10000,maxBuffer:65536});
    assert.equal(removed.status,1);
    assert.equal(JSON.parse(removed.stdout).error.code,'INVALID_ARGUMENT');
    assert.equal(existsSync(data),false);
  } finally {rmSync(dir,{recursive:true,force:true});}
});
