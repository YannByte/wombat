import assert from 'node:assert/strict';
import { test } from 'node:test';
import { createUsageClient, type ConversationRequest, type ConversationResult } from '@wombat/client';
const request: ConversationRequest = { action:'locate',selection:{projects:['/synthetic']} };
const response: ConversationResult = { outputVersion:7, action:'locate',snapshotId:'fixed',selection:{projects:['/synthetic']},items:[],fragments:[],nextCursor:null,scanComplete:true,partial:false,limits:[],scannedEvents:0,sourceBytesRead:0,freshness:null,window:null };
test('conversation responses require a revision and preserve filters and completion semantics',async()=>{
 for(const changed of [{...response,snapshotId:''}, {...response,selection:{projects:['/other']}}, {...response,scanComplete:false}, {...response,window:{anchor:{eventId:'x',sourceInstanceId:'source',eventOffset:0},radius:20}}]) {
  const client=createUsageClient({query:async()=>({}),conversation:async()=>changed});
  await assert.rejects(client.conversation!(request),{code:'PROTOCOL_ERROR'});
 }
});
test('conversation cancellation rejects a valid late transport response',async()=>{
 let start!:()=>void, finish!:(value:ConversationResult)=>void;
 const ready=new Promise<void>(r=>{start=r}), pending=new Promise<ConversationResult>(r=>{finish=r});
 const client=createUsageClient({query:async()=>({}),conversation:async()=>{start();return pending}});
 const controller=new AbortController(),result=client.conversation!(request,{signal:controller.signal});
 await ready;controller.abort();finish(response);await assert.rejects(result,{code:'CANCELLED'});
});
test('read metadata stays bound to the exact selected message',async()=>{
 const evidence={eventId:'message',sourceInstanceId:'source',threadId:'task',eventOffset:0};
 const target:ConversationRequest={action:'read',selection:request.selection,targets:[{evidence}]};
 const value:ConversationResult={...response,action:'read',fragments:[{targetIndices:[0],evidence,message:{evidence:{...evidence,eventId:'other'},project:'/synthetic',kind:'message',hasTextLocator:true,contentPartial:false,collectedAt:'2026-10-01T00:00:00Z'},text:'Synthetic text',locations:[],context:[],partial:false}]};
 const client=createUsageClient({query:async()=>({}),conversation:async()=>value});
 await assert.rejects(client.conversation!(target),{code:'PROTOCOL_ERROR'});
});

test('merged read occurrences must account for every requested target exactly once',async()=>{
 const evidence={eventId:'message',sourceInstanceId:'source',threadId:'task',eventOffset:0};
 const other={...evidence,eventId:'second',eventOffset:1};
 const request:ConversationRequest={action:'read',selection:response.selection,targets:[{evidence},{evidence:other}]};
 const first={targetIndices:[0],evidence,message:{evidence,project:'/synthetic',kind:'message',hasTextLocator:true,contentPartial:false,collectedAt:'2026-10-01T00:00:00Z'},locations:[],context:[],partial:false};
 const second={...first,targetIndices:[1],evidence:other,message:{...first.message,evidence:other}};
 for (const alsoAt of [[],[{...second,targetIndices:[0]}],[{...second,evidence}], [{...second,partial:true}]]) {
  const result:ConversationResult={...response,action:'read',fragments:[{...first,text:'Same text',alsoAt}]};
  const client=createUsageClient({query:async()=>({}),conversation:async()=>result});
  await assert.rejects(client.conversation!(request),{code:'PROTOCOL_ERROR'});
 }
 const result:ConversationResult={...response,action:'read',fragments:[{...first,text:'Same text',alsoAt:[second]}]};
 const client=createUsageClient({query:async()=>({}),conversation:async()=>result});
 assert.deepEqual(await client.conversation!(request),result);
});
