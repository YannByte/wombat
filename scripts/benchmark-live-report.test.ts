import {test} from 'node:test';
import assert from 'node:assert/strict';
import {compareLiveReports} from './benchmark-live-report.js';
function sample(bytes:number,ms=20) {return {benchmark:'live-index-cold-append-rebuild',platform:'fixture',nodeVersion:'fixture',corpus:{initial:{sha256:'fixed'},sourceBytesStableDuringSnapshotAndRebuild:true,appendedSourceReusedForColdRebuild:true},expectedTruth:{tokens:7},consistency:Object.fromEntries(['initialTruthPassed','eachAppendConserved','usageDailySubtotalsConserved','threadPagesAndAmountsConserved','fixedSnapshotMatches','coldRebuildMatches'].map(key=>[key,true])),coldRebuild:{dataHomeBytesAfterIdleExit:bytes,databaseBytesBeforeIdleExit:bytes,walBytesBeforeIdleExit:10},foreground:{scope:'synthetic',queries:[{name:'query',samples:[{elapsedMs:ms,responseBytes:10}]}]}};}
test('comparison counts complete settled storage and every latency violation',()=>{
 const pass=compareLiveReports(sample(1000),sample(100));assert.equal(pass.storage.tenfoldPassed,true);assert.equal(pass.storage.baselineDatabaseAndWalBeforeExit,1010);assert.equal(pass.storage.candidateDatabaseAndWalBeforeExit,110);assert.equal(pass.foreground.within100ms,true);
 const fail=compareLiveReports(sample(1000),sample(101,101));assert.equal(fail.storage.tenfoldPassed,false);assert.equal(fail.foreground.within100ms,false);assert.equal(fail.foreground.queries[0].violations,1);
});
test('mismatched source, failed truth, and absent or invalid measurements cannot pass',()=>{
 const baseline=sample(1000);for(const mutate of [(r:ReturnType<typeof sample>)=>{r.corpus.initial.sha256='changed'},(r:ReturnType<typeof sample>)=>{r.consistency.fixedSnapshotMatches=false},(r:ReturnType<typeof sample>)=>{r.foreground.queries=[]},(r:ReturnType<typeof sample>)=>{r.coldRebuild.dataHomeBytesAfterIdleExit=0},(r:ReturnType<typeof sample>)=>{r.foreground.queries[0].samples[0].elapsedMs=NaN}]) {const candidate=sample(100);mutate(candidate);assert.throws(()=>compareLiveReports(baseline,candidate));}
});
