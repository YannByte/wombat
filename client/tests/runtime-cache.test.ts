import {test} from 'node:test';
import assert from 'node:assert/strict';
import {spawnSync} from 'node:child_process';
import {mkdtempSync,mkdirSync,writeFileSync,rmSync} from 'node:fs';
import os from 'node:os';
import path from 'node:path';

test('module cache stays with its managed version and cannot block a call', () => {
  const root = mkdtempSync(path.join(os.tmpdir(),'wombat-module-cache-'));
  try {
    const version = path.join(root,'versions/test'), lib = path.join(version,'lib');
    mkdirSync(lib,{recursive:true});
    writeFileSync(path.join(root,'.managed-by-wombat'),'managed GitHub installation\n');
    const script = `import {getCompileCacheDir} from 'node:module';
      import {enableInstalledCompileCache} from ${JSON.stringify(new URL('../src/node/runtime.ts',import.meta.url).href)};
      process.argv[1]=process.env.TEST_ENTRY;
      enableInstalledCompileCache();
      console.log(JSON.stringify({directory:getCompileCacheDir()??null,completed:true}));`;
    const run = (entry:string, extra:NodeJS.ProcessEnv={}) => {
      const env:NodeJS.ProcessEnv = {...process.env,WOMBAT_INSTALL_ROOT:root,TEST_ENTRY:entry};
      delete env.NODE_COMPILE_CACHE;delete env.NODE_DISABLE_COMPILE_CACHE;
      const r = spawnSync(process.execPath,['--import','tsx','--input-type=module','-e',script],{
        env:{...env,...extra},encoding:'utf8',timeout:15000,maxBuffer:65536,
      });
      assert.equal(r.status,0,r.stderr);
      const value=JSON.parse(r.stdout);assert.equal(value.completed,true);return value.directory as string|null;
    };
    assert.equal(run(path.join(root,'outside.js')),null);
    assert.equal(run(path.join(lib,'wombat.js'),{NODE_DISABLE_COMPILE_CACHE:'1'}),null);
    const enabled=run(path.join(lib,'wombat.js'));
    assert.ok(enabled?.includes(path.join('versions','test','cache','node')));
    rmSync(path.join(version,'cache'),{recursive:true,force:true});
    writeFileSync(path.join(version,'cache'),'unwritable cache path');
    assert.equal(run(path.join(lib,'wombat.js')),null);
  } finally {rmSync(root,{recursive:true,force:true});}
});
