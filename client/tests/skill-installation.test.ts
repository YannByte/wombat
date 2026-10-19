import {test} from 'node:test';
import assert from 'node:assert/strict';
import {mkdtemp,mkdir,readFile,writeFile,rm,symlink,cp} from 'node:fs/promises';
import os from 'node:os';import path from 'node:path';
import {manageSkill} from '@wombat/client/node';
import {skillRuntimeChecks} from '../src/node/skill-installation.js';
import {nativeCodexFixture} from '../../tests/helpers/native-codex.js';

test('standalone installation protects ownership, modified content, links and source integrity',async()=>{
  const root=await mkdtemp(path.join(os.tmpdir(),'wombat-skill-install-'));
  try{
    const native=await nativeCodexFixture(root),directory=path.join(root,'installed/wombat');
    const options={resourcesPath:path.resolve('..','dist/skill'),codexBinaryPath:native.binary};const input={directory,cwd:root};
    const installed=await manageSkill('install',input,{},options);
    assert.equal(installed.status,'installed');assert.equal(installed.dataStatus,'not_requested');assert.equal(installed.discovery.status,'available');
    await assert.rejects(manageSkill('install',input,{},options),{code:'SKILL_INSTALL_CONFLICT'});
    await manageSkill('install',{...input,replace:true},{},options);
    const corrupt=path.join(root,'corrupt');await cp(options.resourcesPath,corrupt,{recursive:true});await writeFile(path.join(corrupt,'wombat/SKILL.md'),'corrupt');
    const before=await readFile(path.join(directory,'SKILL.md'),'utf8');
    await assert.rejects(manageSkill('install',{...input,replace:true},{},{...options,resourcesPath:corrupt}),{code:'SKILL_RESOURCE_CHANGED'});
    assert.equal(await readFile(path.join(directory,'SKILL.md'),'utf8'),before);
    const entry=path.join(directory,'SKILL.md');await writeFile(entry,'user customization');
    assert.equal((await manageSkill('status',input,{},options)).status,'modified');
    for(const action of ['install','uninstall'] as const)await assert.rejects(manageSkill(action,{...input,replace:true},{},options),{code:'SKILL_INSTALL_CONFLICT'});
    assert.equal(await readFile(entry,'utf8'),'user customization');
    await rm(directory,{recursive:true});await manageSkill('install',input,{},options);
    await manageSkill('uninstall',input,{},options);assert.equal((await manageSkill('status',input,{},options)).status,'absent');
    await mkdir(directory);await writeFile(entry,'custom');
    await assert.rejects(manageSkill('uninstall',input,{},options),{code:'SKILL_INSTALL_CONFLICT'});
    await rm(directory,{recursive:true});const target=path.join(root,'target');await mkdir(target);await symlink(target,directory,process.platform==='win32'?'junction':'dir');
    await assert.rejects(manageSkill('install',{...input,replace:true},{},options),{code:'SKILL_INSTALL_CONFLICT'});
    await writeFile(path.join(target,'preserved'),'yes');await assert.rejects(manageSkill('uninstall',input,{},options),{code:'SKILL_INSTALL_CONFLICT'});
    assert.equal(await readFile(path.join(target,'preserved'),'utf8'),'yes');
    await native.waitForExit();
  }finally{await rm(root,{recursive:true,force:true});}
});


test('runtime checks bind the discovered copy to capabilities and content rather than matching version alone',async()=>{
 const root=await mkdtemp(path.join(os.tmpdir(),'wombat-runtime-check-'));
 try {
  const plugin=path.join(root,'plugin');await cp(path.resolve('../dist/skill/plugin'),plugin,{recursive:true});
  const entry=path.join(plugin,'skills/wombat/SKILL.md'),file=path.join(plugin,'wombat-runtime.json');
  const discovery={status:'available' as const,instances:[{name:'wombat:wombat',path:entry,enabled:true}],errorCode:null};
  const original=JSON.parse(await readFile(file,'utf8'));
  const read=async()=>(await skillRuntimeChecks(discovery))[0];
  assert.equal((await read()).status,'compatible');
  const task=path.join(plugin,'skills/wombat-task-retro/SKILL.md');
  const taskBody=await readFile(task,'utf8');await writeFile(task,'Synthetic task customization');
  assert.equal((await read()).status,'modified','Changes to another shipped Skill must invalidate the bundle');
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
