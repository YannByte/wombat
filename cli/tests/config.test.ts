import {test} from 'node:test';
import assert from 'node:assert/strict';
import path from 'node:path';
import {parseVerificationArgs} from '../src/optimize-cli.js';
test('verification rejects removed view controls and requires an original report',()=>{
 for(const args of [['recheck'],['list'],['keep'],['verify'],['verify','--read-view','v'],['verify','--baseline','missing-report.json']])assert.throws(()=>parseVerificationArgs(args));
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
