import assert from 'node:assert/strict';
import {mkdtemp,writeFile,chmod,rm} from 'node:fs/promises';
import path from 'node:path';import os from 'node:os';import test from 'node:test';
import {queryLocal} from '../src/node/live.js';

test('initial project checks and Skill rechecks capture the same current native scope',async t=>{
 const root=await mkdtemp(path.join(os.tmpdir(),'wombat-host-scope-'));t.after(()=>rm(root,{recursive:true,force:true}));
 const native=path.join(root,'native.cjs'),core=path.join(root,'core.cjs');
 await writeFile(core,`let text='';process.stdin.on('data',chunk=>text+=chunk);process.stdin.on('end',()=>{const {op,args}=JSON.parse(text);console.log(JSON.stringify({ok:true,value:op==='native_hook_context'?{nativeHomeSelected:true,hasDeclarations:false,projects:[${JSON.stringify(root)}],roots:[${JSON.stringify(root)}]}:args}));});`);
 await writeFile(native,`if(process.argv.includes('--version')){console.log('codex-cli 0.146.0');process.exit(0);}let text='';process.stdin.on('data',chunk=>{text+=chunk;let end;while((end=text.indexOf('\\n'))>=0){const r=JSON.parse(text.slice(0,end));text=text.slice(end+1);if(r.id!=null)console.log(JSON.stringify({id:r.id,result:r.method==='skills/list'?{data:[{cwd:r.params.cwds[0],errors:[],skills:[{name:'global',description:'Synthetic global',path:${JSON.stringify(path.join(root,'SKILL.md'))},enabled:true,scope:'user'}]}]}:{}}));}});`);
 await chmod(core,0o700);
 const config={binaryPath:core,codexBinaryPath:native};
 const before=await queryLocal({project:{projectRoot:root,since:'2026-10-26',until:'2026-11-02',timezone:'Asia/Shanghai'}},{},config) as {nativeSkills:{contexts:unknown[]}};
 const after=await queryLocal({optimize:{action:'recheck',projectRoots:[root],project:root}},{},config) as {nativeSkills:{contexts:unknown[]}};
 assert.deepEqual(after.nativeSkills.contexts,before.nativeSkills.contexts);
 assert.equal(after.nativeSkills.contexts.length,1);
});
