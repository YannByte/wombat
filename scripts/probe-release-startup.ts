/** Foreground capability discovery must not touch sources or open a listener. */
import assert from 'node:assert/strict';
import {mkdtempSync,rmSync,existsSync} from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {spawnProductCli} from './cli-invocation.ts';
const root=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'..');
const temporary=mkdtempSync(path.join(os.tmpdir(),'wombat-release-probe-'));
try {
 const data=path.join(temporary,'data');
 const env={...process.env,WOMBAT_AUTO_PRICES:'0',WOMBAT_DATA_HOME:data,CODEX_HOME:path.join(temporary,'missing-source-must-not-be-scanned')};
 for(const method of ['optimize','config','timing'] as const){
  const result=spawnProductCli(path.join(root,'dist/wombat.js'),{method,params:{action:'capabilities'}},{cwd:temporary,env,encoding:'utf8',timeout:15000,maxBuffer:8*1024*1024});
  assert.ifError(result.error);assert.equal(result.status,0,result.stdout+result.stderr);
  assert.equal(result.stderr,'');assert.equal(result.stdout.trim().split('\n').length,1);
  assert.equal(JSON.parse(result.stdout).action,'capabilities');
 }
 assert.equal(existsSync(path.join(data,'live-v2')),false,'capability discovery must not create an index, listener or process registration');
 console.log(`Foreground startup probe passed on ${process.platform}/${process.arch} without scanning sources.`);
}finally{rmSync(temporary,{recursive:true,force:true,maxRetries:5,retryDelay:100});}
