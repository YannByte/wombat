import assert from 'node:assert/strict';
import {test} from 'node:test';
import {withLocalProduct,row} from '../helpers/local-product.js';

test('selected task exposes conflict reasons, original failure output and actionable continuation errors', {timeout:60000},async()=>{
 await withLocalProduct(async f=>{
  const native=(type:string,time:string,status:string,code:number,end:number)=>row('event_msg',{type,thread_id:'task',turn_id:'turn',started_at_ms:1000,completed_at_ms:end,item:{type:'CommandExecution',id:'cmd',command:['false'],cwd:f.project,source:'unified_exec_startup',parsed_cmd:[],status,exit_code:code,aggregated_output:code?'SYNTHETIC_DIAGNOSTIC_FAILURE':''}},time);
  await f.write('task',[
   row('session_meta',{id:'task',cwd:f.project},'2026-10-14T00:00:00Z'),
   row('turn_context',{turn_id:'turn',cwd:f.project},'2026-10-14T00:00:01Z'),
   row('event_msg',{type:'task_started',turn_id:'turn'},'2026-10-14T00:00:01Z'),
   native('item_completed','2026-10-14T00:00:02Z','completed',0,2000),
   native('item_completed','2026-10-14T00:00:03Z','failed',1,3000),
   row('event_msg',{type:'task_complete',turn_id:'turn'},'2026-10-14T00:00:04Z'),
  ]);
  const usage=await f.client.live!({mode:'fresh',query:{action:'threads',roots:[f.root],scope:{allTime:true}}});
  const threadItem=usage.result.items.find(i=>i.kind==='thread');assert.ok(threadItem);
  const thread=threadItem.id;
  const turns=(await f.client.live!({mode:'cached',query:{action:'turns',threadId:thread,roots:[f.root],scope:{allTime:true}}})).result;
  const turnItem=turns.items.find(i=>i.kind==='turn');assert.ok(turnItem);
  const turn=turnItem.id;
  const report=JSON.parse(f.cli(['check','--task',thread,'--turn',turn,'--root',f.root,'--json'],'',2));
  const diagnostic=report.activity.operationDiagnostics;
  assert.equal(diagnostic.total,1);assert.equal(diagnostic.complete,true);
  assert.ok(diagnostic.records[0].reasons.includes('time_conflict'));
  assert.ok(diagnostic.records[0].reasons.includes('outcome_conflict'));
  assert.ok(diagnostic.records[0].evidence.length);
  assert.ok(diagnostic.records[0].eventRefs.length);
  const human=f.cli(['check','--task',thread,'--turn',turn,'--root',f.root,'--lang','zh'],'',2);
  assert.match(human,/执行时间冲突/);assert.match(human,/执行结果冲突/);
  const error=JSON.parse(f.cli({method:'timing',params:{action:'evidence',threadId:thread,turnId:turn,roots:[f.root],cursor:{token:'nothex'}}},'',1)).error;
  assert.equal(error.code,'INVALID_ARGUMENT');assert.equal(error.details.fields[0].path,'/cursor/token');
  assert.equal(error.details.schemaCommand,'wombat system api timing --json');
  assert.equal(JSON.parse(f.cli(['system','api','timing','--json'])).selectedMethod,'timing');
  const conversationError=JSON.parse(f.cli({method:'conversation',params:{action:'locate',roots:[f.root],selection:{threads:[thread]},cursor:'not-json'}},'',1)).error;
  assert.equal(conversationError.details.fields[0].path,'/cursor');
  assert.equal(conversationError.details.schemaCommand,'wombat system api conversation --json');
  const original=JSON.parse(f.cli(['history','read',thread,'--turn',turn,'--root',f.root,'--tools','--find','SYNTHETIC_DIAGNOSTIC_FAILURE','--json'],'',2));
  assert.match(original.result.fragments[0].text,/SYNTHETIC_DIAGNOSTIC_FAILURE/);
  assert.ok(original.result.fragments[0].message.operationId);
 });
});
