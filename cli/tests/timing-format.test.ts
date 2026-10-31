import test from 'node:test';
import assert from 'node:assert/strict';
import { locale } from '@wombat/client/locale';
import { renderTimingResult, timingExitCode } from '../src/timing-format.js';
import { local, share, capabilityResult } from './timing-fixtures.js';
import type { TimingResult } from '@wombat/client';

test('human timing preserves explicit zero, basis-specific missing measures and concurrent interval durations', () => {
  locale.setLocale('en');
  const fixture = structuredClone(local);
  fixture.time.nativeWallClockMs = { value: 0, status: 'observed', basis: 'native_record', evidenceRefs: [] };
  fixture.time.derivedWallClockMs = { value: 40, status: 'derived', basis: 'explicit_boundary', evidenceRefs: [] };
  fixture.time.command.unionMs.value = 30; fixture.time.command.sumMs.value = 50;
  fixture.time.compaction.unionMs.value = 0;
  fixture.time.reasoning.unionMs.value = 20; fixture.time.reasoning.sumMs.value = 35;
  const text = renderTimingResult(fixture);
  assert.match(text, /Native total duration: 0 ms/); assert.match(text, /Boundary-derived total duration: 40 ms/);
  assert.match(text, /Native time to first Token: The source did not record this measure/); assert.match(text, /Commands: union 30 ms; sum 50 ms/);
  assert.match(text, /^Compaction: union 0 ms; sum The source did not record this measure$/m);
  assert.match(text, /Reasoning: union 20 ms; sum 35 ms/); assert.match(text, /category sums are not total duration/);
  assert.doesNotMatch(text, /total duration: 85/);
  assert.doesNotMatch(text, /Turn state not established/);
  locale.setLocale('zh'); const zh = renderTimingResult(fixture);
  assert.match(zh, /原生总耗时: 0 ms/); assert.match(zh, /边界推导总耗时: 40 ms/); assert.match(zh, /原生首 Token 延迟: 来源未记录此测量/);
  assert.match(zh, /并集 30 ms；相加 50 ms/);
  assert.match(zh, /^压缩：并集 0 ms；相加 来源未记录此测量$/m);
});
test('share rendering consumes only core projection aliases and strips terminal controls from display data', () => {
  locale.setLocale('en');
  const text = renderTimingResult(share);
  assert.match(text, /task-1 \/ turn-1/); assert.doesNotMatch(text, /live:scope:fixed|sourceInstanceId|Fixed snapshot/);
  const fixture = structuredClone(local); fixture.scope.threadId = '\u001b[31mthread\u001b[0m';
  assert.doesNotMatch(renderTimingResult(fixture), /\u001b/);
});
test('exit classification uses core quality without making missing optional capabilities an error', () => {
  const complete = structuredClone(local); complete.quality.partial = false; complete.quality.running = false; complete.quality.censored = false;
  assert.equal(timingExitCode(complete), 0); assert.equal(timingExitCode(capabilityResult), 0);
  for (const field of ['partial', 'running', 'censored'] as const) assert.equal(timingExitCode({ ...complete, quality: { ...complete.quality, [field]: true } }), 2);
  locale.setLocale('en'); assert.match(renderTimingResult(capabilityResult), /Total duration: Unavailable/);
});
test('Work facts preserve observed zero, unavailable counts, source failure and physical-record caveats', () => {
  const fixture = structuredClone(local);
  fixture.work.operationCandidates = { value: 0, status: 'observed', basis: 'safe_event_count', evidenceRefs: [] };
  fixture.work.closedOperations = { value: 2, status: 'observed', basis: 'safe_event_count', evidenceRefs: [] };
  fixture.work.fileChangeRecords = { value: 3, status: 'observed', basis: 'safe_event_count', evidenceRefs: [] };
  fixture.work.changedFiles = { value: 1, status: 'derived', basis: 'reported_file_paths', evidenceRefs: [] };
  fixture.work.addedLines = { value: null, status: 'unavailable', basis: 'missing_repository_baseline', evidenceRefs: [] };
  fixture.work.removedLines = { value: null, status: 'unavailable', basis: 'missing_repository_baseline', evidenceRefs: [] };
  fixture.work.labelledCommandMs = { value: null, status: 'unavailable', basis: 'unsupported_method', evidenceRefs: [] };
  fixture.work.userBoundaryRecords = { value: 4, status: 'observed', basis: 'safe_event_count', evidenceRefs: [] };
  fixture.coverage.sourceStatus = 'failed';
  fixture.quality.partial = true;

  locale.setLocale('en');
  const en = renderTimingResult(fixture);
  assert.match(en, /Operation candidates \/ closed \/ failed: 0 \/ 2 \/ The source did not record this measure/);
  assert.match(en, /Reported file paths: 1 · Derived · Basis: Source-reported file paths \(reported_file_paths\)/);
  assert.doesNotMatch(en, /Added lines|Removed lines|Command duration by label/);
  assert.match(en, /addedLines=missing_repository_baseline/);
  assert.match(en, /removedLines=missing_repository_baseline/);
  assert.match(en, /labelledCommandMs=unsupported_method/);
  assert.match(en, /User input records: 4 · Source record · Basis: Event record count \(safe_event_count\)/);
  assert.match(en, /Source coverage: Source reading failed/);
  assert.match(en, /Read quality: Partial results/);
  assert.match(en, /physical records, not requests/);
  assert.match(en, /not code defects/);

  locale.setLocale('zh');
  const zh = renderTimingResult(fixture);
  assert.match(zh, /操作候选 \/ 已闭合 \/ 失败: 0 \/ 2 \/ 来源未记录此测量/);
  assert.match(zh, /报告的文件路径数: 1 · 推导 · 依据: 来源报告的文件路径 \(reported_file_paths\)/);
  assert.doesNotMatch(zh, /新增行数|删除行数|按命令标签归类的时长/);
  assert.match(zh, /addedLines=missing_repository_baseline/);
  assert.match(zh, /removedLines=missing_repository_baseline/);
  assert.match(zh, /labelledCommandMs=unsupported_method/);
  assert.match(zh, /用户输入记录数: 4 · 源记录 · 依据: 事件记录计数 \(safe_event_count\)/);
  assert.match(zh, /来源覆盖: 来源读取失败/);
  assert.match(zh, /读取质量: 结果不完整/);
  assert.match(zh, /物理记录数，不是请求数/);
  assert.match(zh, /不代表代码缺陷/);
});
test('localized timing reasons replace bare protocol codes and running differs from censored scope', () => {
  const fixture = structuredClone(local);
  fixture.quality.partial = true; fixture.quality.running = true; fixture.quality.censored = true;
  fixture.quality.reasonCodes = ['missing_time', 'source_partial', 'boundary_conflict', 'dispatch_not_proven'];
  fixture.coverage.sourceStatus = 'partial';
  locale.setLocale('en');
  const en = renderTimingResult(fixture);
  assert.match(en, /Reasons: Usable timing records are missing for calculation or placement \(missing_time\)/);
  assert.match(en, /Some source records or required evidence are incomplete \(source_partial\)/);
  assert.match(en, /Boundary or content evidence conflicts across records \(boundary_conflict\)/);
  assert.match(en, /The records do not establish that the call was dispatched \(dispatch_not_proven\)/);
  assert.match(en, /The turn is still running; only observed records are shown/);
  assert.match(en, /Results are limited to the current observation window/);
  locale.setLocale('zh');
  const zh = renderTimingResult(fixture);
  assert.match(zh, /具体原因: 缺少可用于计算或定位的时间记录 \(missing_time\)/);
  assert.match(zh, /部分来源记录或所需依据不完整 \(source_partial\)/);
  assert.match(zh, /记录中的边界或内容依据存在冲突 \(boundary_conflict\)/);
  assert.match(zh, /记录不足以确认调用已发出 \(dispatch_not_proven\)/);
  assert.match(zh, /轮次仍在进行，当前只展示已观察到的记录/);
  assert.match(zh, /结果受当前观察范围限制，不代表完整轮次/);
});
test('object and record pages preserve missing counts, use outcomes and safe display text', () => {
  locale.setLocale('en');
  const zero = { value: 0, status: 'observed', basis: 'safe_event_count', evidenceRefs: [] } as const;
  const base = { outputVersion: 6, action: 'evidence' as const, methodVersion: local.methodVersion, profile: 'local' as const, snapshotId: local.readView.snapshotId,
    scope: local.scope, totals: local.uses.totals, total: { ...zero, value: 1, evidenceRefs: [] }, nextCursor: { token: 'next' } };
  const objects: TimingResult = { ...base, collection: 'use_objects', rows: [{
    objectRef: 'object-1', kind: 'skill', state: 'used', path: '/synthetic/\u001b[31mSKILL.md', server: null, project: null,
    associatedUseCount: { ...zero, value: 1, evidenceRefs: [] }, useCount: local.uses.totals.objectCount,
    recordCount: { ...zero, value: 1, evidenceRefs: [] }, unassignedTurnRecords: { ...zero, evidenceRefs: [] }, coverage: local.uses.totals.coverage,
  }] };
  const text = renderTimingResult(objects);
  assert.match(text, /object-1\tSkill\tConfirmed use/); assert.match(text, /The source did not record this measure/); assert.match(text, /1 · Source record/); assert.doesNotMatch(text, /\u001b/);
  const records: TimingResult = { ...base, collection: 'use_records', objectRef: 'object-1', rows: [{
    reference: 'use-1', objectRef: 'object-1', kind: 'skill_read', state: 'used', outcome: 'failed', timestampMs: null,
    timeBasis: 'unknown', nativeDurationMs: null, tool: 'read', exitCode: 1, identityKnown: true,
    replayOf: null, targetConflict: false, gapCodes: ['time_missing'],
  }, {
    reference: 'use-2', objectRef: 'object-1', kind: null, state: 'candidate', outcome: 'unknown', timestampMs: null,
    timeBasis: 'unknown', nativeDurationMs: null, tool: 'read', exitCode: null, identityKnown: true,
    replayOf: null, targetConflict: false, gapCodes: ['operation_result_conflict', 'time_missing'],
  }, {
    reference: 'use-3', objectRef: 'object-1', kind: 'mcp_tool', state: 'used', outcome: 'declined', timestampMs: 1,
    timeBasis: 'source_operation_time', nativeDurationMs: null, tool: null, exitCode: null, identityKnown: true,
    replayOf: null, targetConflict: false, gapCodes: [],
  }] };
  const recordText = renderTimingResult(records);
  assert.match(recordText, /use-1\tobject-1\tTargeted Skill read\tConfirmed use\tFailed\t\t\tread/);
  assert.match(recordText, /use-2\tobject-1\tUse record\tCandidate, dispatch unconfirmed\t\t\t\tread/);
  assert.match(recordText, /Result records conflict/);
  assert.match(recordText, /Declined/);
  assert.doesNotMatch(recordText, /Outcome not established|Turn state not established/);
  assert.match(recordText, /operation_result_conflict, time_missing/); assert.equal(timingExitCode(records), 0);
  locale.setLocale('zh'); assert.match(renderTimingResult(records), /已拒绝/);
});


