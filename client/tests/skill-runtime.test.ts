import {test} from 'node:test';
import assert from 'node:assert/strict';
import {mkdtemp,readFile,writeFile,rm,cp} from 'node:fs/promises';
import os from 'node:os';import path from 'node:path';
import {skillRuntimeChecks} from '../src/node/skill-runtime.js';
test('runtime checks bind the discovered copy to capabilities and content rather than matching version alone',async()=>{
 const root=await mkdtemp(path.join(os.tmpdir(),'wombat-runtime-check-'));
 try {
  const plugin=path.join(root,'plugin');await cp(path.resolve('../dist/skill/plugin'),plugin,{recursive:true});
  const entry=path.join(plugin,'skills/wombat/SKILL.md'),file=path.join(plugin,'wombat-runtime.json');
  const discovery={status:'available' as const,instances:[{name:'wombat:wombat',path:entry,enabled:true}],errorCode:null};
  const original=JSON.parse(await readFile(file,'utf8'));
  const read=async()=>(await skillRuntimeChecks(discovery))[0];
  assert.equal((await read()).status,'compatible');
  const task=path.join(plugin,'skills/wombat/references/diagnose.md');
  const taskBody=await readFile(task,'utf8');await writeFile(task,'Synthetic task customization');
  assert.equal((await read()).status,'modified','Changes to a task flow must invalidate the bundle');
  await writeFile(task,taskBody);assert.equal((await read()).status,'compatible');
  for(const skills of [[...original.skills,original.skills[0]],[{name:'../other',contentHash:'a'.repeat(64)}],[]]){
    await writeFile(file,JSON.stringify({...original,skills}));assert.equal((await read()).errorCode,'SKILL_MANIFEST_INVALID');
  }
  await writeFile(file,JSON.stringify(original));
  await writeFile(file,JSON.stringify({...original,version:'0.1.0'}));assert.equal((await read()).status,'compatible');
  await writeFile(file,JSON.stringify({...original,requiredCapabilities:['future-capability']}));
  const incompatible=await read();assert.equal(incompatible.status,'incompatible');assert.deepEqual(incompatible.missingCapabilities,['future-capability']);
  await writeFile(file,JSON.stringify(original));await writeFile(entry,'Synthetic local customization');
  assert.equal((await read()).status,'modified');
  await writeFile(file,JSON.stringify({...original,format:999}));assert.equal((await read()).errorCode,'SKILL_MANIFEST_INVALID');
  await rm(file);assert.equal((await read()).status,'unmanaged');
  assert.deepEqual(await skillRuntimeChecks({...discovery,instances:[{...discovery.instances[0],enabled:false}]}),[]);
  const controller=new AbortController();controller.abort();await assert.rejects(skillRuntimeChecks(discovery,{signal:controller.signal}),{code:'CANCELLED'});
 } finally {await rm(root,{recursive:true,force:true});}
});
