import assert from 'node:assert/strict';
import {test} from 'node:test';
import type {HistoryRead,UsageResult} from '@wombat/client';
import {withLocalProduct,row} from '../helpers/local-product.js';

test('one operation read recovers its request and result, retains scope and keeps later recovery inspectable', {timeout:90000}, async()=>{
 await withLocalProduct(async f=>{
  const at=(second:number)=>`2026-10-28T00:00:${String(second).padStart(2,'0')}Z`;
  const command=(type:string,turn:string,second:number,id:string,args:string[],code?:number)=>row('event_msg',{
   type,turn_id:turn,item:{type:'CommandExecution',id,command:args,cwd:f.project,source:'unified_exec_startup',parsed_cmd:[],status:code===undefined?'running':code?'failed':'completed',...(code===undefined?{}:{exit_code:code,aggregated_output:code?'SYNTHETIC_EXPECTED_NEGATIVE':'SYNTHETIC_RECOVERY_OK'})},
  },at(second));
  await f.write('task',[
   row('session_meta',{id:'task',cwd:f.project},at(0)),
   row('turn_context',{turn_id:'first',cwd:f.project},at(1)),
   row('event_msg',{type:'user_message',message:'Run the expected negative test first; then use python3 for the real validation.',turn_id:'first'},at(1)),
   row('event_msg',{type:'token_usage_record',turn_id:'first',response_id:'m',usage:{input_tokens:120,cached_input_tokens:0,output_tokens:0,total_tokens:120}},at(1)),
   command('item_started','first',2,'negative',['python','negative_test.py']),
   command('item_completed','first',3,'negative',['python','negative_test.py'],1),
   row('event_msg',{type:'agent_message',message:'The first error is an expected negative test. Next use python3.',turn_id:'first'},at(4)),
   row('turn_context',{turn_id:'second',cwd:f.project},at(5)),
   row('event_msg',{type:'user_message',message:'Continue the selected validation with python3; keep the original scope.',turn_id:'second'},at(5)),
   command('item_started','second',6,'recovery',['python3','-m','unittest']),
   command('item_completed','second',7,'recovery',['python3','-m','unittest'],0),
   row('event_msg',{type:'agent_message',message:'The selected validation passed; this does not establish a live plugin load.',turn_id:'second'},at(8)),
   row('response_item',{type:'function_call',call_id:'outer',name:'synthetic_tool',arguments:JSON.stringify({command:'make smoke-publish'})},at(9)),
   row('response_item',{type:'function_call_output',call_id:'outer',status:'completed',output:'make: *** [smoke-publish] Error 1'},at(10)),
  ]);
  const before=(await f.client.live!({mode:'fresh',query:{action:'threads',roots:[f.root],scope:{allTime:true}}})).result;
  const thread=before.items.find(i=>i.kind==='thread');assert.ok(thread);
  const turns=(await f.client.live!({mode:'cached',query:{action:'turns',threadId:thread.id,roots:[f.root],scope:{allTime:true},limit:20}})).result;
  const ordered=turns.items.filter(i=>i.kind==='turn').sort((a,b)=>(a.ordinal??0)-(b.ordinal??0));assert.equal(ordered.length,2);
  const steps=async(turnId:string)=>(await f.client.live!({mode:'cached',query:{action:'steps',threadId:thread.id,turnId,roots:[f.root],scope:{allTime:true}}})).result.items.filter(i=>i.kind==='operation');
  const failed=(await steps(ordered[0].id)).find(i=>i.status==='failed');assert.ok(failed);
  const successes=await steps(ordered[1].id),success=successes.find(i=>i.name==='exec_command');assert.ok(success);assert.equal(success.status,'completed');
  const read=(operation:string,turn:string):HistoryRead=>JSON.parse(f.cli(['history','read',thread.id,'--turn',turn,'--operation',operation,'--root',f.root,'--json'],'',0));
  const failure=read(failed.id,ordered[0].id),recovered=read(success.id,ordered[1].id);
  assert.equal(failure.result.fragments.length,2);assert.equal(recovered.result.fragments.length,2);
  assert.match(failure.result.fragments.map(f=>f.text).join('\n'),/python\nnegative_test.py[\s\S]*SYNTHETIC_EXPECTED_NEGATIVE/);
  assert.match(recovered.result.fragments.map(f=>f.text).join('\n'),/python3\n-m\nunittest[\s\S]*SYNTHETIC_RECOVERY_OK/);
  assert.ok(recovered.result.fragments.every(f=>f.message.operationId===success.id&&f.locations.length>0));
  const plain:HistoryRead=JSON.parse(f.cli(['history','read',thread.id,'--turn',ordered[0].id,'--root',f.root,'--find','expected negative test','--json'],'',0));
  assert.match(plain.result.fragments[0].text,/expected negative test/);assert.equal(plain.result.fragments[0].message.kind,'message');
  const outer=successes.find(i=>i.name==='synthetic_tool');assert.ok(outer);assert.equal(outer.status,'completed');
  const outerRead=read(outer.id,ordered[1].id);assert.equal(outerRead.result.fragments.length,2);assert.match(outerRead.result.fragments[0].text,/make smoke-publish/);assert.match(outerRead.result.fragments[1].text,/Error 1/);
  // A source-level success is a fact; the inner command error remains source evidence for Codex.
  const selection={includeToolOutput:true,threads:[thread.id],turns:[ordered[1].id],operationId:success.id};
  const first=await f.client.conversation!({action:'locate',roots:[f.root],selection,limit:1});assert.equal(first.items.length,1);assert.ok(first.nextCursor);
  const next=await f.client.conversation!({action:'locate',roots:[f.root],selection,limit:1,cursor:first.nextCursor});assert.equal(next.items.length,1);assert.notEqual(first.items[0].evidence.eventId,next.items[0].evidence.eventId);
  await assert.rejects(f.client.conversation!({action:'locate',roots:[f.root],selection:{...selection,operationId:outer.id},cursor:first.nextCursor}),{code:'CURSOR_MISMATCH'});
  await assert.rejects(f.client.conversation!({action:'locate',roots:[f.root],selection:{...selection,turns:[ordered[0].id]}}),{code:'NOT_FOUND'});
  await assert.rejects(f.client.conversation!({action:'read',roots:[f.root],selection,targets:[{evidence:failure.result.fragments[0].evidence}]}),{code:'NOT_FOUND'});
  const invalid=JSON.parse(f.cli({method:'conversation',params:{action:'locate',roots:[f.root],selection:{operationId:success.id}}},'',1)).error;
  assert.equal(invalid.details.fields[0].path,'/selection/operationId');
  assert.equal(JSON.parse(f.cli(['history','read',thread.id,'--operation',success.id,'--json'],'',1)).error.code,'INVALID_ARGUMENT');
  const dated:HistoryRead=JSON.parse(f.cli(['history','read',thread.id,'--turn',ordered[1].id,'--operation',success.id,'--root',f.root,'--since','2026-10-29','--until','2026-10-30','--timezone','UTC','--json'],'',0));
  assert.equal(dated.result.fragments.length,0);assert.equal(dated.result.scanComplete,true);
  const after:UsageResult=(await f.client.live!({mode:'cached',query:{action:'threads',roots:[f.root],scope:{allTime:true}}})).result;
  assert.deepEqual(after.summary,before.summary);
 });
});

