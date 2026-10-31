import test from 'node:test';
import assert from 'node:assert/strict';
import {locale} from '@wombat/client/locale';
import type {UsageSummary,UsageResult,UsageItem} from '@wombat/client';
import {withTokenAnalysis} from '../../tests/fixtures/token-analysis.js';
import {usageLabel,itemLines,summaryDetails} from '../src/format.js';
const base:UsageSummary=withTokenAnalysis({measurementCount:2,tokens:{total:null,input:100,cacheRead:0,cacheCreate:0,output:20,reasoning:0},price:{status:'unknown',currency:'USD',policy:'synthetic',priceRevision:'synthetic',cost:null,knownCost:'0',components:[],basis:[],issues:[]}});
const usage:UsageSummary={...base,tokenAnalysis:{...base.tokenAnalysis,fields:{...base.tokenAnalysis.fields,total:{observedSubtotal:100,coveredRecords:1,missingRecords:1,conflictingRecords:0,invalidRecords:0,indeterminateRecords:0}}}};
test('CLI token summaries and narrow or wide rows show the core subtotal with coverage',()=>{
 const previous=locale.getLocale();
 try{for(const language of ['zh','en'] as const){locale.setLocale(language);
 const marker=language==='en'?'Known subtotal':'已知小计';
 assert.ok(usageLabel(usage).includes(marker));assert.match(usageLabel(usage),/100/);
 const item:UsageItem={kind:'usage',isSubtotal:true,date:'2026-10-05',scope:{},usage};
 const result={snapshotRef:{createdAt:'2026-10-05T00:00:00Z'},scope:{}} as UsageResult;
 for(const width of [60,120]){const text=itemLines(item,result,width).join('\n');assert.ok(text.includes(marker));assert.match(text,/100/);}
 const zero={...usage,tokenAnalysis:{...usage.tokenAnalysis,fields:{...usage.tokenAnalysis.fields,total:{...usage.tokenAnalysis.fields.total,observedSubtotal:0}}}};
 assert.match(usageLabel(zero),/0/);assert.ok(usageLabel(zero).includes(marker));
 assert.ok(summaryDetails(usage).some(row=>row.includes('100')));
 }}finally{locale.setLocale(previous);}
});

test('CLI labels complete and partial calculated totals at narrow and wide widths',()=>{
 const saved=locale.getLocale();
 try{for(const language of ['zh','en'] as const){locale.setLocale(language);
  for(const unavailableRecords of [0,1]){
   const calculated:UsageSummary={...base,tokenAnalysis:{...base.tokenAnalysis,totalAnalysis:{methodVersion:1,subtotal:120,coveredRecords:2-unavailableRecords,recordedRecords:0,calculatedRecords:2-unavailableRecords,unavailableRecords,overflowRecords:0}}};
   const item:UsageItem={kind:'usage',isSubtotal:true,date:'2026-10-05',scope:{},usage:calculated};
   const result={snapshotRef:{createdAt:'2026-10-05T00:00:00Z'},scope:{}} as UsageResult;
   for(const width of [60,120]){
    const text=itemLines(item,result,width).join('\n');
    assert.match(text,language==='en'?/Calculated value/:/计算值/);assert.match(text,/120/);
    assert.ok(text.includes(language==='en'?'raw input':'原始输入'));
    if(unavailableRecords)assert.match(text,language==='en'?/Known subtotal/:/已知小计/);
   }
  }
 }}finally{locale.setLocale(saved);}
});
