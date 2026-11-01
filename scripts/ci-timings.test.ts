import assert from 'node:assert/strict';
import test from 'node:test';
import {jobTiming,stepPhase} from './ci-timings.ts';
test('separates dependency waiting, runner queue, preparation and validation',()=>{
  const time=(s:number)=>new Date(Date.UTC(2026,0,1,0,0,s)).toISOString();
  const run={id:1,name:'CI',created_at:time(0),updated_at:time(60),head_sha:'a',conclusion:'success',html_url:'https://example.invalid/run'};
  const job={id:2,name:'Verify',created_at:time(10),started_at:time(15),completed_at:time(60),conclusion:'success',html_url:'https://example.invalid/job',steps:[{name:'Set up job',started_at:time(15),completed_at:time(20),conclusion:'success'},{name:'Release gate',started_at:time(20),completed_at:time(60),conclusion:'success'}]};
  const row=jobTiming(run,job,`${time(20)} === Build ===\n${time(40)} === Tests ===`);
  assert.deepEqual([row.dependencyWaitSeconds,row.queueSeconds,row.preparationSeconds,row.verificationSeconds,row.unclassifiedSeconds],[10,5,25,20,0]);
  assert.equal(jobTiming(run,job).unclassifiedSeconds,40);
  assert.equal(jobTiming(run,{...job,started_at:null,completed_at:null}).executionSeconds,null);
  assert.equal(stepPhase('Compile Rust tests'),'preparation');
  assert.equal(stepPhase('Run Rust tests and doc tests'),'verification');
});
