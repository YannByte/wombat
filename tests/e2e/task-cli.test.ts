import test from 'node:test';
import assert from 'node:assert/strict';
import {spawnSync} from 'node:child_process';
import {readFile,writeFile} from 'node:fs/promises';
import path from 'node:path';
import type {AgentRequest,UsageResult,ProjectResult,ConversationResult,OptimizeResult,HistoryRead} from '@wombat/client';
import {withLocalProduct,row,measurement} from '../helpers/local-product.js';
import {nativeCodexFixture} from '../helpers/native-codex.js';

test('five task entries join project evidence, preserve totals, read bounded history and reject unattended deletion',{timeout:90000},async()=>{
  await withLocalProduct(async f=>{
    const native=await nativeCodexFixture(f.dir),saved=process.env.WOMBAT_CODEX_BIN;
    process.env.WOMBAT_CODEX_BIN=native.binary;
    const invoke=(args:string[],status?:number,input?:AgentRequest)=>{
      const run=spawnSync(process.execPath,[f.entry,...args],{cwd:f.project,env:process.env,encoding:'utf8',input:input?JSON.stringify(input):'',timeout:20000,maxBuffer:2*1024*1024});
      assert.ifError(run.error);assert.ok(status===undefined?[0,2].includes(run.status??-1):run.status===status,run.stderr+run.stdout);
      return JSON.parse(run.stdout);
    };
    const at='2026-10-29T01:00:00Z';
    const dates=['--since','2026-10-29','--until','2026-10-30','--timezone','UTC'];
    const shortRules='Read [missing validation](missing-validation.md).\n';
    const rules=shortRules+'Synthetic detail. '.repeat(1000)+'\n';
    try{
      await writeFile(path.join(f.project,'AGENTS.md'),rules);
      await f.write('alpha',[row('session_meta',{id:'alpha',cwd:f.project},at),row('turn_context',{turn_id:'turn',model:'gpt-6.1-sol'},at),measurement('alpha','a',1000,at),row('event_msg',{type:'agent_message',message:'Unique release evidence '+'x'.repeat(40000),turn_id:'turn'},at)]);
      await f.write('unmetered',[row('session_meta',{id:'unmetered',cwd:f.project},at),row('event_msg',{type:'agent_message',message:'Work without token observations'},at)]);
      await f.write('beta',[row('session_meta',{id:'beta',cwd:f.project},at),measurement('beta','b',200,at)]);
      await f.write('outside',[row('session_meta',{id:'outside',cwd:f.project+'-other'},at),measurement('outside','c',9000,at)]);
      await f.client.live!({query:{action:'usage',roots:[f.root],scope:{allTime:true}},mode:'fresh'});
      const before:UsageResult=invoke(['usage','--project',f.project,...dates,'--json']);assert.equal(before.summary.tokens.total,1200);
      const history:ConversationResult=invoke(['history','--project',f.project,...dates,'--json']);assert.equal(history.items.length,3);assert.ok(history.items.some(i=>i.record?.upstreamId==='unmetered'));
      const selected=history.items.find(i=>i.record?.upstreamId==='alpha');assert.ok(selected?.evidence.threadId);if(!selected?.evidence.threadId)assert.fail('Missing alpha');
      const found:ConversationResult=invoke(['history','search','Unique release evidence','--project',f.project,...dates,'--json']);assert.ok(found.items.length>0);assert.ok(found.items.every(i=>i.project===f.project));
      const read:HistoryRead=invoke(['history','read',selected.evidence.threadId,'--find','Unique release evidence','--project',f.project,...dates,'--json']);
      const text=read.result;
      assert.ok(text.fragments.some(f=>f.text.includes('Unique release evidence')));assert.ok(text.sourceBytesRead<100000);assert.ok(text.fragments[0].nextCursor,'Long original content must expose continuation');
      const fragment=text.fragments[0];
      const continued:HistoryRead=invoke(['history','read',selected.evidence.threadId,'--project',f.project,...dates,'--snapshot',text.snapshotId,'--event',fragment.evidence.eventId,'--event-source',fragment.evidence.sourceInstanceId,'--event-offset',String(fragment.evidence.eventOffset),'--event-turn',fragment.evidence.turnId!,'--cursor',fragment.nextCursor!,'--json']);
      assert.ok(continued.result.fragments.length);assert.ok(continued.result.fragments.every(f=>f.message.evidence.eventId===f.evidence.eventId));
      const report:ProjectResult=invoke(['check','--project',f.project,'--root',f.root,...dates,'--json']);
      assert.equal(report.scope.projectRoot,f.project);assert.ok(report.configuration.result);assert.ok(report.checks.result);assert.ok(report.artifacts.result);assert.equal(report.history.result?.result.summary.tokens.total,1200);
      assert.equal(await readFile(path.join(f.project,'AGENTS.md'),'utf8'),rules,'Diagnosis edited the project');
      const viaApi:ProjectResult=invoke(['system','call'],undefined,{method:'project',params:report.scope});assert.equal(viaApi.history.result?.result.summary.tokens.total,1200);
      const methods=invoke(['system','api']);assert.ok(methods.methods.includes('project'));
      assert.deepEqual(invoke(['--help','--json']).commands,['history','usage','check','clean','system']);
      for(const topic of ['history','usage','check','clean','system']){
        const help=invoke([topic,'--help','--json'],0);assert.ok(help.examples.length);assert.ok(help.next.length);assert.equal(help.command,'wombat '+topic);
      }
      const baseline=report.checks.result!.suggestions.find(s=>s.findings.some(f=>f.rule==='localReference'));assert.ok(baseline?.baselineId);
      await writeFile(path.join(f.project,'missing-validation.md'),'Run the project validation before delivery.\n');
      await writeFile(path.join(f.project,'AGENTS.md'),shortRules);
      const verified:OptimizeResult=invoke(['check','verify','--project',f.project,'--root',f.root,'--read-view',report.checks.result!.readView!,'--baseline',baseline!.baselineId!,'--suggestion',baseline!.id,'--json']);
      assert.ok(verified.checks.some(c=>c.rule==='localReference'&&c.outcome==='miss'&&c.comparison.status==='unknown'));
      assert.ok(verified.checks.some(c=>c.rule==='fileSize'&&c.outcome==='miss'&&c.comparison.status==='comparable'));
      for(const args of [['api'],['call'],['check','inventory'],['check','cleanup'],['system','watch'],['clean','--codex','--apply','--no-input']])assert.equal(invoke([...args,'--json'],1).error.code,'INVALID_ARGUMENT');
      const after:UsageResult=invoke(['usage','--project',f.project,...dates,'--json']);assert.deepEqual(after.summary,before.summary);
      const emptyPlan=invoke(['usage','alerts','--once','--json'],2);assert.deepEqual(emptyPlan.plans,[]);
    }finally{if(saved===undefined)delete process.env.WOMBAT_CODEX_BIN;else process.env.WOMBAT_CODEX_BIN=saved;}
  });
});