test('partial object counts retain observed associations including zero without presenting a full total', () => {
 const saved=locale.getLocale();
 try {for(const language of ['zh','en'] as const){locale.setLocale(language);
  for(const count of [0,2]){
   const metric={value:count,status:'observed' as const,basis:'canonical_use_identity' as const,evidenceRefs:[]};
   const missing={value:null,status:'unavailable' as const,basis:'missing_target' as const,evidenceRefs:[]};
   const result:TimingResult={outputVersion: 6,action:'evidence',profile:'local',methodVersion:local.methodVersion,snapshotId:local.readView.snapshotId,scope:local.scope,totals:local.uses.totals,total:{...metric,value:1},nextCursor:null,collection:'use_objects',rows:[{objectRef:'synthetic-object',kind:'mcp',state:'used',path:null,server:'synthetic-server',project:null,useCount:missing,associatedUseCount:metric,recordCount:{...metric,value:3},unassignedTurnRecords:{...metric,value:0},coverage:local.uses.totals.coverage}]};
   const text=renderTimingResult(result);
   assert.match(text,new RegExp(`已关联次数: ${count}|Associated uses: ${count}`));
   assert.match(text,/完整使用次数|Full use count/);
   assert.match(text,/证据缺口|evidence gaps/);
   assert.match(text,/0 不证明未使用|0 does not prove absence of use/);
   assert.doesNotMatch(text,/使用次数: 0|\tUses: 0/);
  }
 }}finally{locale.setLocale(saved);}
});

