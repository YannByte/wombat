import assert from 'node:assert/strict';
import {test} from 'node:test';
import {mkdir,writeFile,readFile,rm} from 'node:fs/promises';
import path from 'node:path';
import type {ProjectResult,OptimizeResult} from '@wombat/client';
import {withLocalProduct,row} from '../helpers/local-product.js';

test('project and global Skill repairs retain fresh scope, stable resource identity and project isolation',{timeout:120000},async()=>{
 await withLocalProduct(async f=>{
  const global=path.join(f.root,'skills/global/SKILL.md'),local=path.join(f.project,'.agents/skills/local/SKILL.md'),target=path.join(path.dirname(local),'references/setup.md');
  const managed=path.join(f.root,'plugins/cache/provider/presentations/SKILL.md');
  const other=path.join(f.dir,'project-b');await mkdir(other);const otherFile=path.join(other,'keep.txt');await writeFile(otherFile,'Synthetic project B stays unchanged\n');
  for(const file of [global,local,managed])await mkdir(path.dirname(file),{recursive:true});
  await writeFile(global,'---\nname: Invalid_Name\ndescription: Synthetic global skill\n---\nGlobal body.\n');
  await writeFile(local,'---\nname: local\ndescription: Synthetic local skill\n---\nSee [setup](references/setup.md).\n');
  await writeFile(managed,'---\nname: Presentations\ndescription: Synthetic managed plugin\n---\nManaged body.\n');
  const native=path.join(f.dir,'native.cjs');
  const skills=[{name:'global',description:'Synthetic global skill',path:global,enabled:true,scope:'user'},{name:'presentations',description:'Synthetic managed plugin',path:managed,enabled:true,scope:'system'}];
  await writeFile(native,`if(process.argv.includes('--version')){console.log('codex-cli 0.146.0');process.exit(0);}let text='';process.stdin.on('data',chunk=>{text+=chunk;let end;while((end=text.indexOf('\\n'))>=0){const r=JSON.parse(text.slice(0,end));text=text.slice(end+1);if(r.id!=null)console.log(JSON.stringify({id:r.id,result:r.method==='skills/list'?{data:r.params.cwds.map(cwd=>({cwd,errors:[],skills:${JSON.stringify(skills)}}))}:{}}));}});`);
  const saved=process.env.WOMBAT_CODEX_BIN;process.env.WOMBAT_CODEX_BIN=native;
  try{
   await f.write('task',[row('session_meta',{id:'task',cwd:f.project},'2026-10-28T00:00:00Z'),row('turn_context',{turn_id:'turn',cwd:f.project},'2026-10-28T00:00:01Z'),row('event_msg',{type:'agent_message',message:'Synthetic current work',turn_id:'turn'},'2026-10-28T00:00:02Z')]);
   const check=():ProjectResult=>JSON.parse(f.cli(['check','--project',f.project,'--root',f.root,'--json'],'',2));
   const before=check(),file=path.join(f.dir,'before.json');await writeFile(file,JSON.stringify(before));
   const source=before.checks.result;assert.ok(source);
   const original=source.suggestions.find(s=>s.item.path===global);assert.ok(original);
   const reference=source.suggestions.find(s=>s.item.path===local&&s.findings.some(f=>f.rule==='localReference'));assert.ok(reference);
   const refFinding=reference.findings.find(f=>f.rule==='localReference');assert.ok(refFinding);assert.ok(refFinding.identity.findingId);
   const plugin=source.suggestions.find(s=>s.item.path===managed);assert.ok(plugin);
   assert.equal(plugin.item.project,null);assert.ok(plugin.item.authorizedProjects.includes(f.project));
   assert.equal(plugin.category,'review');assert.equal(plugin.findings[0].status,'advisory');
   const verify=(id:string,baseline=file,args:string[]=[]):OptimizeResult=>JSON.parse(f.cli(['check','verify','--baseline',baseline,'--suggestion',id,...args,'--json'],'',2));
   for(const extra of [[],['--project',f.project]]){
    const same:OptimizeResult['checks'][number]|undefined=verify(original.id,file,extra).checks.find(c=>c.rule==='skillFormat');assert.ok(same);
    assert.equal(same.outcome,'hit');assert.equal(same.comparison.status,'comparable');
    assert.deepEqual(same.basis.scope,original.checks.find(c=>c.rule==='skillFormat')?.basis.scope);
   }
   const sub=path.join(f.dir,'checks.json');await writeFile(sub,JSON.stringify(source));
   assert.equal(verify(original.id,sub).checks.find(c=>c.rule==='skillFormat')?.comparison.status,'comparable');
   await mkdir(path.dirname(target));await writeFile(target,'Synthetic setup\n');
   assert.ok(!check().checks.result?.suggestions.some(s=>s.item.path===local&&s.findings.some(f=>f.rule==='localReference')));
   const fixed=verify(reference.id).checks.find(c=>c.rule==='localReference');assert.ok(fixed);assert.equal(fixed.outcome,'miss');assert.equal(fixed.comparison.status,'comparable');
   await rm(target);
   const restored=verify(reference.id).checks.find(c=>c.rule==='localReference');assert.ok(restored);assert.equal(restored.outcome,'hit');assert.equal(restored.comparison.status,'comparable');
   assert.equal(restored.findings[0].identity.findingId,refFinding.identity.findingId);
   await assert.rejects(f.client.optimize!({projectRoots:[f.project],roots:[f.root],project:other}),{code:'PROJECT_NOT_AUTHORIZED'});
   if(process.platform==='darwin'){
    const alias=f.project.replace(/^\/private\//,'/');
    const viaAlias:ProjectResult=JSON.parse(f.cli(['check','--project',alias,'--root',f.root,'--json'],'',2));
    assert.equal(viaAlias.scope.projectRoot,alias);assert.ok(viaAlias.checks.result);
    assert.equal(viaAlias.checks.result.suggestions.find(s=>s.item.path===global)?.checks.find(c=>c.rule==='skillFormat')?.basis.scope.project,f.project);
    assert.equal(verify(original.id,file,['--project',alias]).checks.find(c=>c.rule==='skillFormat')?.comparison.status,'comparable');
   }
   assert.equal(await readFile(otherFile,'utf8'),'Synthetic project B stays unchanged\n');
   assert.equal(await readFile(managed,'utf8'),'---\nname: Presentations\ndescription: Synthetic managed plugin\n---\nManaged body.\n');
  }finally{if(saved===undefined)delete process.env.WOMBAT_CODEX_BIN;else process.env.WOMBAT_CODEX_BIN=saved;}
 });
});

test('weekly record discovery prunes old frames and continuation treats omitted offset as zero',{timeout:120000},async()=>{
 await withLocalProduct(async f=>{
  const old=[row('session_meta',{id:'long',cwd:f.project},'2026-10-11T00:00:00Z'),row('turn_context',{turn_id:'turn',cwd:f.project},'2026-10-11T00:00:00Z')];
  for(let n=0;n<2500;n++)old.push(row('event_msg',{type:'agent_message',message:'Synthetic old record '+n,turn_id:'turn'},'2026-10-11T00:00:01Z'));
  old.push(row('event_msg',{type:'agent_message',message:'Synthetic selected-week work',turn_id:'turn'},'2026-10-28T00:00:00Z'));
  await f.write('long',old);
  await f.write('second',[row('session_meta',{id:'second',cwd:f.project},'2026-10-29T00:00:00Z'),row('event_msg',{type:'agent_message',message:'Synthetic second work'},'2026-10-29T00:00:01Z')]);
  const request={action:'records' as const,roots:[f.root],selection:{projects:[f.project],since:'2026-10-26',until:'2026-11-02',timezone:'Asia/Shanghai'},limit:100};
  const found=await f.client.conversation!(request);
  assert.equal(found.items.length,2);assert.equal(found.scanComplete,true);assert.ok(found.scannedEvents<2000);
  const first=await f.client.conversation!({...request,limit:1,offset:0});assert.ok(first.nextCursor);
  const next=await f.client.conversation!({...request,limit:1,cursor:first.nextCursor});
  assert.equal(next.items.length,1);assert.notEqual(first.items[0].evidence.threadId,next.items[0].evidence.threadId);assert.equal(next.scanComplete,true);
  const mismatch=JSON.parse(f.cli({method:'conversation',params:{...request,cursor:first.nextCursor,selection:{...request.selection,timezone:'UTC'}}},'',1)).error;
  assert.equal(mismatch.code,'CURSOR_MISMATCH');assert.equal(mismatch.details.fields[0].path,'/cursor');assert.equal(mismatch.recovery,'retry_same_scope');
 });
});

test('inherited execution copies remain source-visible without new endpoints; partial checks expose a bounded local result',{timeout:120000},async()=>{
 await withLocalProduct(async f=>{
  const command=(type:string,at:string,status:string,id='cmd',source='unified_exec_startup')=>row('event_msg',{type,turn_id:'turn',item:{type:'CommandExecution',id,command:['echo','synthetic'],cwd:f.project,source,parsed_cmd:[],status,...(status==='completed'?{exit_code:0}:{})}},at);
  const start=command('item_started','2026-10-28T00:00:02Z','running'),end=command('item_completed','2026-10-28T00:00:03Z','completed');
  await f.write('task',[
   row('session_meta',{id:'task',cwd:f.project},'2026-10-28T00:00:00Z'),row('turn_context',{turn_id:'turn',cwd:f.project},'2026-10-28T00:00:01Z'),row('event_msg',{type:'task_started',turn_id:'turn'},'2026-10-28T00:00:01Z'),
   start,end,{...start,timestamp:'2026-10-28T00:00:08Z',metadata:{inherited_user_message:true}},{...end,timestamp:'2026-10-28T00:00:09Z',metadata:{inherited_user_message:true}},
   command('item_completed','2026-10-28T00:00:10Z','completed','user','user_shell'),
   row('event_msg',{type:'task_complete',turn_id:'turn'},'2026-10-28T00:00:12Z'),
  ]);
  const rows=await f.client.live!({mode:'fresh',query:{action:'threads',roots:[f.root],scope:{allTime:true}}});const thread=rows.result.items.find(i=>i.kind==='thread');assert.ok(thread);
  const turns=await f.client.live!({mode:'cached',query:{action:'turns',threadId:thread.id,roots:[f.root],scope:{allTime:true}}});const turn=turns.result.items.find(i=>i.kind==='turn');assert.ok(turn);
  const report=await f.client.optimize!({action:'activity',roots:[f.root],activity:{threadId:thread.id,turnId:turn.id}});assert.ok(report.activity);
  assert.equal(report.activity.operationDiagnostics?.total,0);assert.equal(report.activity.coverage.conflictingOperations.value,0);assert.equal(report.activity.coverage.eligibleCommands.value,1);
  const repeated=report.activity.checks.find(c=>c.rule==='inspect_repeated_requests');assert.ok(repeated);assert.equal(repeated.observed.value,0);assert.equal(repeated.outcome,'insufficient');
  assert.equal(repeated.observedSubset?.outcome,'miss');assert.equal(repeated.observedSubset?.scope,'eligible_recorded_commands');assert.equal(repeated.observedSubset?.eligibleCommands,1);
  const evidence=await f.client.conversation!({action:'activity',roots:[f.root],selection:{threads:[thread.id],turns:[turn.id]},limit:200});
  assert.equal(evidence.items.filter(i=>i.kind==='operation'&&i.origin==='inherited').length,2);
 });
});
