import assert from 'node:assert/strict';
import { test } from 'node:test';
import { createUsageClient, type ConversationRequest, type ConversationResult } from '@wombat/client';
const request: ConversationRequest = { action:'locate',snapshotId:'fixed',selection:{projects:['/synthetic']} };
const response: ConversationResult = { outputVersion:1, action:'locate',snapshotId:'fixed',baselineSnapshotId:null,selection:{projects:['/synthetic']},items:[],fragments:[],nextCursor:null,scanComplete:true,partial:false,limits:[],scannedEvents:0,sourceBytesRead:0,freshness:null,window:null };
test('conversation responses cannot change the selected view, filters or completion semantics',async()=>{
 for(const changed of [{...response,snapshotId:'other'}, {...response,selection:{projects:['/other']}}, {...response,scanComplete:false}, {...response,window:{anchor:{eventId:'x',sourceInstanceId:'source',eventOffset:0},radius:20}}]) {
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