test('MCP timing and four-way overlap use core measurements without summing categories in either locale',()=>{
 const previous=locale.getLocale();
 const fixture=structuredClone(local);
 const measure=(value:number)=>({value,status:'derived' as const,basis:'interval_mask' as const,evidenceRefs:[]});
 fixture.time.mcp.unionMs=measure(5000);fixture.time.mcp.sumMs=measure(7000);
 fixture.time.intersectionMasksMs=[measure(0), measure(0), measure(0), measure(0), measure(0), measure(0), measure(0), measure(0), measure(0), measure(0), measure(0), measure(0), measure(0), measure(0), measure(0), measure(0)];fixture.time.intersectionMasksMs[15]=measure(1000);
 fixture.time.coveredMs=measure(8000);fixture.time.unclassifiedMs=measure(2000);
 try{for(const language of ['en','zh'] as const){locale.setLocale(language);const text=renderTimingResult(fixture);
  assert.match(text,language==='zh'?/MCP 调用：并集 5000 ms；相加 7000 ms/:/MCP calls: union 5000 ms; sum 7000 ms/);
  assert.match(text,language==='zh'?/命令 ∩ 压缩 ∩ 推理 ∩ MCP 调用: 1000 ms/:/Commands ∩ Compaction ∩ Reasoning ∩ MCP calls: 1000 ms/);
  assert.match(text,language==='zh'?/已覆盖区间: 8000 ms/:/Covered intervals: 8000 ms/);
  assert.doesNotMatch(text,/12000 ms/);
 }}finally{locale.setLocale(previous);}
});


