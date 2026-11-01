import { test } from 'node:test';
import assert from 'node:assert/strict';
import { parseUsageArgs } from '../src/usage-app-cli.js';

test('timezone validation keeps named zones and Intl aliases without rewriting the request', () => {
  for (const timezone of ['UTC', 'Asia/Shanghai', 'America/New_York', 'US/Pacific', 'utc'])
    assert.equal(parseUsageArgs(['usage', '--timezone', timezone]).request.scope?.timezone, timezone);
  assert.throws(() => parseUsageArgs(['usage', '--timezone', 'Not/AZone']));
});

test('v1 arguments enforce action-specific flags and exact independent filters', () => {
  assert.deepEqual(parseUsageArgs(['usage', '--since', '2026-09-29', '--until', '2026-09-30', '--model', 'gpt-5.4', '--effort', 'high', '--timezone', 'Asia/Shanghai', '--json']).request, { action: 'usage', scope: { since: '2026-09-29', until: '2026-09-30', timezone: 'Asia/Shanghai', model: 'gpt-5.4', reasoningEffort: 'high' } });
  assert.deepEqual(parseUsageArgs(['usage', '--root', '/tmp/a', '--root=/tmp/b']).request, { action: 'usage', roots: ['/tmp/a', '/tmp/b'] });
  assert.equal(parseUsageArgs(['--version']).version, true);
  assert.deepEqual(parseUsageArgs(['usage','--agent','codex','--source','s']).request, { action:'usage',scope:{agentKind:'codex',sourceInstanceId:'s'} });
  assert.throws(() => parseUsageArgs(['--version', '--since', '2026-09-01']));
  assert.equal(parseUsageArgs([]).request.action, 'usage');
  for (const args of [['usage', '--sort', 'recent'], ['threads', '--presentation', 'distribution'], ['usage', '--presentation', 'graph'], ['refresh', '--model', 'x'], ['usage', '--fresh', '--cached'], ['steps', '--thread', 'x'], ['turns'], ['threads', '--sort', 'time'], ['refresh', '--agent', 'codex'], ['scan'], ['usage', '--limit', '501'], ['usage', '--limit', '1.5'], ['usage', '--offset', '-1'], ['usage', '--since', '2026-02-30'], ['usage', '--since', '2026-09-30', '--until', '2026-09-30'], ['usage', '--timezone', 'Not/AZone'], ['usage', '--model', 'a', '--model', 'b'], ['usage', '--json', '--json'], ['usage', 'other']])
    assert.throws(() => parseUsageArgs(args), args.join(' '));
});

test('distribution and cost sorting use the shared public request', () => {
  assert.deepEqual(parseUsageArgs(['usage','--presentation','distribution','--sort','cost']).request, { action: 'usage', presentation: 'distribution', sort: 'cost' });
});

test('unknown dimensions remain distinct from explicit names and dates', () => {
  assert.deepEqual(parseUsageArgs(['usage', '--model-unknown', '--effort-unknown', '--undated']).request.scope, { undated: true, modelUnknown: true, effortUnknown: true });
  assert.throws(() => parseUsageArgs(['usage', '--model-unknown', '--model', 'gpt-5.4']));
  assert.throws(() => parseUsageArgs(['usage', '--undated', '--since', '2026-09-29']));
});

test('live flags keep cached, fixed and watched queries distinct', () => {
  assert.equal(parseUsageArgs(['usage','--fresh']).mode,'fresh');
  assert.equal(parseUsageArgs(['usage','--cached']).mode,'cached');
  assert.equal(parseUsageArgs(['usage','--watch','--json']).watch,true);
  assert.deepEqual(parseUsageArgs(['usage','--root','/tmp/a']).request.roots,['/tmp/a']);
  for (const args of [['usage','--watch','--cached'],['usage','--snapshot','x','--fresh'],['threads','--watch'],['usage','--verify'],['refresh','--cached']]) assert.throws(()=>parseUsageArgs(args));
});
test('bounded preparation waits and explicit rebuilds do not enter usage filters',()=>{
 const wait=parseUsageArgs(['usage','--wait-ms','600000','--include-history','--json']);
 assert.equal(wait.mode,'fresh');assert.equal(wait.waitMs,600000);assert.equal(wait.includeHistory,true);
 assert.equal(wait.request.scope,undefined);
 assert.equal(parseUsageArgs(['usage','--rebuild']).rebuild,true);
 assert.equal(parseUsageArgs(['usage','--rebuild']).mode,'fresh');
 for(const args of [['usage','--wait-ms','0'],['usage','--wait-ms','600001'],['usage','--wait-ms','1.5'],['usage','--wait-ms','1','--cached'],['usage','--rebuild','--watch'],['usage','--rebuild','--snapshot','fixed'],['usage','--wait-ms','1','--wait-ms','2']]) assert.throws(()=>parseUsageArgs(args));
});

test('comparisons preserve explicit operands and never refresh implicitly',()=>{
 const p=parseUsageArgs(["usage", "compare",'--since','2026-09-08','--until','2026-09-15','--baseline-since','2026-09-01','--baseline-until','2026-09-08','--dimension','model']);
 assert.equal(p.mode,'cached');assert.deepEqual(p.request.comparison,{kind:'periods',baselineSince:'2026-09-01',baselineUntil:'2026-09-08',dimension:'model'});
 assert.deepEqual(parseUsageArgs(["usage", "compare",'--thread','a','--other-thread','b','--family']).request.comparison,{kind:'sessions',leftThreadId:'a',rightThreadId:'b',includeDescendants:true});
 for(const args of [["usage", "compare"],["usage", "compare",'--thread','a'],["usage", "compare",'--other-thread','b'],['usage','--family'],["usage", "compare",'--thread','a','--other-thread','b','--dimension','model']])assert.throws(()=>parseUsageArgs(args));
});

test('retired top-level commands fail before any query',()=>{
 for(const command of ['refresh','threads','turns','steps','statistics','compare','investigate','trajectory','resources','review','context','timing','account','prices','monitor','directories','service','doctor','setup','update','optimize','collection','hook'])assert.throws(()=>parseUsageArgs([command,'--json']),command);
});
test('compact usage and stats preserve the public request',()=>{
 assert.deepEqual(parseUsageArgs(['usage','stats','--thread','a','--compact','--json']).request,{action:'statistics',compact:true,threadId:'a'});
 for(const args of [['usage','--compact'],['usage','--compact','--compact','--json'],['usage','--search','x'],['usage','--locate-thread','x']])assert.throws(()=>parseUsageArgs(args));
});
