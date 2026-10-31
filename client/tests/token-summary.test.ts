import test from 'node:test';
import assert from 'node:assert/strict';
import { locale, t, tokenSummaryPresentation, tokenSummaryText, analyzedTokenSubtotal } from '@wombat/client/locale';
import type { UsageSummary } from '@wombat/client';
import { withTokenAnalysis } from '../../tests/fixtures/token-analysis.js';

const complete:UsageSummary=withTokenAnalysis({measurementCount:2,inputTotal:100,tokens:{rawInput:100,input:80,cacheRead:20,cacheCreate:0,output:12,reasoning:3,total:112},price:{currency:'USD',policy:'synthetic',priceRevision:'synthetic',cost:'0',knownCost:'0',status:'priced',components:[],basis:[],issues:[]}});
const partial = (subtotal:number|null):UsageSummary => ({...complete,tokens:{...complete.tokens,total:null},tokenAnalysis:{...complete.tokenAnalysis,fields:{...complete.tokenAnalysis.fields,total:{observedSubtotal:subtotal,coveredRecords:subtotal==null?0:1,missingRecords:subtotal==null?2:1,conflictingRecords:0,invalidRecords:0,indeterminateRecords:0}}}});

test('shared token presentation preserves complete, partial, partial zero, unavailable and empty fields',()=>{
 const previous=locale.getLocale();
 try{for(const language of ['zh','en'] as const){locale.setLocale(language);
  assert.equal(tokenSummaryPresentation(complete).value,112);
  assert.equal(tokenSummaryPresentation(complete).state,'complete');
  assert.equal(tokenSummaryPresentation(complete,'rawInput').value,100);
  for(const subtotal of [100,0]){
   const fixture=partial(subtotal),before=JSON.stringify(fixture),value=tokenSummaryPresentation(fixture);
   assert.equal(value.value,subtotal);assert.equal(value.state,'partial');
   assert.match(tokenSummaryText(fixture),language==='en'?/Known subtotal/:/已知小计/);
   assert.match(value.description,language==='en'?/canonical measurement records only/:/所选范围内的规范测量记录/);
   assert.equal(JSON.stringify(fixture),before);
  }
  const unknown=partial(null);assert.equal(tokenSummaryPresentation(unknown).value,null);assert.equal(tokenSummaryPresentation(unknown).state,'unavailable');
  assert.equal(tokenSummaryText(unknown),t('usage.tokenValueUnavailable'));
  assert.doesNotMatch(tokenSummaryText(unknown),/0/);
  const empty=withTokenAnalysis({...complete,measurementCount:0,tokens:{total:0}});
  assert.equal(tokenSummaryPresentation(empty).state,'empty');assert.equal(tokenSummaryPresentation(empty).value,null);
  assert.equal(tokenSummaryPresentation(partial(100),'output').state,'complete');
 }}finally{locale.setLocale(previous);}
});

test('token coverage retains separate absence reasons and does not rebuild native input from parts',()=>{
 const saved=locale.getLocale();
 try{for(const language of ['zh','en'] as const){locale.setLocale(language);
 const fixture:UsageSummary={...complete,inputTotal:null,tokens:{...complete.tokens,rawInput:null},tokenAnalysis:{...complete.tokenAnalysis,fields:{...complete.tokenAnalysis.fields,rawInput:{observedSubtotal:null,coveredRecords:0,missingRecords:1,conflictingRecords:2,invalidRecords:3,indeterminateRecords:4}}}};
 const value=tokenSummaryPresentation(fixture,'rawInput');assert.equal(value.value,null);assert.equal(value.state,'unavailable');
 for(const text of language==='en'?['Count not recorded: 1','Conflicting counts: 2','Invalid counts: 3','Indeterminate counts: 4']:['未提供此项数量：1','此项数量有冲突：2','此项数量无效：3','此项数量无法确定：4'])assert.ok(value.description.includes(text));
 }}finally{locale.setLocale(saved);}
});

test('analyzed totals label calculations and keep native gaps, zero and historical captures',()=>{
 const saved=locale.getLocale();
 try{for(const language of ['zh','en'] as const){locale.setLocale(language);
  const fixture:UsageSummary={...partial(null),tokenAnalysis:{...partial(null).tokenAnalysis,totalAnalysis:{methodVersion:1,subtotal:120,coveredRecords:2,recordedRecords:0,calculatedRecords:2,unavailableRecords:0,overflowRecords:0}}};
  const before=JSON.stringify(fixture),value=tokenSummaryPresentation(fixture);
  assert.equal(value.value,120);assert.equal(value.state,'complete');assert.equal(value.calculated,true);assert.equal(value.coveredRecords,2);
  assert.equal(value.coverage.coveredRecords,0);
  assert.ok(tokenSummaryText(fixture).includes(t('usage.tokenAnalysis.calculated')));
  assert.ok(value.description.includes(t('usage.tokenAnalysis.missing',{count:2})));
  assert.ok(value.description.includes(t('usage.tokenAnalysis.formula',{count:2})));
  const mixed={...fixture,tokenAnalysis:{...fixture.tokenAnalysis,totalAnalysis:{...fixture.tokenAnalysis.totalAnalysis!,recordedRecords:1,calculatedRecords:1}}};
  assert.ok(tokenSummaryText(mixed).includes(t('usage.tokenAnalysis.includesCalculated')));
  const zero={...fixture,tokenAnalysis:{...fixture.tokenAnalysis,totalAnalysis:{...fixture.tokenAnalysis.totalAnalysis!,subtotal:0,coveredRecords:1,calculatedRecords:1,unavailableRecords:1}}};
  assert.equal(tokenSummaryPresentation(zero).value,0);assert.equal(tokenSummaryPresentation(zero).state,'partial');
  assert.ok(tokenSummaryText(zero).includes(t('usage.tokenAnalysis.subtotal')));
  assert.equal(JSON.stringify(fixture),before);
  assert.equal(tokenSummaryPresentation(partial(100)).calculated,false);assert.equal(analyzedTokenSubtotal(partial(100)),100);assert.equal(analyzedTokenSubtotal(fixture),120);
  const unavailable={...complete,tokenAnalysis:{...complete.tokenAnalysis,totalAnalysis:{methodVersion:1,subtotal:null,coveredRecords:0,recordedRecords:0,calculatedRecords:0,unavailableRecords:2,overflowRecords:1}}};
  assert.equal(tokenSummaryPresentation(unavailable).value,null);assert.equal(analyzedTokenSubtotal(unavailable),null);
  assert.ok(tokenSummaryPresentation(unavailable).description.includes(t('usage.tokenAnalysis.overflow',{count:1})));
 }}finally{locale.setLocale(saved);}
});
