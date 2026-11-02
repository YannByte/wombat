import test from 'node:test';
import assert from 'node:assert/strict';
import {spawnSync} from 'node:child_process';
import {readFile,readdir,writeFile} from 'node:fs/promises';
import path from 'node:path';
import {withLocalProduct,row,measurement} from '../helpers/local-product.js';
import type {ConversationResult} from '@wombat/client';

test('fresh tool calls need no service or view and preserve scoped continuation after unrelated updates',{timeout:60000},async()=>{
 await withLocalProduct(async f=>{
  const at='2026-10-14T00:00:00Z';
  await f.write('task',[row('session_meta',{id:'task',cwd:f.project},at),row('turn_context',{turn_id:'turn',model:'gpt-5.4'},at),row('event_msg',{type:'agent_message',turn_id:'turn',message:'First concrete message'},at),row('event_msg',{type:'agent_message',turn_id:'turn',message:'Second concrete message'},at),measurement('task','one',110,at)]);
  // Identity resolution itself can prepare a new installation; no setup/refresh command.
  const identity=await f.client.conversation!({action:'resolve',query:'task'});
  assert.ok(identity.record);assert.equal(identity.scannedEvents,0);assert.equal(identity.sourceBytesRead,0);
  const selected={threads:[identity.record!.id]};
  const first=await f.client.conversation!({action:'locate',selection:selected,limit:1});
  assert.equal(first.items.length,1);assert.ok(first.nextCursor);
  const before=await f.client.live!({query:{action:'usage',scope:{allTime:true},compact:true},mode:'cached'});
  assert.equal(before.result.summary.tokens.total,110);
  await f.write('other',[row('session_meta',{id:'other',cwd:f.project},at),row('turn_context',{turn_id:'turn',model:'gpt-5.4'},at),measurement('other','two',220,at)]);
  const updated=await f.client.live!({query:{action:'usage',scope:{allTime:true},compact:true},mode:'fresh',also:[{action:'statistics',scope:{allTime:true},compact:true}]});
  assert.equal(updated.result.summary.tokens.total,330);assert.notEqual(updated.freshness.revision,before.freshness.revision);
  const next=await f.client.conversation!({action:'locate',selection:selected,limit:1,cursor:first.nextCursor});
  assert.equal(next.items.length,1);assert.equal(next.scanComplete,true);assert.notEqual(next.items[0].evidence.eventId,first.items[0].evidence.eventId);
  const text=await f.client.conversation!({action:'read',selection:selected,targets:[{evidence:next.items[0].evidence,maxBytes:128}]});
  assert.equal(text.fragments[0].text,'Second concrete message');
  const api=JSON.parse(f.cli(['system','api']));assert.ok(!api.methods.includes('service'));assert.ok(!api.methods.includes('snapshot'));
  const usageSchema=JSON.parse(f.cli(['system','api','usage']));assert.ok(!JSON.stringify(usageSchema.inputSchema).includes('"snapshotId"'));
  const files=await readdir(f.data,{recursive:true});assert.ok(!files.some(file=>/endpoint|service-registration|\.sock$/.test(file)));
  const database=path.join(f.data,'live-v2/index.sqlite'),retained=await readFile(database);
  const rejected=spawnSync(process.execPath,[f.entry,'usage','--snapshot','old','--json'],{env:process.env,encoding:'utf8',timeout:5000});
  assert.equal(rejected.status,1);assert.equal(JSON.parse(rejected.stdout).error.code,'INVALID_ARGUMENT');assert.deepEqual(await readFile(database),retained);
  const binary=path.resolve('dist',process.platform==='win32'?'wombat-core.exe':'wombat-core');
  for(const flag of ['--serve-usage','--daemon','--daemon-config']){
   const result=spawnSync(binary,[flag],{env:process.env,encoding:'utf8',input:'',timeout:5000});assert.ifError(result.error);assert.equal(JSON.parse(result.stdout).code,'INVALID_ARGUMENT');
  }
 });
});

test('cleanup baselines cross process boundaries without expiry or an index',{timeout:45000},async()=>{
 await withLocalProduct(async f=>{
  await writeFile(path.join(f.project,'temporary.txt'),'selected temporary data');
  const before=await f.client.artifacts!({action:'inspect',projectRoot:f.project,paths:['temporary.txt']});
  const file=path.join(f.dir,'cleanup.json');await writeFile(file,JSON.stringify(before));
  const raw=f.cli(['clean','verify','--plan',file,'--item',before.items[0].id,'--json'],'',before.partial?2:0);
  const after=JSON.parse(raw);assert.equal(after.items[0].id,before.items[0].id);assert.equal(after.items[0].recheck,'unchanged');
  assert.equal('readView' in after,false);assert.equal('expiresAt' in after,false);
  const entries=await readdir(f.data,{recursive:true}).catch((e:NodeJS.ErrnoException)=>{if(e.code==='ENOENT')return [];throw e;});
  assert.ok(!entries.some(v=>v.endsWith('.sqlite')));
 });
});

test('cancelling a real staged rebuild cleans pending data and retains the committed index',{timeout:60000},async()=>{
 await withLocalProduct(async f=>{
  const at='2026-10-14T00:00:00Z';
  const head=[row('session_meta',{id:'task',cwd:f.project},at),row('turn_context',{turn_id:'turn',model:'gpt-5.4'},at)];
  await f.write('task',[...head,measurement('task','original',110,at)]);
  const before=await f.client.live!({query:{action:'usage',scope:{allTime:true}},mode:'fresh'});
  await f.write('task',[...head,measurement('task','original',110,at),...Array.from({length:30000},(_,n)=>measurement('task','extra-'+n,1,at))]);
  const controller=new AbortController();let finished=false;
  const work=f.client.live!({query:{action:'usage',scope:{allTime:true}},mode:'fresh',rebuild:true},{signal:controller.signal}).finally(()=>{finished=true;});
  const outcome=work.then(()=>null,error=>error);
  let pending=false;
  for(let n=0;n<500&&!finished;n++){
   const files=await readdir(path.join(f.data,'live-v2'),{recursive:true});
   if(files.some(name=>name.endsWith('.wombat-pending-index-v1'))){pending=true;break;}
   await new Promise(resolve=>setTimeout(resolve,10));
  }
  assert.ok(pending,'test must interrupt an actual unpublished generation');controller.abort();
  const error=await outcome;assert.equal(error?.code,'CANCELLED');
  const files=await readdir(path.join(f.data,'live-v2'),{recursive:true});
  assert.ok(!files.some(name=>name.endsWith('.wombat-pending-index-v1')),'abort resolves only after owned pending data is removed');
  const retained=await f.client.live!({query:{action:'usage',scope:{allTime:true}},mode:'cached'});
  assert.equal(retained.result.summary.tokens.total,110);assert.equal(retained.freshness.revision,before.freshness.revision);
 });
});
