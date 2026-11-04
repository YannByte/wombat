import {test} from 'node:test';
import assert from 'node:assert/strict';
import path from 'node:path';
import {parseVerificationArgs} from '../src/optimize-cli.js';
import {commandHelp} from '../src/help.js';
test('configuration and verification help lead directly to their actual interfaces',()=>{
 for(const target of ['skills','mcp','rules','hooks'])assert.deepEqual(commandHelp('check',[target]).next,['wombat system api config --json']);
 assert.deepEqual(commandHelp('check',['verify']).next,['wombat system api optimize --json']);
 assert.deepEqual(commandHelp('check',[]).next,['wombat system api project --json']);
});
test('verification rejects removed view controls and requires an original report',()=>{
 for(const args of [['recheck'],['list'],['keep'],['verify'],['verify','--read-view','v'],['verify','--baseline','missing-report.json']])assert.throws(()=>parseVerificationArgs(args));
});

test('explain requires a configuration target instead of silently affecting a project or task check', async () => {
  const {runProjectCli} = await import('../src/project-cli.js');
  for(const args of [['--explain'],['--explain','--task','synthetic'],['skills','--explain=true'],['skills','--explain','--explain']]) {
    await assert.rejects(runProjectCli(args), {code:'INVALID_ARGUMENT'});
  }
});

test('config text uses the core basis to distinguish known zero, missing history and identity gaps', async () => {
  const { configUseBasisLines } = await import('../src/config-cli.js');
  const { locale } = await import('@wombat/client/locale');
  const saved=locale.getLocale();
  const item = {kind:'skill',usageCount:0,counts:{fileReads:0},useBasis:{methodVersion:3,status:'observed',unit:'object_use',capturedAt:'2026-10-04T02:00:00Z',snapshotId:'synthetic',scope:{sourceInstanceIds:['synthetic'],project:'/synthetic\nproject',threadId:null,agentKind:null,window:{kind:'all_history'}},timeBasis:'source_operation_time',coverage:{dispatchGaps:0,identityGaps:0,targetGaps:0,timeGaps:0,turnGaps:0},sourceCompleteness:'partial'}} as import('@wombat/client').ConfigItem;
  try {for(const language of ['zh','en'] as const){locale.setLocale(language);
    const known=configUseBasisLines(item);assert.match(known[0],/: 0$/);assert.ok(known.every(line=>!line.includes('\n')));assert.match(known.join(' '),/采集部分完整|collection partially complete/);
    for(const count of [0,3]){const partial=configUseBasisLines({...item,usageCount:count,useBasis:{...item.useBasis!,status:'partial',sourceCompleteness:'complete',coverage:{targetGaps:1}}});assert.match(partial[0],new RegExp(`: ${count}$`));assert.match(partial.join(' '),/完整次数更高|full count higher/);assert.match(partial.join(' '),/0.*不证明|0 does not prove/);assert.doesNotMatch(partial.join(' '),/未使用。$|No recorded use/);}
    const missing=configUseBasisLines({...item,useBasis:{...item.useBasis!,status:'unavailable',coverage:{identityGaps:1}}});assert.match(missing[0],/没有|No fixed/);assert.doesNotMatch(missing[0],/: 0$/);
    const unavailable=configUseBasisLines({...item,useBasis:null});assert.doesNotMatch(unavailable[0],/: 0$/);assert.match(unavailable.join(' '),/没有|No fixed/);
  }}finally{locale.setLocale(saved);}
});

test('default configuration reports are concise and explanation is opt-in in both languages', async () => {
  const { formatConfigText } = await import('../src/config-cli.js');
  const { locale } = await import('@wombat/client/locale');
  const saved=locale.getLocale();
  const item = {id:'skill',name:'review',kind:'skill',path:'/synthetic/SKILL.md',observation:'unknown',usageCount:0,counts:{fileReads:0},useBasis:{methodVersion:3,status:'partial',unit:'object_use',capturedAt:'2026-10-04T02:00:00Z',snapshotId:'synthetic',scope:{sourceInstanceIds:['synthetic'],project:'/synthetic/project',window:{kind:'date_window',since:'2026-10-01',until:'2026-10-05',timezone:'UTC'}},timeBasis:'source_operation_time',coverage:{identityGaps:1},sourceCompleteness:'complete'}} as import('@wombat/client').ConfigItem;
  const report:import('@wombat/client').ConfigResult = {outputVersion:3,action:'list',capabilities:{kinds:['skill'],evidenceTypes:[],tokenEstimates:true,historicalContent:false,historicalContentHashes:true,writes:false,projectRegistry:false},configRevision:'synthetic',checkedAt:'2026-10-04T02:00:00Z',authorizedProjects:['/synthetic/project'],summary:{currentItems:1,historicalItems:0,observedItems:0},page:{offset:0,limit:20,total:1},scope:{project:'/synthetic/project',since:'2026-10-01',until:'2026-10-05',timezone:'UTC'},items:[item],evidence:[],relatedScopes:[],hookRegistry:{status:'unavailable',contexts:[]},coverage:{status:'partial',historyStatus:'current',absenceObservable:false,issues:[],supportedEvidence:[]}};
  const before=structuredClone(report);
  try {for(const language of ['zh','en'] as const){locale.setLocale(language);
    const brief=formatConfigText(report);
    assert.match(brief,/所选时间内未使用|Not used in the selected period/);
    assert.ok(brief.includes('2026-10-01')&&brief.includes('2026-10-05')&&brief.includes('/synthetic/project'));
    assert.doesNotMatch(brief,/方法版本|Method version|不证明|does not prove|身份缺口|identity gaps/);
    const explained=formatConfigText(report,{action:'list'},{explain:true});
    assert.match(explained,/方法版本|Method version/);
    assert.match(explained,/完整次数更高|full count higher/);
    const failed=formatConfigText({...report,coverage:{...report.coverage,historyStatus:'unavailable',issues:[{code:'SOURCE_UNREADABLE',path:'/synthetic/blocked\nsource'}]} });
    assert.match(failed,/SOURCE_UNREADABLE/);assert.ok(failed.includes('/synthetic/blocked source'));
    assert.doesNotMatch(failed,/所选时间内未使用|Not used in the selected period/);
  }}finally{locale.setLocale(saved);}
  assert.deepEqual(report,before);
});