test('completion-only commands retain their request and large result with operation-bound continuation', {timeout:60000}, async()=>{
 await withLocalProduct(async f=>{
  const at='2026-10-28T00:00:00Z';
  await f.write('task',[
   row('session_meta',{id:'task',cwd:f.project},at),
   row('turn_context',{turn_id:'turn',cwd:f.project},at),
   row('event_msg',{type:'item_completed',turn_id:'turn',item:{type:'CommandExecution',id:'only',command:['python3','validate.py'],cwd:f.project,source:'unified_exec_startup',parsed_cmd:[],status:'completed',exit_code:0,aggregated_output:'RESULT_BEGIN '+'中文'.repeat(10000)+' LATE_RESULT_MARKER'}},at),
  ]);
  const threads=(await f.client.live!({mode:'fresh',query:{action:'threads',roots:[f.root],scope:{allTime:true}}})).result;
  const thread=threads.items.find(i=>i.kind==='thread');assert.ok(thread);
  const turns=(await f.client.live!({mode:'cached',query:{action:'turns',threadId:thread.id,roots:[f.root],scope:{allTime:true}}})).result;
  const turn=turns.items.find(i=>i.kind==='turn');assert.ok(turn);
  const steps=(await f.client.live!({mode:'cached',query:{action:'steps',threadId:thread.id,turnId:turn.id,roots:[f.root],scope:{allTime:true}}})).result;
  const op=steps.items.find(i=>i.kind==='operation');assert.ok(op);
  const args=['history','read',thread.id,'--turn',turn.id,'--operation',op.id,'--root',f.root];
  const first:HistoryRead=JSON.parse(f.cli([...args,'--max-bytes','128','--json']));
  const fragment=first.result.fragments[0];assert.match(fragment.text,/python3\nvalidate.py/);assert.ok(fragment.nextCursor);assert.equal(fragment.message.status,'completed');
  const next:HistoryRead=JSON.parse(f.cli([...args,'--event',fragment.evidence.eventId,'--event-source',fragment.evidence.sourceInstanceId,'--event-offset',String(fragment.evidence.eventOffset),'--event-turn',turn.id,'--cursor',fragment.nextCursor,'--max-bytes','128','--json']));
  assert.match(next.result.fragments[0].text,/RESULT_BEGIN/);assert.equal(next.result.fragments[0].message.operationId,op.id);
  const late:HistoryRead=JSON.parse(f.cli([...args,'--find','LATE_RESULT_MARKER','--json']));
  assert.match(late.result.fragments[0].text,/LATE_RESULT_MARKER/);
  assert.equal(JSON.parse(f.cli(['history','read',thread.id,'--turn',turn.id,'--root',f.root,'--find','LATE_RESULT_MARKER','--json'])).result.fragments.length,0);
 });
});
