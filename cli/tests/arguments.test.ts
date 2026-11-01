import test from 'node:test';
import assert from 'node:assert/strict';
import path from 'node:path';
import {parseUsageArgs} from '../src/usage-app-cli.js';
import {dateWindow} from '../src/calendar.js';
const now=new Date('2026-11-01T12:00:00Z');
const parse=(args:string[])=>parseUsageArgs(args,now);

test('calendar presets use local calendar boundaries, including DST and month changes',()=>{
  const expected={today:['2026-11-01','2026-11-02'],yesterday:['2026-10-31','2026-11-01'],week:['2026-10-26','2026-11-02'],'last-week':['2026-10-19','2026-10-26'],month:['2026-11-01','2026-11-02']} as const;
  for(const [word,[since,until]]of Object.entries(expected))assert.deepEqual(parse(['usage',word,'--timezone','America/New_York']).request.scope,{since,until,timezone:'America/New_York'});
  assert.equal(dateWindow({timezone:'America/New_York'},'today',undefined,new Date('2026-11-01T02:00:00Z')).since,'2026-10-31');
  assert.equal(dateWindow({timezone:'Asia/Shanghai'},'today',undefined,new Date('2026-10-31T23:00:00Z')).since,'2026-11-01');
});

test('rankings and statistics have explicit bounded defaults without changing accounting',()=>{
  const top=parse(['usage','top','week','--timezone','UTC']);
  assert.equal(top.request.action,'threads');assert.equal(top.request.limit,5);assert.equal(top.request.sort,'tokens');assert.equal(top.request.scope?.since,'2026-10-26');
  const stats=parse(['usage','stats','--timezone','UTC']);assert.equal(stats.request.action,'statistics');assert.equal(stats.request.scope?.since,'2026-10-03');
  const cmp=parse(['usage','compare','--timezone','UTC']);assert.equal(cmp.request.scope?.until,'2026-10-26');assert.deepEqual(cmp.request.comparison,{kind:'periods',baselineSince:'2026-10-12',baselineUntil:'2026-10-19',dimension:'project'});
});

test('explicit dates, unknowns, sources and model filters retain independent semantics',()=>{
  const r=parse(['usage','--since','2026-09-29','--until','2026-09-30','--timezone','UTC','--model','gpt-6.1-sol','--effort','high','--source','s','--agent','codex','--root','a','--root','b','--json']).request;
  assert.equal(r.scope?.model,'gpt-6.1-sol');assert.equal(r.scope?.sourceInstanceId,'s');assert.equal(r.scope?.since,'2026-09-29');assert.deepEqual(r.roots,[path.resolve('a'),path.resolve('b')]);
  assert.equal(parse(['usage','--project','.']).request.scope?.project,process.cwd());
  assert.equal(parse(['usage','--all-time']).request.scope?.allTime,true);
  assert.equal(parse(['usage','--model-unknown','--undated']).request.scope?.since,undefined);
  assert.equal(parse(['usage','stats','--task','record','--compact','--json']).request.threadId,'record');
  assert.deepEqual(parse(['usage','compare','--task','a','--against','b','--family']).request.comparison,{kind:'sessions',leftThreadId:'a',rightThreadId:'b',includeDescendants:true});
  assert.equal(parse(['usage','compare','--task','a','--against','b']).request.limit,undefined);
});

test('invalid dates, conflicting selections and retired spellings fail before queries',()=>{
  for(const args of [
    ['usage','week','--since','2026-10-01','--until','2026-10-03'],['usage','--since','2026-10-01'],['usage','--since','2026-02-30','--until','2026-03-02'],
    ['usage','--since','2026-10-02','--until','2026-10-01'],['usage','--all-time','--until','2026-11-01'],['usage','--timezone','Not/AZone'],
    ['usage','--json','--json'],['usage','--limit','201'],['usage','--limit','1.5'],['usage','--offset','-1'],['usage','top','--sort','time'],
    ['usage','--thread','a'],['usage','compare','--task','a'],['usage','compare','--against','b'],['usage','--family'],['usage','--compact'],
    ['usage','compare','--task','a','--against','b','--limit','5'],
    ['usage','--fresh','--cached'],['usage','--watch','--cached'],['usage','--wait-ms','0'],['usage','--rebuild','--watch'],['usage','--model','a','--model','b'],
  ])assert.throws(()=>parse(args),args.join(' '));
});

test('live wait, rebuild and fixed view controls retain their execution meanings',()=>{
  const r=parse(['usage','--wait-ms','600000','--include-history','--json']);assert.equal(r.mode,'fresh');assert.equal(r.waitMs,600000);assert.equal(r.includeHistory,true);
  assert.equal(parse(['usage','--cached']).mode,'cached');assert.equal(parse(['usage','--rebuild']).rebuild,true);assert.equal(parse(['--version']).version,true);
  for(const timezone of ['UTC','Asia/Shanghai','America/New_York'])assert.equal(parse(['usage','--timezone',timezone]).request.scope?.timezone,timezone);
});
