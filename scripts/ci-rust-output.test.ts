import assert from 'node:assert/strict';
import test from 'node:test';
import {spawnSync} from 'node:child_process';
import {existsSync,mkdirSync,mkdtempSync,readFileSync,rmSync,statSync,utimesSync,writeFileSync} from 'node:fs';
import path from 'node:path';
import os from 'node:os';
import {fileURLToPath} from 'node:url';

test('Windows Cargo output survives pruning only for an exact cache hit',()=>{
  const dir=mkdtempSync(path.join(os.tmpdir(),'wombat-cargo-output-'));
  const helper=fileURLToPath(new URL('../.github/actions/rust/windows-output.sh',import.meta.url));
  const artifact=path.join(dir,'core/target/release/deps/wombat_core.exe');
  const retained=path.join(dir,'core/.ci-rust-outputs/release/wombat_core.exe');
  const run=(mode:string,hit='false',platform='Windows')=>spawnSync('bash',[helper,mode],{
    cwd:dir,encoding:'utf8',timeout:10000,maxBuffer:65536,env:{...process.env,RUNNER_OS:platform,CI_RUST_CACHE_HIT:hit},
  });
  try{
    assert.equal(run('invalid').status,1);
    assert.equal(run('restore','true').status,0);assert.equal(existsSync(artifact),false);
    mkdirSync(path.dirname(artifact),{recursive:true});writeFileSync(artifact,'synthetic native output');
    const stamp=new Date('2026-01-01T00:00:00Z');utimesSync(artifact,stamp,stamp);
    assert.equal(run('save','false','Linux').status,0);assert.equal(existsSync(retained),false);
    assert.equal(run('save').status,0);rmSync(artifact);
    assert.equal(run('restore').status,0);assert.equal(existsSync(artifact),false);
    assert.equal(run('restore','true').status,0);
    assert.equal(readFileSync(artifact,'utf8'),'synthetic native output');assert.equal(statSync(artifact).mtimeMs,stamp.getTime());
  }finally{rmSync(dir,{recursive:true,force:true});}
});
