/** Public, source-free comparison of two real-core benchmark runs. */
import assert from 'node:assert/strict';
import {readFileSync,statSync,writeFileSync} from 'node:fs';
import {option} from './benchmark-common.js';

// Compare the shared evidence contract. Layout-specific producer metadata is
// forwarded as unknown rather than pretending to support every historical schema.
interface LatencySample {elapsedMs:number;responseBytes:number}
interface Report {
  benchmark:string; platform:string; nodeVersion:string;
  corpus:{sourceBytesStableDuringSnapshotAndRebuild:boolean;appendedSourceReusedForColdRebuild:boolean;[key:string]:unknown};
  expectedTruth:unknown; build?:unknown; consistency:Record<string,boolean>;
  coldRebuild:{dataHomeBytesAfterIdleExit:number;databaseBytesBeforeIdleExit:number;walBytesBeforeIdleExit:number;pages?:unknown;peakRssBytes?:number;syncAndCliMs?:number};
  coldRestore?:unknown;
  foreground?:{scope:string;queries:{name:string;samples:LatencySample[]}[]};
}
export function compareLiveReports(baseline:Report,candidate:Report) {
  for(const report of [baseline,candidate]) {
    assert.equal(report.benchmark,'live-index-cold-append-rebuild');
    assert.equal(report.corpus.sourceBytesStableDuringSnapshotAndRebuild,true);
    assert.equal(report.corpus.appendedSourceReusedForColdRebuild,true);
    for(const key of ['initialTruthPassed','eachAppendConserved','usageDailySubtotalsConserved','threadPagesAndAmountsConserved','fixedSnapshotMatches','coldRebuildMatches']) assert.equal(report.consistency[key],true,key);
  }
  assert.deepEqual(candidate.corpus,baseline.corpus,'Comparisons require the same frozen source corpus');
  assert.deepEqual(candidate.expectedTruth,baseline.expectedTruth,'Independent expected truth changed');
  assert.equal(candidate.platform,baseline.platform,'Platform changed');
  assert.equal(candidate.nodeVersion,baseline.nodeVersion,'Node version changed');
  const settled=(report:Report)=>{
    const bytes=report.coldRebuild.dataHomeBytesAfterIdleExit;
    assert(Number.isSafeInteger(bytes)&&bytes>0,'Missing settled product-directory size');
    return bytes as number;
  };
  const baselineBytes=settled(baseline),candidateBytes=settled(candidate);
  assert(Array.isArray(candidate.foreground?.queries)&&candidate.foreground.queries.length>0,'Missing foreground request measurements');
  const queries=candidate.foreground.queries.map(query=>{
    assert(Array.isArray(query.samples)&&query.samples.length>0,'Missing latency samples');
    for(const sample of query.samples) assert(Number.isFinite(sample.elapsedMs)&&sample.elapsedMs>=0&&Number.isSafeInteger(sample.responseBytes)&&sample.responseBytes>0,'Invalid latency sample');
    const ordered=query.samples.map(s=>s.elapsedMs).sort((a:number,b:number)=>a-b);
    const percentile=(p:number)=>ordered[Math.ceil(ordered.length*p)-1];
    return {name:query.name,samples:ordered.length,firstMs:query.samples[0].elapsedMs,p50Ms:percentile(.5),p95Ms:percentile(.95),p99Ms:percentile(.99),maxMs:ordered.at(-1),violations:ordered.filter((ms:number)=>ms>100).length,responseBytes:Math.max(...query.samples.map(s=>s.responseBytes))};
  });
  return {
    formatVersion:1,benchmark:'production-live-index-comparison',generatedAt:new Date().toISOString(),
    platform:candidate.platform,nodeVersion:candidate.nodeVersion,corpus:candidate.corpus,expectedTruth:candidate.expectedTruth,
    builds:{baseline:baseline.build,candidate:candidate.build},
    storage:{basis:'logical bytes of the complete fresh product data directory after service exit; no saved snapshot in this directory; all index tables included',baselineBytes,candidateBytes,reduction:baselineBytes/candidateBytes,tenfoldPassed:candidateBytes*10<=baselineBytes,baselineDatabaseAndWalBeforeExit:baseline.coldRebuild.databaseBytesBeforeIdleExit+baseline.coldRebuild.walBytesBeforeIdleExit,candidateDatabaseAndWalBeforeExit:candidate.coldRebuild.databaseBytesBeforeIdleExit+candidate.coldRebuild.walBytesBeforeIdleExit,pages:candidate.coldRebuild.pages},
    foreground:{scope:candidate.foreground.scope,queries,within100ms:queries.every(q=>q.violations===0)},
    service:{baselinePeakRssBytes:baseline.coldRebuild.peakRssBytes,candidatePeakRssBytes:candidate.coldRebuild.peakRssBytes,baselineRebuildMs:baseline.coldRebuild.syncAndCliMs,candidateRebuildMs:candidate.coldRebuild.syncAndCliMs,baselineRestore:baseline.coldRestore,candidateRestore:candidate.coldRestore},
    consistency:candidate.consistency,
    limitations:['Synthetic Rust-parsed corpus on one platform; source logs and user records were not scanned.','Filesystem caches uncontrolled; no physical-cache-cold or allocated-block claim. Database plus WAL sizes are observations before exit, not sampled transaction maxima.','Single-reader complete CLI requests, including first queries against a ready view; collection, synchronization, restore and export are measured separately.','This scoped result does not accept the full 3,000,000-event, concurrency, source-backed-detail, or bounded-residency proposal.'],
  };
}
export function writeLiveComparison() {
  const read=(name:string)=>{const file=option(name);assert(statSync(file).size<=16*1024*1024,'Benchmark report too large');return JSON.parse(readFileSync(file,'utf8'));};
  const report=compareLiveReports(read('--compare-baseline'),read('--compare-candidate'));
  const output=option('--output');writeFileSync(output,JSON.stringify(report,null,2)+'\n');
  console.log(JSON.stringify({output,storage:report.storage,foreground:report.foreground}));
  if(!report.storage.tenfoldPassed||!report.foreground.within100ms)process.exitCode=1;
}
