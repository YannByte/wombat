import test from 'node:test';
import assert from 'node:assert/strict';
import {activityFindingTitle,inspectionCandidateText,inspectionActivityFindingText,inspectionOpportunityFindingText,inspectionOpportunityCheckText,locale} from '../src/locale/index.js';
import type {UsageResult} from '../src/index.js';
import {activityResult} from '../../tests/fixtures/activity.js';

test('concrete findings use published values, retain unknowns and follow locale changes',()=>{
 const saved=locale.getLocale();
 const candidate={signals:['low_cache_reuse','input_jump','failure_share','repeated_request'] as const,cacheShare:0.2,largestUncachedJump:100_000,failedOperations:2,determinateOperations:5,repeatedRequests:3};
 const input={...candidate,signals:[...candidate.signals]};
 const before=JSON.stringify(input);
 try{for(const language of ['zh','en'] as const){
  locale.setLocale(language);
  const findings=inspectionCandidateText(input);
  assert.equal(findings.length,4);assert.match(findings[0].observed,/20%/);assert.match(findings[1].observed,/100,000/);
  assert.match(findings[2].observed,/2/);assert.match(findings[2].observed,/5/);assert.match(findings[3].observed,/3/);
  for(const finding of findings){assert.ok(finding.advice);assert.doesNotMatch(finding.advice,/inspection\.|undefined|NaN/);}
  const missing=inspectionCandidateText({...input,cacheShare:null,largestUncachedJump:null});
  assert.match(missing[0].observed,language==='zh'?/未知/:/Unknown/);assert.doesNotMatch(missing[0].observed,/0%/);
  assert.match(missing[1].observed,language==='zh'?/未知/:/Unknown/);
  const check=activityResult().activity!.checks[0];
  assert.match(activityFindingTitle(check),language==='zh'?/失败后再次调用 1 次/:/Calls after failure: 1/);
  assert.deepEqual(inspectionCandidateText({...input,signals:[]}),[]);
 }}finally{locale.setLocale(saved);}
 assert.equal(JSON.stringify(input),before);
});

test('workflow findings present all published signals with localized advice and missing baselines',()=>{
 const saved=locale.getLocale();
 const stats={operations:7,tasks:2,determinateOperations:5,failedOperations:3,rejectedOperations:2,failureShare:0.6,outcomeGaps:0,durationSamples:7,slowOperations:7,maximumDurationMs:30000,medianDurationMs:30000,maximumFailuresInTask:3,maximumRejectionsInTask:2};
 const finding:NonNullable<NonNullable<UsageResult['inspection']>['activity']>['findings'][number]={id:'safe',sourceInstanceId:'synthetic',project:'/synthetic',tool:'exec_command',current:stats,baseline:{...stats,operations:20,determinateOperations:20,failedOperations:0,failureShare:0,durationSamples:20,medianDurationMs:3000},evidence:[],baselineEvidence:[],signals:['repeated_slow_request','failure_spike','duration_spike','recurring_workflow','repeated_failure','repeated_rejection']};
 const before=JSON.stringify(finding);
 try{for(const language of ['zh','en'] as const){
  locale.setLocale(language);const rows=inspectionActivityFindingText(finding);
  assert.equal(rows.length,6);assert.match(rows[1].observed,/60%/);assert.match(rows[2].observed,/3,000/);assert.match(rows[3].advice,/Skill/);
  assert.match(inspectionActivityFindingText({...finding,baseline:null})[1].observed,language==='zh'?/未知/:/Unknown/);
  for(const row of rows){assert.ok(row.advice);assert.doesNotMatch(row.observed+row.advice,/inspection\.|undefined|NaN/);}
 }}finally{locale.setLocale(saved);}
 assert.equal(JSON.stringify(finding),before);
});

test('opportunity copy keeps safety categories and unknown measurements explicit',()=>{
 const saved=locale.getLocale();
 try{locale.setLocale('en');
 const text=inspectionOpportunityFindingText({id:'safe',object:null,metrics:[{name:'interval_ms',value:null,unit:'milliseconds'}],safetyLabels:['broad_deletion'],evidence:[],baselineEvidence:[]});
 assert.ok(text.includes('Root or home deletion request'));assert.ok(text.some(s=>s.includes('Unknown')));
 const check=inspectionOpportunityCheckText({rule:'unanswered_question',status:'insufficient',gaps:['interaction_association'],findingCount:0,findings:[]});
 assert.equal(check.status,'Insufficient evidence');assert.match(check.gaps[0],/association incomplete/);
 locale.setLocale('zh');assert.match(inspectionOpportunityCheckText({rule:'model_review',status:'hit',gaps:[],findingCount:1,findings:[]}).advice,/代表性任务/);
 }finally{locale.setLocale(saved);}
});
