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
  assert.deepEqual(parseUsageArgs(['refresh', '--root', '/tmp/a', '--root=/tmp/b']).request, { action: 'refresh', roots: ['/tmp/a', '/tmp/b'] });
  assert.equal(parseUsageArgs(['--version']).version, true);
  assert.deepEqual(parseUsageArgs(['threads','--agent','codex','--source','s','--locate-thread','id']).request, { action:'threads',locateThreadId:'id',scope:{agentKind:'codex',sourceInstanceId:'s'} });
  assert.throws(() => parseUsageArgs(['--version', '--since', '2026-09-01']));
  assert.equal(parseUsageArgs([]).request.action, 'usage');
  for (const args of [['usage', '--sort', 'recent'], ['threads', '--presentation', 'distribution'], ['usage', '--presentation', 'graph'], ['refresh', '--model', 'x'], ['usage', '--fresh', '--cached'], ['steps', '--thread', 'x'], ['turns'], ['threads', '--sort', 'time'], ['refresh', '--agent', 'codex'], ['scan'], ['usage', '--limit', '501'], ['usage', '--limit', '1.5'], ['usage', '--offset', '-1'], ['usage', '--since', '2026-02-30'], ['usage', '--since', '2026-09-30', '--until', '2026-09-30'], ['usage', '--timezone', 'Not/AZone'], ['usage', '--model', 'a', '--model', 'b'], ['usage', '--json', '--json'], ['usage', 'other']])
    assert.throws(() => parseUsageArgs(args), args.join(' '));
});

test('distribution and cost sorting use the shared public request', () => {
  assert.deepEqual(parseUsageArgs(['usage','--presentation','distribution','--sort','cost']).request, { action: 'usage', presentation: 'distribution', sort: 'cost' });
  assert.equal(parseUsageArgs(['threads','--sort','cost']).request.sort, 'cost');
  assert.equal(parseUsageArgs(['turns','--thread','fixture','--sort','cost']).request.sort, 'cost');
  assert.equal(parseUsageArgs(['turns','--thread','fixture','--sort','recent']).request.sort, 'recent');
});

test('unknown dimensions remain distinct from explicit names and dates', () => {
  assert.deepEqual(parseUsageArgs(['threads', '--model-unknown', '--effort-unknown', '--undated']).request.scope, { undated: true, modelUnknown: true, effortUnknown: true });
  assert.throws(() => parseUsageArgs(['threads', '--model-unknown', '--model', 'gpt-5.4']));
  assert.throws(() => parseUsageArgs(['usage', '--undated', '--since', '2026-09-29']));
});

test('live flags keep cached, fixed and watched queries distinct', () => {
  assert.equal(parseUsageArgs(['usage','--fresh']).mode,'fresh');
  assert.equal(parseUsageArgs(['usage','--cached']).mode,'cached');
  assert.equal(parseUsageArgs(['usage','--watch','--json']).watch,true);
  assert.equal(parseUsageArgs(['refresh','--verify']).verify,true);
  assert.deepEqual(parseUsageArgs(['usage','--root','/tmp/a']).request.roots,['/tmp/a']);
  for (const args of [['usage','--watch','--cached'],['usage','--snapshot','x','--fresh'],['threads','--watch'],['usage','--verify'],['refresh','--cached']]) assert.throws(()=>parseUsageArgs(args));
});

test('matching turns and turn location are only accepted by turns',()=>{
 assert.deepEqual(parseUsageArgs(['turns','--thread','id','--matched-only','--locate-turn','t']).request,{action:'turns',threadId:'id',matchedOnly:true,locateTurnId:'t'});
 for(const action of ['usage','threads','steps','refresh']) {
  assert.throws(()=>parseUsageArgs([action,'--matched-only']));
  assert.throws(()=>parseUsageArgs([action,'--locate-turn','t']));
 }
});

test('comparisons preserve explicit operands and never refresh implicitly',()=>{
 const p=parseUsageArgs(['compare','--since','2026-09-08','--until','2026-09-15','--baseline-since','2026-09-01','--baseline-until','2026-09-08','--dimension','model']);
 assert.equal(p.mode,'cached');assert.deepEqual(p.request.comparison,{kind:'periods',baselineSince:'2026-09-01',baselineUntil:'2026-09-08',dimension:'model'});
 assert.deepEqual(parseUsageArgs(['compare','--thread','a','--other-thread','b','--family']).request.comparison,{kind:'sessions',leftThreadId:'a',rightThreadId:'b',includeDescendants:true});
 for(const args of [['compare'],['compare','--thread','a'],['compare','--other-thread','b'],['usage','--family'],['compare','--thread','a','--other-thread','b','--dimension','model']])assert.throws(()=>parseUsageArgs(args));
});

test('inspection plans retain fixed scopes, require trajectory identity and default to committed data',()=>{
 for(const action of ['investigate','resources','review','context'])assert.equal(parseUsageArgs([action]).mode,'cached');
 assert.equal(parseUsageArgs(['trajectory','--thread','complete-id','--snapshot','fixed','--all-time']).request.scope?.threadId,'complete-id');
 for(const args of [['trajectory'],['review','--all-time'],['review','--limit','5'],['review','--since','2026-09-01'],['resources','--sort','cost']])assert.throws(()=>parseUsageArgs(args));
});

test('operation evidence location is restricted to a full turn record query',()=>{
 assert.equal(parseUsageArgs(['steps','--thread','a','--turn','b','--locate-operation','op']).request.locateOperationId,'op');
 assert.throws(()=>parseUsageArgs(['threads','--locate-operation','op']));
});

test('compact JSON and selected-turn investigation preserve public scope',()=>{
 const r=parseUsageArgs(['investigate','--snapshot','fixed','--project','/synthetic','--thread','a','--turn','b','--compact','--json']).request;
 assert.equal(r.compact,true);assert.deepEqual(r.scope,{project:'/synthetic',threadId:'a',turnId:'b'});
 for(const args of [['usage','--compact'],['refresh','--compact','--json'],['investigate','--turn','b','--json'],['trajectory','--thread','a','--turn','b'],['usage','--compact','--compact','--json']])assert.throws(()=>parseUsageArgs(args));
});
