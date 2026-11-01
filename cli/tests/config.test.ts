import { test } from 'node:test';
import assert from 'node:assert/strict';
import path from 'node:path';
import { parseConfigArgs } from '../src/config-cli.js';
test('extension review resolves local timezone and preserves explicit scope and ordering', () => {
  const r = parseConfigArgs(['--review','--kind','skill']).request;
  assert.equal(r.review, true); assert.equal(r.sort, 'review');
  assert.equal(r.scope?.timezone, Intl.DateTimeFormat().resolvedOptions().timeZone);
  assert.equal(parseConfigArgs(['--review','--timezone','UTC','--sort','context_tokens']).request.scope?.timezone, 'UTC');
  assert.equal(parseConfigArgs(['--review','--sort','least_activity']).request.sort, 'least_activity');
  assert.throws(() => parseConfigArgs(['--review=true']));
  assert.throws(() => parseConfigArgs(['--review','--review']));
});
test('configuration arguments preserve scope and reject ambiguous flags', () => {
  const result = parseConfigArgs(['--root','/one','--root=/two','--project-root','/project','--kind','skill','--json']);
  assert.deepEqual(result.request.roots, [path.resolve('/one'), path.resolve('/two')]);
  assert.deepEqual(result.request.projectRoots, [path.resolve('/project')]);
  assert.equal(result.request.kind, 'skill'); assert.equal(result.json, true);
  for (const args of [['--json=true'], ['--limit','0'], ['--limit','201'], ['--offset','1.5'], ['--kind','unknown'], ['--sort','name','--sort','size'], ['--write'], ['--project-root']]) assert.throws(() => parseConfigArgs(args));
});

test('all dates and measured-content sorts have unambiguous CLI contracts', () => {
  assert.equal(parseConfigArgs(['--all-time','--sort','content_tokens']).request.scope?.allTime,true);
  assert.throws(()=>parseConfigArgs(['--all-time','--since','2026-10-01']));
});
test('object review arguments retain versions and expose only narrow actions', async () => {
  const {parseOptimizeArgs}=await import('../src/optimize-cli.js');
  const r=parseOptimizeArgs(['keep','--reason','necessary','--suggestion','one','--read-view','config:one','--decision-revision','2','--json']);
  assert.equal(r.request.action,'keep');assert.equal(r.request.suggestionId,'one');assert.equal(r.request.decisionRevision,'2');
  assert.throws(()=>parseOptimizeArgs(['history']));
  for(const args of [['keep'],['execute'],['list','--shell','anything'],['list','--limit','201'],['list','--json=true']])assert.throws(()=>parseOptimizeArgs(args));
});

test('product reminder overrides cannot change specification thresholds',async()=>{
 const {parseOptimizeArgs}=await import('../src/optimize-cli.js');
 assert.deepEqual(parseOptimizeArgs(['list','--agents-bytes','20000','--description-characters','1024']).request.ruleOverrides,{agentsBytes:20000,descriptionCharacters:1024});
 for(const args of [['--agents-bytes','0'],['--description-characters','1025'],['--description-characters','-1'],['--body-tokens','10000']])assert.throws(()=>parseOptimizeArgs(args));
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