test('timing evidence retains native scalars without timestamps and preserves explicit zero',()=>{
 const saved=locale.getLocale();
 try{for(const language of ['zh','en'] as const){locale.setLocale(language);
  for(const duration of [0,25]){
   const base={outputVersion: 6 as const,action:'evidence' as const,methodVersion:local.methodVersion,profile:'local' as const,snapshotId:local.readView.snapshotId,scope:local.scope,total:{value:1,status:'observed' as const,basis:'safe_event_count' as const,evidenceRefs:[]},nextCursor:null};
   const events:TimingResult={...base,collection:'turn_events',rows:[{reference:'event-1',recordKind:'turn',phase:'completed',timestampMs:null,durationMs:duration,firstTokenMs:0,gapCodes:['missing_time']}]};
   const eventText=renderTimingResult(events);
   assert.ok(eventText.includes(`${language==='zh'?'原生耗时':'Native duration'}: ${duration} ms`));
   assert.ok(eventText.includes(`${language==='zh'?'原生首 Token 延迟':'Native time to first Token'}: 0 ms`));
   const records:TimingResult={...base,collection:'use_records',objectRef:null,totals:local.uses.totals,rows:[{reference:'use-1',objectRef:null,kind:'mcp_tool',state:'used',outcome:'unknown',timestampMs:null,timeBasis:'unknown',nativeDurationMs:duration,tool:'lookup',exitCode:0,identityKnown:true,replayOf:null,targetConflict:false,gapCodes:['missing_time']}]};
   const text=renderTimingResult(records);
   assert.ok(text.includes(`${language==='zh'?'原生操作耗时':'Native operation duration'}: ${duration} ms`));
   assert.ok(text.includes(`${language==='zh'?'退出码':'Exit code'}: 0`));
   assert.ok(text.includes(language==='zh'?'已确认使用':'Confirmed use'));
   const absent={...records,rows:[{...records.rows[0],nativeDurationMs:null,exitCode:null}]};
   const unavailable=renderTimingResult(absent);assert.doesNotMatch(unavailable,/Native operation duration|原生操作耗时|Exit code|退出码/);
  }
 }}finally{locale.setLocale(saved);}
});

