import test from 'node:test';
import assert from 'node:assert/strict';
import {mkdir,writeFile,readFile,rm,readdir,open} from 'node:fs/promises';
import {spawnSync} from 'node:child_process';
import path from 'node:path';
import {withLocalProduct} from '../helpers/local-product.js';
import type {ArtifactsResult,AgentRequest} from '@wombat/client';

test('assembled cleanup is read-only, retains original selection, protects assets and rechecks authorized removal', {timeout:45000}, async()=>{
 await withLocalProduct(async f=>{
  const invoke=(input:string[]|AgentRequest)=>{const a=Array.isArray(input)?input:['system','call'];const r=spawnSync(process.execPath,[f.entry,...a],{env:process.env,input:Array.isArray(input)?'':JSON.stringify(input),encoding:'utf8',timeout:15000});assert.ifError(r.error);assert.ok(r.status===0||r.status===2,r.stderr+r.stdout);const value=JSON.parse(r.stdout) as ArtifactsResult;assert.equal(r.status,value.partial?2:0);return value;};
  assert.equal(spawnSync('git',['init',f.project],{encoding:'utf8',timeout:5000}).status,0);
  await writeFile(path.join(f.project,'.gitignore'),'target/\n');
  await writeFile(path.join(f.project,'Cargo.toml'),"[package]\nname='synthetic'\nversion='0.1.0'\n");
  const output=path.join(f.project,'target');await mkdir(output);
  await writeFile(path.join(output,'CACHEDIR.TAG'),'Signature: 8a477f597d28d172789f06886806bc55\n');
  await writeFile(path.join(output,'object.o'),'synthetic compile output');
  const before=await f.client.artifacts!({action:'inspect',projectRoot:f.project});
  assert.equal(before.items.length,1);
  assert.notEqual(before.items[0].assessment,'protected');
  assert.ok(['candidate','unknown'].includes(before.items[0].assessment));
  assert.equal(before.items[0].regeneration,'cargo_cache');
  assert.equal(await readFile(path.join(output,'object.o'),'utf8'),'synthetic compile output');
  const entries=await readdir(f.data,{recursive:true}).catch((e:NodeJS.ErrnoException)=>{if(e.code==='ENOENT')return [];throw e;});
  assert.ok(!entries.some(v=>/\.sqlite(?:-|$)|ledger\.json$|manifest\.json$/.test(v)),'cleanup does not build a session index or saved snapshot');
  const held=await open(path.join(output,'object.o'),'r');
  try {const busy=await f.client.artifacts!({action:'inspect',projectRoot:f.project});assert.ok(['protected','unknown'].includes(busy.items[0].assessment));assert.ok(['in_use','unknown'].includes(busy.items[0].useState));} finally {await held.close();}
  const q={action:'recheck' as const,baseline:before,selection:[before.items[0].id]};
  const cli=invoke({method:'artifacts',params:q});
  assert.equal('readView' in cli,false);
  assert.deepEqual(cli.items.map(i=>i.id),before.items.map(i=>i.id));
  await writeFile(path.join(output,'MEMORY.md'),'user asset');
  const protectedResult=await f.client.artifacts!(q);assert.equal(protectedResult.items[0].assessment,'protected');
  await rm(path.join(output,'MEMORY.md'));
  const selected=invoke(['clean','--project',f.project,'--path','target','--json']);
  assert.equal(selected.items[0].path,output);
  // The host removes only this test-owned fixture, never through a product mutation API.
  await rm(output,{recursive:true});
  const after=await f.client.artifacts!(q);
  assert.equal(after.items[0].recheck,'gone');assert.equal(after.items[0].assessment,'absent');assert.equal(after.partial,false);
  await assert.rejects(f.client.artifacts!({...q,selection:['foreign']}),{code:'INVALID_ARGUMENT'});
  const generation=path.join(f.data,'usage-v4/generations/00000000-0000-4000-8000-000000000001');const stage=path.join(generation,'.pending-synthetic');await mkdir(stage,{recursive:true});await writeFile(path.join(stage,'stage.bin'),'potential recovery data');await mkdir(path.join(generation,'committed'));await writeFile(path.join(f.data,'usage-v4/refresh.lock'),'');
  const product=await f.client.artifacts!({action:'inspect',projectRoot:f.project,productTemporary:true});const pending=product.items.find(i=>i.path===stage)!;assert.ok(pending,JSON.stringify({expected:stage,items:product.items.map(i=>({path:i.path,kind:i.kind}))}));assert.equal(pending.kind,'product_temporary');assert.equal(pending.owner,f.data);assert.ok(['review','unknown'].includes(pending.assessment));assert.ok(!product.items.some(i=>i.path.endsWith('committed')));
  await mkdir(output);await writeFile(path.join(output,'replacement.o'),'replacement');
  const replaced=await f.client.artifacts!(q);assert.ok(['changed','unavailable'].includes(replaced.items[0].recheck!));
 });
});

test('Codex cleanup uses the real daemon lock and only the original selected obsolete release', {timeout:45000,skip:process.platform!=='darwin'},async()=>{
 await withLocalProduct(async f=>{
  const daemon=path.join(f.root,'packages/app-server-daemon'),releases=path.join(daemon,'releases');
  for(const v of ['1.0.0-darwin-arm64','2.0.0-darwin-arm64','3.0.0-darwin-arm64']){await mkdir(path.join(releases,v),{recursive:true});await writeFile(path.join(releases,v,'runtime'),'synthetic');}
  const {symlink}=await import('node:fs/promises');await symlink(path.join(releases,'2.0.0-darwin-arm64'),path.join(daemon,'current'));await writeFile(path.join(daemon,'auto-update-version'),'3.0.0-darwin-arm64');await writeFile(path.join(daemon,'install.lock'),'');await writeFile(path.join(f.root,'state_5.sqlite'),'persistent state');
  const response=await f.client.artifacts!({action:'inspect',projectRoot:f.project,codex:true,paths:[daemon]});assert.equal(response.outputVersion,3);assert.equal(response.partial,false);
  const old=response.items.find(i=>i.path===path.join(releases,'1.0.0-darwin-arm64'))!;assert.equal(old.assessment,'candidate');assert.equal(old.codexEvidence!.currentRelease,'2.0.0-darwin-arm64');assert.equal(old.codexEvidence!.pendingRelease,'3.0.0-darwin-arm64');
  const request={method:'artifacts' as const,params:{action:'apply' as const,baseline:response,selection:[old.id]}};
  const result=JSON.parse(f.cli(request)) as ArtifactsResult;assert.equal(result.items[0].removal,'removed');assert.equal(result.items[0].recheck,'gone');
  assert.equal(await readFile(path.join(releases,'2.0.0-darwin-arm64/runtime'),'utf8'),'synthetic');assert.equal(await readFile(path.join(releases,'3.0.0-darwin-arm64/runtime'),'utf8'),'synthetic');assert.equal(await readFile(path.join(f.root,'state_5.sqlite'),'utf8'),'persistent state');
  const recheck=await f.client.artifacts!({...request.params,action:'recheck'});assert.equal(recheck.items[0].recheck,'gone');
  await assert.rejects(f.client.artifacts!({...request.params,selection:response.items.filter(i=>i.assessment==='protected').map(i=>i.id)}),{code:'INVALID_ARGUMENT'});
  const entries=await readdir(f.data,{recursive:true}).catch((e:NodeJS.ErrnoException)=>{if(e.code==='ENOENT')return [];throw e;});assert.ok(!entries.some(p=>/index.*\.sqlite|ledger\.json$/.test(p)),'Codex cleanup does not build a usage index');
 });
});
