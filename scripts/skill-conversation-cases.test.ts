import {test} from 'node:test';
import assert from 'node:assert/strict';
import path from 'node:path';
import {focusedQuestions,focusedQuestion,focusedResults,validateFocusedResults,isFocusedScenario,type FocusedScenario} from './skill-conversation-cases.ts';

const scope={snapshotId:'fixed-synthetic-view',project:'/synthetic/project',threadId:'selected-task'};
const usage={outputVersion:5,action:'usage',snapshotRef:{snapshotId:scope.snapshotId},scope:{project:scope.project,allTime:true},summary:{tokens:{total:1100000}}};
const values:Record<FocusedScenario,Record<string,unknown>[]>={
  'daily-review':[usage,{...usage,action:'threads',items:[{kind:'thread',id:scope.threadId}]}],
  'task-retro':[{...usage,scope:{...usage.scope,threadId:scope.threadId},inspection:{}},{outputVersion:1,snapshotId:scope.snapshotId,selection:{projects:[scope.project],threads:[scope.threadId]},items:[],fragments:[{text:'SYNTHETIC_WORK_EVIDENCE'}]}],
  'config-review':[{action:'list',suggestions:[{id:'original',item:{path:path.join(scope.project,'.agents/skills/sample/SKILL.md')},findings:[{rule:'descriptionSize'}]}]}],
  usage:[usage],
  pricing:[{...usage,summary:{tokens:{total:1101000},price:{cost:null,knownCost:'7.25'}}}],
  timing:[{outputVersion:6,action:'summary',time:{nativeWallClockMs:{value:15000}}}],
  statistics:[{...usage,action:'statistics',statistics:{population:{completeTasks:1,medianTokens:1100000,p90Tokens:1100000}}}],
  monitor:[{action:'list',plans:[]}],
  account:[{action:'read',windows:[{usedPercent:20,durationMinutes:17}]}],
};
test('focused scenarios retain paired natural requests and bound variant selection',()=>{
  for(const scenario of Object.keys(focusedQuestions).filter(isFocusedScenario)){
    for(let variant=0;variant<4;variant++){
      assert.match(focusedQuestion(scenario,'zh',variant),/\p{Script=Han}/u);
      assert.ok(focusedQuestion(scenario,'en',variant).trim());
    }
    assert.throws(()=>focusedQuestion(scenario,'en',-1));assert.throws(()=>focusedQuestion(scenario,'zh',4));
  }
});
test('runtime extraction keeps live and fixed JSON results without treating prose or metadata as results',()=>{
  const event=(output:string)=>JSON.stringify({type:'item.completed',item:{type:'command_execution',aggregated_output:output}});
  const events=[event('progress\n'+JSON.stringify({result:usage,freshness:{status:'fixed'}})),event(JSON.stringify(usage)),event('{not json\nexplanation'),JSON.stringify({type:'item.completed',item:{type:'agent_message',text:JSON.stringify({plans:['invented']})}})].join('\n');
  assert.deepEqual(focusedResults(events),[usage,usage]);
});
test('capability checks accept independent truth and reject unrelated queries, mutations and scope changes',()=>{
  for(const scenario of Object.keys(values).filter(isFocusedScenario)){
    validateFocusedResults(scenario,values[scenario],scope);
    assert.throws(()=>validateFocusedResults(scenario,[],scope));
    assert.throws(()=>validateFocusedResults(scenario,[...values[scenario],scenario==='config-review'?usage:{action:'list',suggestions:[]}],scope));
    assert.throws(()=>validateFocusedResults(scenario,[...values[scenario],{action:'upsert',plans:[]}],scope));
  }
  for(const change of [{snapshotRef:{snapshotId:'other'}},{scope:{project:'/other'}},{summary:{tokens:{total:0}}}])assert.throws(()=>validateFocusedResults('usage',[{...usage,...change}],scope));
  assert.throws(()=>validateFocusedResults('pricing',[{...values.pricing[0],summary:{tokens:{total:1101000},price:{cost:'7.25',knownCost:'7.25'}}}],scope));
  assert.throws(()=>validateFocusedResults('timing',[{outputVersion:6,time:{nativeWallClockMs:{value:null}}}],scope));
  assert.throws(()=>validateFocusedResults('statistics',[{...usage,statistics:{population:{completeTasks:1,medianTokens:0,p90Tokens:0}}}],scope));
  assert.throws(()=>validateFocusedResults('account',[{windows:[{usedPercent:null,durationMinutes:17}]}],scope));
});

test('task workflows require selected original evidence and reject unrelated task identities',()=>{
  assert.throws(()=>validateFocusedResults('task-retro',[values['task-retro'][0]],scope));
  assert.throws(()=>validateFocusedResults('task-retro',[values['task-retro'][1]],scope));
  const wrong={...values['task-retro'][1],selection:{projects:[scope.project],threads:['other-task']}};
  assert.throws(()=>validateFocusedResults('task-retro',[values['task-retro'][0],wrong],scope));
  assert.throws(()=>validateFocusedResults('daily-review',[usage],scope));
  assert.throws(()=>validateFocusedResults('config-review',[{suggestions:[{item:{path:'/other'},findings:[{rule:'descriptionSize'}]}]}],scope));
});

test('navigation accepts observed empty dates and a different selected project only at the original view',()=>{
  const dated={...scope,totalTokens:0,since:'2026-09-03',until:'2026-09-04'};
  const empty={...usage,scope:{project:scope.project,since:dated.since,until:dated.until},summary:{tokens:{total:0}}};
  validateFocusedResults('usage',[empty],dated);
  assert.throws(()=>validateFocusedResults('usage',[empty],{...dated,since:'2026-09-02'}));
  const changed={...scope,project:'/synthetic/other-project',totalTokens:650};
  const other={...usage,scope:{project:changed.project,allTime:true},summary:{tokens:{total:650}}};
  validateFocusedResults('usage',[other],changed);
  assert.throws(()=>validateFocusedResults('usage',[other],{...changed,project:scope.project}));
  assert.throws(()=>validateFocusedResults('usage',[{...other,snapshotRef:{snapshotId:'new-view'}}],changed));
});