test('operation residual renders supplied durations and coverage reasons in both languages without a wait label',()=>{
 const fixture=structuredClone(local),c=fixture.time.operationCoverage;
 c.coveredMs={value:0,status:'derived',basis:'operation_union',evidenceRefs:[]};
 c.residualMs={value:30,status:'derived',basis:'operation_residual',evidenceRefs:[]};
 c.residualRangeCount={value:1,status:'derived',basis:'operation_residual',evidenceRefs:[]};
 c.residualRanges=[{startMs:0,endMs:30}];c.reasonCodes=['no_paired_operations','identity_gaps','source_partial','detail_limit'];
 const previous=locale.getLocale();
 try {for(const language of ['en','zh'] as const){locale.setLocale(language);const text=renderTimingResult(fixture);assert.match(text,/0 ms/);assert.match(text,/30 ms/);assert.match(text,/0–30 ms/);assert.doesNotMatch(text,/execution\.operations\.|undefined/);assert.match(text,language==='en'?/no operation duration was subtracted/:/没有从轮次窗口减去操作时长/);}}
 finally {locale.setLocale(previous);}
});

test('repeat text distinguishes confirmed counts, request observations, subtotals and missing durations in both locales',()=>{
 const previous=locale.getLocale();
 try {for(const language of ['en','zh'] as const){
  locale.setLocale(language);const fixture=structuredClone(local),r=fixture.time.repeatedBehavior;
  const m=(value:number)=>({value,status:'derived' as const,basis:'repeat_after_failure' as const,evidenceRefs:[]});
  r.afterFailure.count=m(2);r.afterFailure.duration={knownSumMs:m(40),recordedCount:m(1),calculatedCount:m(0),missingCount:m(1)};
  r.repeatedRead.count=m(0);r.sameRequestObservationCount=m(5);r.repeatedReadRequestCount=m(3);
  r.combinedUnionMs={value:null,status:'unavailable',basis:'missing_time',evidenceRefs:[]};
  r.coverage.reasonCodes=['missing_start','missing_durations','missing_window'];
  const text=renderTimingResult(fixture);
  assert.match(text,language==='en'?/Calls after failure: 2/:/失败后再次调用: 2/);
  assert.match(text,language==='en'?/Known duration subtotal: 40 ms/:/可计算耗时小计: 40 ms/);
  assert.match(text,language==='en'?/Calls without usable duration: 1/:/缺少可计算耗时的调用: 1/);
  assert.match(text,language==='en'?/Additional identical request observations: 5/:/相同请求再次出现次数: 5/);
  assert.match(text,language==='en'?/not wasted time/:/不表示浪费/);
  assert.match(text,language==='en'?/counts and duration subtotals are retained/:/仍保留次数与耗时小计/);
  assert.doesNotMatch(text,/execution\.repeats\.|timing\.basis\.repeat|undefined|NaN/);
 }}finally{locale.setLocale(previous);}
});

