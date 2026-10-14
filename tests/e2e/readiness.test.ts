import {test} from 'node:test';
import assert from 'node:assert/strict';
import path from 'node:path';
import {utimes} from 'node:fs/promises';
import {withLocalProduct,row,measurement} from '../helpers/local-product.ts';

test('initial recent preparation, batch reads and history expansion agree across CLI and Web',{timeout:40000},async()=>{
 await withLocalProduct(async f=>{
  const day=new Date().toISOString().slice(0,10),at=day+'T00:00:00Z';
  for(const [id,total,time] of [['hot',110,at],['cold',220,'2020-01-01T00:00:00Z']] as const){
   await f.write(id,[row('session_meta',{id,cwd:f.project},time),row('turn_context',{turn_id:'turn',model:'gpt-5.4'},time),measurement(id,id,total,time)]);
  }
  await utimes(path.join(f.root,'sessions/cold.jsonl'),new Date('2020-01-01'),new Date('2020-01-01'));
  const scope={since:day,until:new Date(Date.parse(day)+86400000).toISOString().slice(0,10),timezone:'UTC'};
  const params={query:{action:'usage',scope,compact:true},mode:'fresh',waitMs:20000,also:[{action:'threads',scope,compact:true},{action:'statistics',scope,compact:true}]};
  const recent=JSON.parse(f.cli(['call','--json'],JSON.stringify({method:'usage',params}),2));
  assert.equal(recent.result.summary.tokens.total,110);assert.equal(recent.freshness.initialScan,false);
  assert(recent.result.quality.sources.some((s:any)=>s.issues.some((i:any)=>i.code==='historyDeferred')));
  for(const value of recent.additional){assert.equal(value.summary.tokens.total,110);assert.equal(value.snapshotRef.snapshotId,recent.freshness.revision);}
  const browser=await f.browser();
  const restored=await browser.live!({query:params.query,also:params.also,mode:'cached'} as import('@wombat/client').LiveRequest);
  assert.equal(restored.freshness.revision,recent.freshness.revision);assert.equal(restored.additional?.[1].statistics?.population.completeTaskTokens,110);
  for(const patch of [{roots:['/outside']},{snapshotId:recent.freshness.revision}])
   await assert.rejects(browser.live!({query:{action:'usage',scope},also:[{action:'usage',...patch}]}),{code:'INVALID_ARGUMENT'});
  const complete=await browser.live!({query:{action:'usage',scope:{allTime:true},compact:true},mode:'fresh',waitMs:20000});
  assert.equal(complete.result.summary.tokens.total,330);assert(!complete.result.quality.sources.some(s=>s.issues.some(i=>i.code==='historyDeferred')));
  await f.stop();await f.start();
  const restarted=await browser.live!({query:{action:'usage',scope:{allTime:true},compact:true},mode:'cached',also:[{action:'statistics',scope:{allTime:true},compact:true}]});
  assert.deepEqual(restarted.result.summary,complete.result.summary);assert.equal(restarted.additional?.[0].statistics?.population.completeTaskTokens,330);
 });
});
