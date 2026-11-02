import test from 'node:test';
import assert from 'node:assert/strict';
import {cliInvocation,queryInvocation} from './cli-invocation.ts';
import {spawnSync} from 'node:child_process';

test('verification requests preserve filters and use the foreground usage protocol',()=>{
  const scope={project:'/synthetic/project',since:'2026-10-01',until:'2026-10-08',timezone:'UTC'};
  const query={action:'turns' as const,threadId:'full-task',matchedOnly:true,locateTurnId:'full-turn',scope,limit:1,offset:2};
  const saved=queryInvocation(query,{mode:'cached'});
  assert.deepEqual(saved,{method:'usage',params:{query:query,mode:'cached'}});
  const live=queryInvocation({...query},{mode:'cached'});
  assert.deepEqual(live,{method:'usage',params:{query:{...query},mode:'cached'}});
  assert.deepEqual(cliInvocation(live),{args:['system','call'],input:JSON.stringify(live)});
  const human=['usage','stats','--json'];assert.deepEqual(cliInvocation(human),{args:human});
});

test('verification transport loads in native Node without a TypeScript loader or build',()=>{
  const moduleUrl=new URL('./cli-invocation.ts',import.meta.url).href;
  const run=spawnSync(process.execPath,['--input-type=module','-e',`const {cliInvocation}=await import(${JSON.stringify(moduleUrl)});process.stdout.write(cliInvocation({method:'config',params:{action:'capabilities'}}).args[0]);`],{encoding:'utf8',timeout:5000,maxBuffer:4096});
  assert.ifError(run.error);assert.equal(run.status,0,run.stderr);assert.equal(run.stdout,'system');
});