test('local text shows paired evidence aliases and detail limits without changing sharing',()=>{
 const response=structuredClone(local),navigation=response.evidence.repeatPages;
 const count=(value:number)=>({value,status:'derived' as const,basis:'exact_event_page' as const,evidenceRefs:[]});
 navigation.detail={support:'supported',reason:'exact_event_page'};navigation.locatedOperationCount=count(1);
 navigation.entries=[{later:{operationAlias:'repeat:0:later',pages:[{limit:200,evidenceRefs:['event:later']}]},afterFailure:{operationAlias:'repeat:0:failure',pages:[{limit:200,evidenceRefs:['event:failure']}]},successfulReads:[{operationAlias:'repeat:0:read:0',pages:[{limit:200,evidenceRefs:['event:read']}]}],repeatedReadTargetCount:1,laterDurationMs:count(40),recoverySpanMs:count(80)}];
 const previous=locale.getLocale();
 try{for(const language of ['en','zh'] as const){locale.setLocale(language);const text=renderTimingResult(response);assert.match(text,/repeat:0:later/);assert.match(text,/repeat:0:failure/);assert.match(text,language==='en'?/Earlier successful read 1/:/前次成功读取 1/);assert.doesNotMatch(text,/execution\.repeats\.|undefined|NaN|未知/);const limited=structuredClone(response);limited.evidence.repeatPages.entries=[];limited.evidence.repeatPages.detail={support:'unavailable',reason:'resource_limit'};assert.match(renderTimingResult(limited),language==='en'?/aggregate counts and duration subtotals remain/:/完整次数与耗时小计仍保留/);assert.doesNotMatch(renderTimingResult(share),/repeat:0|event:later|event:failure/);}}finally{locale.setLocale(previous);}
});


test('CLI outcome text uses the shared subset, exclusions and zero-denominator explanation',async()=>{
 const {outcomeStatistics}=await import('../../tests/fixtures/outcomes.js');
 const previous=locale.getLocale();
 try{for(const language of ['en','zh'] as const){
  locale.setLocale(language);const fixture=structuredClone(local);fixture.work.outcomes=outcomeStatistics();
  const text=renderTimingResult(fixture);assert.match(text,/33\.3/);assert.match(text,language==='en'?/Failed 1 of 3/:/3 次操作中，失败 1 次/);
  assert.match(text,language==='en'?/Interrupted or cancelled: 1/:/中断或取消：1 次/);assert.match(text,language==='en'?/Without a determinate result: 1/:/未记录可判定结果：1 次/);
  assert.doesNotMatch(text,/execution\.outcomes\.|NaN|undefined/);
  for(const field of ['determinateOperations','failed','succeeded'] as const)fixture.work.outcomes[field].value=0;
  fixture.work.outcomes.failureRatio={value:null,status:'unavailable',basis:'no_candidates',evidenceRefs:[]};
  const empty=renderTimingResult(fixture);assert.match(empty,language==='en'?/No operations with determinate/:/不计算比例/);assert.doesNotMatch(empty,/0%|NaN|undefined/);
 }}finally{locale.setLocale(previous);}
});

test('input change text retains observed comparison without timing and translates exclusions',async()=>{
 const {inputChangeFixture}=await import('../../tests/fixtures/input-change.js');
 const previous=locale.getLocale();
 try{for(const language of ['zh','en'] as const){
  locale.setLocale(language);const fixture=structuredClone(local);fixture.context.inputChange=inputChangeFixture();
  fixture.context.inputChange.statistics!.candidates=3;fixture.context.inputChange.statistics!.missingInput=1;fixture.context.inputChange.statistics!.partial=true;
  const text=renderTimingResult(fixture);
  assert.match(text,/19900/);assert.match(text,language==='zh'?/不代表执行顺序/:/does not establish execution order/);
  assert.match(text,language==='zh'?/1 条未记录输入量/:/1 missing input counts/);
  assert.doesNotMatch(text,/inputChange\.|NaN|undefined/);
 }}finally{locale.setLocale(previous);}
});
