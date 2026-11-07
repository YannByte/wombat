import { queryInvocation, spawnProductCli, type CliInput } from '../../scripts/cli-invocation.js';
import {test} from 'node:test';
import assert from 'node:assert/strict';
import {mkdtemp,mkdir,writeFile,appendFile,rm} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import path from 'node:path';
import {spawn,spawnSync} from 'node:child_process';
import {once} from 'node:events';
import {setTimeout as delay} from 'node:timers/promises';
const jsonl=(rows:unknown[])=>rows.map(r=>JSON.stringify(r)+'\n').join('');
const usage=(thread:string,id:string,n:number,day:string)=>({type:'event_msg',timestamp:`${day}T00:00:00Z`,payload:{type:'token_usage_record',thread_id:thread,turn_id:'u',response_id:id,usage:{input_tokens:n,cached_input_tokens:0,cache_write_input_tokens:0,output_tokens:0,reasoning_output_tokens:0,total_tokens:n}}});
test('built CLI and native share period drivers, family totals and durable refresh changes',{timeout:60_000},async()=>{
 const dir=await mkdtemp(path.join(tmpdir(),'wombat-comparison-')),root=path.join(dir,'source'),data=path.join(dir,'data');
 await mkdir(path.join(root,'sessions'),{recursive:true});
 for(const [id,parent,n] of [['parent',null,100],['child','parent',30],['other',null,70]] as const)await writeFile(path.join(root,'sessions',`${id}.jsonl`),jsonl([{type:'session_meta',payload:{id,cwd:`/synthetic/${id}`,forked_from_id:parent}},{type:'turn_context',payload:{turn_id:'u',model:'gpt-5.4',effort:'high'}},{type:'response_item',timestamp:'2026-09-09T00:00:00Z',payload:{type:'message',role:'developer',content:[{type:'input_text',text:'Synthetic injected instruction'}]}},usage(id,`before-${id}`,n,'2026-09-02'),...(id==='parent'?[{type:'event_msg',timestamp:'2026-09-09T00:00:00Z',payload:{type:'compacted'}}]:[]),usage(id,`after-${id}`,id==='parent'?140:n,'2026-09-09')]));
 const env:NodeJS.ProcessEnv={...process.env,CODEX_HOME:root,WOMBAT_DATA_HOME:data,WOMBAT_AUTO_PRICES:'0',WOMBAT_CODEX_BIN:path.join(dir,'absent-codex')};delete env.WOMBAT_CORE_BIN;
 const binary=path.resolve('dist',process.platform==='win32'?'wombat-core.exe':'wombat-core'),cli=path.resolve('dist/wombat.js');

 const stop=async(child:ReturnType<typeof spawn>|undefined)=>{if(!child||child.exitCode!==null||child.signalCode!==null)return;const exit=once(child,'close');child.kill('SIGTERM');const timer=setTimeout(()=>child.kill('SIGKILL'),2000);try{await exit;}finally{clearTimeout(timer);}};
 const run=(args: CliInput,input?:string,timeout=15_000)=>{const p=spawnProductCli(cli, Array.isArray(args) ? [...args, '--json'] : args, {env,input,encoding:'utf8',timeout,maxBuffer:8*1024*1024});assert.ifError(p.error);assert.ok([0,2].includes(p.status!),p.stdout+p.stderr);return JSON.parse(p.stdout);};
 try{
  await delay(100);
  // Establish a completed fixture publication; these assertions are not latency checks.
  const first=run(['usage','--fresh','--all-time'],undefined,25_000);
  assert.notEqual(first.freshness?.initialScan,true);
  const args=["usage", "compare",'--since','2026-09-08','--until','2026-09-15','--baseline-since','2026-09-01','--baseline-until','2026-09-08','--by','project','--limit','1'];
  const compared=run(args);assert.equal(compared.snapshotRef.snapshotId,first.snapshotRef.snapshotId);assert.equal(compared.comparison.delta.tokens,40);assert.equal(compared.page.total,3);assert.equal(compared.comparison.drivers[0].delta.tokens+compared.comparison.remaining.tokens,40);
  const threads=run({"method":"usage","params":{"query":{"action":"threads","scope":{"allTime":true}},"mode":"auto"}});const id=(name:string)=>threads.items.find((t:{upstreamId:string})=>t.upstreamId===name).id;
  const family=run(["usage", "compare",'--task',id('parent'),'--against',id('other'),'--family','--all-time']);assert.equal(family.comparison.left.own.tokens.total,240);assert.equal(family.comparison.left.descendants.tokens.total,60);assert.equal(family.comparison.left.selected.tokens.total,300);assert.equal(family.comparison.left.memberCount,2);
  const inspect=run({"method":"usage","params":{"query":{"action":"investigate","scope":{"allTime":true}},"mode":"cached"}});assert.equal(inspect.inspection.candidateCount,0);assert.equal(inspect.snapshotRef.snapshotId,first.snapshotRef.snapshotId);
  const trajectory=run({"method":"usage","params":{"query":{"action":"trajectory","scope":{"allTime":true,"threadId":id('parent')}},"mode":"cached"}});assert.equal(trajectory.inspection.trajectory.length,2);assert.equal(trajectory.inspection.trajectory[1].uncachedDelta,null);assert.equal(trajectory.inspection.trajectory[1].compactionComparison.inputDifference,40);assert.ok(trajectory.inspection.limitations.includes('context_occupancy_unavailable'));
  const contexts=run({"method":"usage","params":{"query":{"action":"context","scope":{"allTime":true,"threadId":id('parent')}},"mode":"cached"}});assert.equal(contexts.inspection.context.injectedRecords,1);assert.equal(contexts.inspection.context.records[0].contentVersion,null);
  const review=run({"method":"usage","params":{"query":{"action":"review","scope":{"since":"2026-09-08","until":"2026-09-15"}},"mode":"cached"}});assert.equal(review.inspection.review.comparison.delta.tokens,40);assert.equal(review.inspection.review.models[0].usage.tokens.total,240);
  const emptyHistory=run({"method":"account","params":{"action":"history"}});assert.equal(emptyHistory.history.totalObservations,0,'history must work without a native Codex executable');
  await appendFile(path.join(root,'sessions','parent.jsonl'),jsonl([usage('parent','new',10,'2026-09-10')]));
  assert.equal(run(args).snapshotRef.snapshotId,first.snapshotRef.snapshotId,'compare must not refresh');
  const refreshed=run(['usage','--fresh','--all-time'],undefined,25_000);assert.equal(refreshed.freshness.publicationChange.measurementsAdded,1);assert.equal(refreshed.freshness.publicationChange.delta.tokens,10);
  const fixed=run([...args,]);assert.equal(fixed.comparison.delta.tokens,50);
  await delay(100);
  let restored=run(['usage','--cached','--all-time']);const restoredBy=Date.now()+5000;
  while(restored.snapshotRef.snapshotId!==refreshed.snapshotRef.snapshotId&&Date.now()<restoredBy){await delay(30);restored=run(['usage','--cached','--all-time']);}
  assert.equal(restored.snapshotRef.snapshotId,refreshed.snapshotRef.snapshotId);assert.deepEqual(restored.freshness.publicationChange,refreshed.freshness.publicationChange);
  const call=(query:import('@wombat/client').UsageRequest)=>run(['system','call'],JSON.stringify({method:'usage',params:{query:{roots:[root],...query},mode:'cached'}}));
  await call({action:'usage',scope:{allTime:true}});
  const value=(await call({action:'compare',scope:{since:'2026-09-08',until:'2026-09-15',timezone:'UTC'},comparison:{kind:'periods',baselineSince:'2026-09-01',baselineUntil:'2026-09-08',dimension:'project'},limit:1})).result;
  assert.equal(value.comparison.delta.tokens,50);assert.deepEqual(value.freshness.publicationChange,restored.freshness.publicationChange);
  for(const action of ['investigate','resources','review','context'] as const){const q={action,scope:{since:'2026-09-08',until:'2026-09-15',timezone:'UTC'}};const response=(await call(q)).result;assert.equal(response.inspection.kind,action);assert.equal(response.summary.tokens.total,250);if(action==='review')assert.equal(response.inspection.review.comparison.delta.tokens,50);}
  const trajectoryValue=(await call({action:'trajectory',threadId:id('parent'),scope:{allTime:true}})).result;assert.deepEqual(trajectoryValue.inspection.trajectory.map((p:{input:number})=>p.input),[100,140,10]);assert.equal(trajectoryValue.inspection.trajectory[2].uncachedDelta,-130);
  const parentTurns=run(queryInvocation({"action":"turns","threadId":id('parent'),"scope":{"allTime":true}}, {mode:"auto"}));
  const turn=parentTurns.items[0].id;
  const exactCli=run(queryInvocation({action:'investigate',scope:{threadId:id('parent'),turnId:turn,allTime:true},compact:true},{mode:'cached'}));
  const exactNative=(await call({action:'investigate',scope:{threadId:id('parent'),turnId:turn,allTime:true},compact:true})).result;
  assert.deepEqual(exactNative,exactCli);assert.equal(exactNative.summary.tokens.total,250);
  assert.equal(exactNative.scope.turnId,turn);assert.ok(exactNative.inspection.limitations.includes('selected_turn_only'));
  const fullUsage=(await call({action:'usage',scope:{allTime:true},limit:1})).result;
  const compactUsage=(await call({action:'usage',scope:{allTime:true},limit:1,compact:true})).result;
  assert.deepEqual(compactUsage.summary,fullUsage.summary);assert.deepEqual(compactUsage.items,fullUsage.items);assert.deepEqual(compactUsage.page,fullUsage.page);
  assert.equal(compactUsage.facets,undefined);assert.ok(compactUsage.quality.detailSummary);

  const bad=spawnProductCli(cli, ['compare', '--since', '2026-09-08', '--until', '2026-09-15', '--baseline-since', '2026-09-04', '--baseline-until', '2026-09-11', '--json'], {env,encoding:'utf8',timeout:10_000});assert.equal(bad.status,1);assert.equal(JSON.parse(bad.stdout).error.code,'INVALID_ARGUMENT');
 }finally{await rm(dir,{recursive:true,force:true,maxRetries:10,retryDelay:100});}
});
