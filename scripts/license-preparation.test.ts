import assert from 'node:assert/strict';
import test from 'node:test';
import {mkdtempSync,mkdirSync,readFileSync,writeFileSync,copyFileSync,rmSync,existsSync} from 'node:fs';
import {spawnSync} from 'node:child_process';
import path from 'node:path';
import os from 'node:os';
import {pathToFileURL} from 'node:url';

test('license preparation fetches the complete locked source set without compiling or changing build outputs',()=>{
  const root=mkdtempSync(path.join(os.tmpdir(),'wombat-license-prepare-'));
  try{
    mkdirSync(path.join(root,'scripts'));
    for(const file of ['build-core.mjs','native-binary.ts'])copyFileSync(new URL('./'+file,import.meta.url),path.join(root,'scripts',file));
    const preload=path.join(root,'cargo-probe.mjs'),log=path.join(root,'calls.json');
    writeFileSync(preload,`import path from 'node:path';import {writeFileSync} from 'node:fs';if(path.basename(process.argv[1]??'')==='fetch'){writeFileSync(process.env.FETCH_LOG_FILE,JSON.stringify(['fetch',...process.argv.slice(2)]));process.exit(0);}`);
    const result=spawnSync(process.execPath,[path.join(root,'scripts/build-core.mjs'),'--fetch'],{cwd:root,env:{...process.env,WOMBAT_CARGO:process.execPath,NODE_OPTIONS:'--import='+pathToFileURL(preload).href,FETCH_LOG_FILE:log},encoding:'utf8',timeout:10000,maxBuffer:65536});
    assert.ifError(result.error);assert.equal(result.status,0,result.stderr);
    assert.deepEqual(JSON.parse(readFileSync(log,'utf8')),['fetch','--locked','--manifest-path','core/Cargo.toml']);
    assert.equal(existsSync(path.join(root,'dist')),false);
  }finally{rmSync(root,{recursive:true,force:true});}
});
