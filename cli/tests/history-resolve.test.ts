import test from 'node:test';
import assert from 'node:assert/strict';
import {CoreError,type ConversationResult,type ConversationRequest} from '@wombat/client';
import {resolveRecord} from '../src/history-cli.js';
const response=(request:ConversationRequest):ConversationResult=>({outputVersion:7,action:request.action,snapshotId:'fixed',selection:request.selection??{},items:[],fragments:[],scanComplete:true,partial:false,limits:[],scannedEvents:0,sourceBytesRead:0});
test('exact task identity skips activity and preserves the requested date scope',async()=>{
 const calls:ConversationRequest[]=[];
 const scope={since:'2026-10-26',until:'2026-11-02',timezone:'Asia/Shanghai'};
 const result=await resolveRecord({conversation:async request=>{
  calls.push(request);
  return {...response(request),record:{id:'known',upstreamId:'native',sourceInstanceId:'source',project:null,title:'Task'}};
 }},'known',scope,undefined,[],new AbortController().signal);
 assert.equal(result.thread.id,'known');
 assert.equal(calls.length,1);assert.equal(calls[0].action,'resolve');
 assert.equal(calls[0].selection?.since,scope.since);
});
test('bounded ambiguous lookup exposes its original continuation rather than reporting indexing pending',async()=>{
 const calls:ConversationRequest[]=[];
 await assert.rejects(resolveRecord({conversation:async request=>{
  calls.push(request);return request.action==='resolve'?response(request):{...response(request),scanComplete:false,partial:true,nextCursor:'resume',limits:['scan_budget']};
 }},'a title',{since:'2026-10-26',until:'2026-11-02',timezone:'UTC'},['/synthetic'],[],new AbortController().signal),error=>{
  assert.ok(error instanceof CoreError);assert.equal(error.code,'SCAN_INCOMPLETE');
  assert.deepEqual(error.details,{next:{method:'conversation',params:{...calls[1],cursor:'resume'}}});return true;
 });
});
