import test from 'node:test';
import assert from 'node:assert/strict';
import {createUsageClient} from '../src/client.js';
const result={outputVersion:3,policyVersion:2,action:'inspect',projectIdentity:'physical-id',baselineId:'b',checkedAt:'2026-10-31T00:00:00Z',scope:{projectRoot:'/synthetic',paths:[],keepPaths:[],productTemporary:false,codex:false},items:[],summary:{candidates:0,review:0,protected:0,unknown:0,absent:0,observedLogicalBytes:0,complete:true},partial:false};
const client=(value:unknown)=>createUsageClient({query:async()=>{throw Error('No source scan');},artifacts:async()=>value});
test('artifact boundary rejects unsupported versions, extra fields and mismatched original views',async()=>{
 assert.deepEqual(await client(result).artifacts!({action:'inspect',projectRoot:'/synthetic'}),result);
 for(const value of [{...result,policyVersion:1},{...result,outputVersion:1},{...result,items:[{path:'unsafe'}]},{...result,secret:'raw'}])await assert.rejects(client(value).artifacts!({action:'inspect',projectRoot:'/synthetic'}),{code:'PROTOCOL_ERROR'});
 await assert.rejects(client(result).artifacts!({action:'delete'} as never),{code:'INVALID_ARGUMENT'});
 await assert.rejects(client({...result,action:'recheck',baselineId:'foreign'}).artifacts!({action:'recheck',baseline:result as never,selection:[]}),{code:'PROTOCOL_ERROR'});
});
test('artifact cancellation does not publish a successful late response',async()=>{
 const controller=new AbortController();let finish!:()=>void;
 const c=createUsageClient({query:async()=>{throw Error('No scan');},artifacts:async()=>{await new Promise<void>(r=>{finish=r;});return result;}});
 const call=c.artifacts!({action:'inspect',projectRoot:'/synthetic'},{signal:controller.signal});await new Promise(r=>setTimeout(r,0));controller.abort();finish();await assert.rejects(call,{code:'CANCELLED'});
});
