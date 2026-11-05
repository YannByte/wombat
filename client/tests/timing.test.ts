import { test } from 'node:test';
import assert from 'node:assert/strict';
import { CoreError, createUsageClient, type TimingRequest, type TimingLocalResult, type TimingShareResult } from '@wombat/client';
import { withAutomaticPrices } from '../src/node/auto-prices.js';
import { validate as validateLocal } from '../src/generated/validate-timing-local-response.js';
import { validate as validateShare } from '../src/generated/validate-timing-share-response.js';
import { validate as validateRequest } from '../src/generated/validate-timing-request.js';
import { readFile } from 'node:fs/promises';

const metric = { value: null, status: 'unavailable', basis: 'not_recorded', evidenceRefs: [] } as const;
const unavailable = { support: 'unavailable', reason: 'not_recorded' } as const;
const capabilities = {
  wallClock: unavailable, nativeTtft: unavailable, firstContentRecordDelay: unavailable,
  lifecycleIntervals: unavailable, operationIntervals: unavailable, contextPressure: unavailable, strictResponseGap: unavailable,
  exploratoryGap: unavailable, commandLabels: unavailable, fileChanges: unavailable, messageRecords: unavailable, objectUses: unavailable,
};
const capabilityResult = { outputVersion: 8, action: 'capabilities', methodVersion: 'safe_event_turn_v7', profile: 'local', capabilities } as const;
const scope = { sourceInstanceId: 'source', threadId: 'thread', turnId: 'turn', agentKind: 'codex', wholeTurn: true };
const count = () => ({ ...metric, evidenceRefs: [] });
const repeatDuration=()=>({knownSumMs:count(),recordedCount:count(),calculatedCount:count(),missingCount:count()});
const repeatedBehavior=()=>({failureMethod:'same_operation_after_failure_v1' as const,readMethod:'same_target_read_v1' as const,endpointMethodVersion:1,support:unavailable,readLayer:'same_path_range_unconfirmed' as const,afterFailure:{count:count(),duration:repeatDuration()},repeatedRead:{count:count(),duration:repeatDuration()},sameRequestObservationCount:count(),repeatedReadRequestCount:count(),recoverySpanSumMs:count(),missingRecoverySpanCount:count(),combinedOperationCount:count(),combinedUnionMs:count(),combinedMissingIntervalCount:count(),coverage:{candidateOperations:count(),eligibleCommands:count(),missingIdentityRecords:count(),excludedReceivers:count(),missingMatching:count(),conflictingOperations:count(),missingStart:count(),indeterminateOutcomes:count(),orderGaps:count(),contextBoundaries:count(),crossedContext:count(),missingClockDomain:count(),sourceMetadataGaps:count(),durationConflicts:count(),partial:true,reasonCodes:[]}});
const category = () => ({ candidates: count(), closed: count(), unionMs: count(), sumMs: count() });
const distribution = () => ({ samples: count(), median: count(), p90: count() });
const useTotals = { methodVersion: 4 as const, sourceCoverage: 'unknown' as const, objectCount: count(), recordCount: count(), unboundTargetRecords: count(),
  unassignedSkillRecords: count(), unassignedMcpRecords: count(), coverage: { dispatchGaps: count(), identityGaps: count(), targetGaps: count(), timeGaps: count(), associatedTurnGaps: count() } };
const local: TimingLocalResult = {
  outputVersion: 8, action: 'summary', methodVersion: 'safe_event_turn_v7', profile: 'local',
  uses: { totals: useTotals, detail: unavailable, limit: 50, objects: [], nextCursor: null },
  privacy: { profile: 'local', omittedFields: [], aliases: 'none' },
  basis: { snapshotId: 'live:scope:fixed', snapshotSchema: 4, createdAt: '2026-10-05T00:00:00Z', adapterVersions: [], projectionVersion: 1 },
  scope, capabilities, anchors: { startMs: count(), endMs: count() },
  time: {
    timeline: { presentation: 'list', detail: unavailable, entryCount: count(), trackCount: count(), identifiedIntervalCount: count(), unclassifiedGapCount: count(), unlocatedIntervalCount: count(), outsideWindowIntervalCount: count(), detailLimit: 200, tracks: [], unclassifiedGaps: [] },
    state: 'unknown', nativeWallClockMs: count(), derivedWallClockMs: count(), nativeTtftMs: count(), firstContentRecordDelayMs: count(),
    boundaryDiscrepancyMs: count(), observedWindowMs: count(), command: category(), compaction: category(), reasoning: category(), mcp: category(),
    intersectionMasksMs: [count(), count(), count(), count(), count(), count(), count(), count(), count(), count(), count(), count(), count(), count(), count(), count()], coveredMs: count(), unclassifiedMs: count(), coverageRatio: count(), waitingProxyMs: count(),
    strictResponseGapMs: count(), exploratoryGapMs: count(),
    repeatedBehavior:repeatedBehavior(),
    operationCoverage: {methodVersion:1,endpointMethodVersion:1,candidateOperations:count(),pairedOperations:count(),identityGapRecords:count(),conflictingOperations:count(),coveredMs:count(),residualMs:count(),residualRangeCount:count(),partial:true,reasonCodes:['missing_window'],detail:unavailable,detailLimit:200,residualRanges:[]},
  },
  context: {
    inputChange:{method:'request_input_observation_change_v1',availability:unavailable,statistics:null},
    activeContextOccupancy: count(), compactionRecords: count(), compactionTimeMs: count(), method: 'synthetic', quantileMethod: 'type7',
    candidates: count(), conflictingMeasurements: count(), conflictingWindowRecords: count(), input: distribution(), ratio: distribution(),
    segmentCount: count(), segments: [], compactionNeighbors: [], detail: unavailable,
  },
  work: {
    outcomes: {method:'terminal_success_failure_subset_v1',determinateOperations:count(),succeeded:count(),failed:count(),interrupted:count(),rejected:count(),nonterminal:count(),indeterminate:count(),conflicting:count(),identityGapRecords:count(),unclassified:count(),failureRatio:count(),partial:true},
    operationCandidates: count(), closedOperations: count(), failedOperations: count(), labelledCommandMs: count(), fileChangeRecords: count(),
    changedFiles: count(), addedLines: count(), removedLines: count(), messageRecordCandidates: count(), nonemptyVisibleContentRecords: count(),
    unknownContentRecords: count(), missingContentTimeRecords: count(), userBoundaryRecords: count(), injectedContextRecords: count(),
    reasoningMessageRecords: count(), compactionRecords: count(), repositoryBaseline: unavailable,
  },
  findings: [], coverage: {
    facts: count(), bytes: count(), metadata: count(), eventBlocks: count(), scopedEvents: count(), scopedMeasurements: count(),
    boundaryCandidates: count(), lifecycleCandidates: [count(), count(), count(), count()], linkedLifecycles: [count(), count(), count(), count()], conflictingLifecycles: count(), missingIdentityLifecycles: count(),
    contentCandidates: count(), domainCount: count(), missingWatermarks: count(), generationMismatches: count(), incompleteDomains: count(),
    snapshotUnassignedTotal: count(), threadUnassignedTotal: count(), sourceStatus: 'unknown',
  },
  quality: { partial: true, running: false, censored: true, reasonCodes: [], factLimit: 100000, summaryLimitBytes: 262144 },
  freshness: { status: 'fixed' }, evidence: { repeatPages: {detail:unavailable,candidateOperationCount:count(),locatedOperationCount:count(),pageCount:count(),limitBytes:65536,entries:[]}, intervalPages: { detail: unavailable, candidateIntervalCount: count(), locatedIntervalCount: count(), missingEventRefCount: count(), pageCount: count(), limitBytes: 65536, entries: [] }, collections: [], available: false, limit: 50, snapshotId: 'live:scope:fixed', refs: [], method: 'synthetic' },
};
const share: TimingShareResult = {
  outputVersion: 8, action: 'summary', methodVersion: local.methodVersion, profile: 'share-v1',
  uses: useTotals,
  privacy: { profile: 'share-v1', omittedFields: ['local_ids'], aliases: 'package' },
  scope: { taskAlias: 'task-1', turnAlias: 'turn-1', wholeTurn: true }, capabilities, relativeAnchors: local.anchors,
  time: local.time, context: local.context, work: local.work, findings: [], coverage: local.coverage, quality: local.quality,
  freshness: { status: 'fixed' }, basisCollections: [],
};
const summary: TimingRequest = { action: 'summary', threadId: 'thread', turnId: 'turn', scope: { sourceInstanceId: 'source' } };
function client(result: unknown) { return createUsageClient({ query: async () => null, timing: async () => result }); }

test('standalone ESM validators retain Unicode lengths and the tighter Rust page bound', async () => {
  const request = { action: 'evidence', threadId: 'thread', turnId: 'turn', limit: 200, cursor: { token: '😀'.repeat(8192) } };
  assert.equal(validateRequest(request), true);
  assert.equal(validateRequest({ ...request, cursor: { token: '😀'.repeat(8193) } }), false);
  assert.equal(validateRequest({ ...request, cursor: { token: '\ud800'.repeat(8192) } }), true);
  assert.equal(validateRequest({ ...request, limit: 201 }), false);
  assert.equal(validateRequest({ ...request, limit: 0 }), false);
  for (const name of ['request', 'response', 'local-response', 'share-response']) {
    const source = await readFile(new URL(`../src/generated/validate-timing-${name}.js`, import.meta.url), 'utf8');
    assert.doesNotMatch(source, /\brequire\s*\(/);
  }
});

test('numeric timing schemas retain null zero and fractions while rejecting unsafe integer measures', async () => {
  for (const value of [null, 0, Number.MAX_SAFE_INTEGER]) {
    const response = { ...local, time: { ...local.time, nativeWallClockMs: { ...count(), value } } };
    assert.equal(validateLocal(response), true);
    assert.equal(await client(response).timing!(summary), response);
  }
  assert.equal(validateLocal({ ...local, anchors: { ...local.anchors, startMs: { ...count(), value: -Number.MAX_SAFE_INTEGER } } }), true);
  assert.equal(validateLocal({ ...local, time: { ...local.time, coverageRatio: { ...count(), value: 0.5 } } }), true);
  for (const value of [-1, 0.5, Number.MAX_SAFE_INTEGER + 1, NaN, Infinity]) {
    const response = { ...local, time: { ...local.time, nativeWallClockMs: { ...count(), value } } };
    assert.equal(validateLocal(response), false);
    await assert.rejects(client(response).timing!(summary), { code: 'PROTOCOL_ERROR' });
  }
  assert.equal(validateLocal({ ...local, anchors: { ...local.anchors, startMs: { ...count(), value: -Number.MAX_SAFE_INTEGER - 1 } } }), false);
  assert.equal(validateShare({ ...share, work: { ...share.work, changedFiles: { ...count(), value: Number.MAX_SAFE_INTEGER + 1 } } }), false);
});

test('timing validates its narrow request before calling the independent host', async () => {
  let calls = 0;
  const reader = createUsageClient({ query: async () => null, timing: async () => { calls++; return capabilityResult; } });
  assert.equal(createUsageClient({ query: async () => null }).timing, undefined);
  for (const request of [
    { action: 'execute' }, { action: 'capabilities', nativeHooks: {} },
    { action: 'summary', threadId: 'thread', turnId: 'turn', verify: true },
    { action: 'evidence', threadId: 'thread', turnId: 'turn', limit: 201 },
  ]) await assert.rejects(reader.timing!(request as TimingRequest), { code: 'INVALID_ARGUMENT' });
  for (const [selection,path] of [
    [{privacyProfile:'share-v1'},'/privacyProfile'],
    [{objectRef:'use:'+'a'.repeat(64)},'/collection'],
    [{objectRef:'invalid',collection:'use_records'},'/objectRef'],
  ] as const) {
    await assert.rejects(reader.timing!({action:'evidence',threadId:'thread',turnId:'turn',...selection}),error=>{
      assert.ok(error instanceof CoreError);assert.equal(error.code,'INVALID_ARGUMENT');
      assert.equal((error.details as {fields:{path:string}[]}).fields[0].path,path);return true;
    });
  }
  assert.equal(calls, 0);
  assert.equal(await reader.timing!({ action: 'capabilities' }), capabilityResult);
});

test('timing responses bind action profile method target and selected snapshot', async () => {
  assert.equal(await client(local).timing!(summary), local);
  const evidence = { outputVersion: 8, action: 'evidence', collection: 'turn_events', methodVersion: local.methodVersion, profile: 'local', snapshotId: local.basis.snapshotId, scope, total: count(), rows: [] };
  assert.equal(await client(evidence).timing!({ ...summary, action: 'evidence'  }), evidence);
  for (const invalid of [
    { ...local, outputVersion: 1 }, { ...local, outputVersion: 7 }, { ...local, action: 'evidence' }, { ...local, methodVersion: 'future' }, { ...local, methodVersion: 'safe_event_turn_v1' },
    { ...local, profile: 'share-v1' },
    ...[1, 2, 3].map(methodVersion => ({ ...local, uses: { ...local.uses, totals: { ...local.uses.totals, methodVersion } } })), { ...local, basis: { ...local.basis, snapshotId: '' } },
    ...['threadId', 'turnId', 'sourceInstanceId', 'agentKind'].map(field => ({ ...local, scope: { ...scope, [field]: 'other' } })),
    { ...local, scope: { ...scope, wholeTurn: false } },
  ]) await assert.rejects(client(invalid).timing!(summary), { code: 'PROTOCOL_ERROR' });
  await assert.rejects(client({ ...evidence, snapshotId: '' }).timing!({ action: 'evidence', threadId: 'thread', turnId: 'turn'  }), { code: 'PROTOCOL_ERROR' });
});

test('timing rejects unsupported analysis methods for every response action and profile', async () => {
  const evidence = { outputVersion: 8, action: 'evidence', collection: 'turn_events', methodVersion: local.methodVersion, profile: 'local', snapshotId: local.basis.snapshotId, scope, total: count(), rows: [] };
  for (const methodVersion of ['safe_event_turn_v1', 'safe_event_turn_v2', 'safe_event_turn_v3','safe_event_turn_v4']) {
    await assert.rejects(client({ ...capabilityResult, methodVersion }).timing!({ action: 'capabilities' }), { code: 'PROTOCOL_ERROR' });
    await assert.rejects(client({ ...local, methodVersion }).timing!(summary), { code: 'PROTOCOL_ERROR' });
    await assert.rejects(client({ ...share, methodVersion }).timing!({ ...summary, privacyProfile: 'share-v1' }), { code: 'PROTOCOL_ERROR' });
    await assert.rejects(client({ ...evidence, methodVersion }).timing!({ ...summary, action: 'evidence'  }), { code: 'PROTOCOL_ERROR' });
  }
});

test('timing conflict reasons coexist with retained duration in local and share contracts', async () => {
  const time = { ...local.time, nativeWallClockMs: { ...count(), value: 120, status: 'observed', basis: 'native_record' } };
  const quality = { ...local.quality, reasonCodes: ['target_conflict', 'outcome_conflict'] };
  const localized = { ...local, time, quality };
  const shared = { ...share, time, quality };
  assert.equal(validateLocal(localized), true);
  assert.equal(validateShare(shared), true);
  assert.equal(await client(localized).timing!(summary), localized);
  assert.equal(await client(shared).timing!({ ...summary, privacyProfile: 'share-v1' }), shared);
  assert.equal(localized.time.nativeWallClockMs.value, 120);
  assert.equal(shared.time.nativeWallClockMs.value, 120);
});

test('local and share validators have their correct roots and reject leaked locating fields', async () => {
  assert.equal(validateLocal(local), true); assert.equal(validateShare(share), true);
  assert.equal(validateLocal(share), false); assert.equal(validateShare(local), false);
  assert.equal(await client(share).timing!({ ...summary, privacyProfile: 'share-v1' }), share);
  for (const invalid of [{ ...share, readView: local.readView }, { ...share, scope }, { ...share, privacy: local.privacy }])
    await assert.rejects(client(invalid).timing!({ ...summary, privacyProfile: 'share-v1' }), { code: 'PROTOCOL_ERROR' });
});

test('timing cancellation discards late results and never invokes prices or other products', async () => {
  const controller = new AbortController(); let calls = 0;
  const forbidden = async () => { throw new Error('unrelated transport'); };
  const reader = withAutomaticPrices(createUsageClient({ query: forbidden, prices: forbidden, config: forbidden, account: forbidden,
    live: forbidden, timing: async () => { calls++; controller.abort(); return local; } }));
  await assert.rejects(reader.timing!(summary, { signal: controller.signal }), (error: unknown) => error instanceof CoreError && error.code === 'CANCELLED');
  await assert.rejects(reader.timing!(summary, { signal: controller.signal }), { code: 'CANCELLED' });
  assert.equal(calls, 1);
});


test('nonempty local fragment navigation enforces bounds and stays outside sharing contracts', async () => {
  const refs = ['event:synthetic-endpoint'];
  const page = { cursor: null, limit: 200, evidenceRefs: refs };
  const entry = { intervalAlias: 'interval:1', pages: [page] };
  const value = (n: number) => ({ ...count(), value: n, status: 'derived', basis: 'exact_event_page' });
  const navigation = {
    detail: { support: 'supported', reason: 'exact_event_page' }, candidateIntervalCount: value(1),
    locatedIntervalCount: value(1), missingEventRefCount: value(0), pageCount: value(1), limitBytes: 65536, entries: [entry],
  };
  const response = (nav: unknown) => ({
    ...local,
    time: { ...local.time, timeline: {
      ...local.time.timeline, presentation: 'timeline', detail: { support: 'supported', reason: 'lifecycle_union' },
      entryCount: value(1), trackCount: value(1), identifiedIntervalCount: value(1), unclassifiedGapCount: value(0),
      unlocatedIntervalCount: value(0), outsideWindowIntervalCount: value(0),
      tracks: [{ intervalAlias: entry.intervalAlias, category: 'command', startMs: 0, endMs: 20, clipped: false, evidenceScope: 'event_records', evidenceRefs: refs }],
    } },
    evidence: { ...local.evidence, intervalPages: nav },
  });
  for (const cursor of [null, { token: 'opaque-token-for-mock' }]) {
    const valid = response({ ...navigation, entries: [{ ...entry, pages: [{ ...page, cursor }] }] });
    assert.equal(validateLocal(valid), true);
    assert.equal(await client(valid).timing!(summary), valid);
  }
  for (const invalid of [
    { ...navigation, entries: [{ ...entry, pages: Array.from({ length: 4 }, () => page) }] },
    { ...navigation, entries: [{ ...entry, pages: [{ ...page, evidenceRefs: Array.from({ length: 4 }, (_, i) => `event:${i}`) }] }] },
    { ...navigation, entries: Array.from({ length: 201 }, () => entry) },
    ...[0, 199, 201].map(limit => ({ ...navigation, entries: [{ ...entry, pages: [{ ...page, limit }] }] })),
  ]) {
    const rejected = response(invalid);
    assert.equal(validateLocal(rejected), false);
    await assert.rejects(client(rejected).timing!(summary), { code: 'PROTOCOL_ERROR' });
  }
  for (const rejected of [
    { ...share, intervalPages: navigation },
    { ...share, evidence: { repeatPages: {detail:unavailable,candidateOperationCount:count(),locatedOperationCount:count(),pageCount:count(),limitBytes:65536,entries:[]}, intervalPages: navigation } },
    { ...share, time: { ...share.time, timeline: { ...share.time.timeline, intervalPages: navigation } } },
  ]) {
    assert.equal(validateShare(rejected), false);
    await assert.rejects(client(rejected).timing!({ ...summary, privacyProfile: 'share-v1' }), { code: 'PROTOCOL_ERROR' });
  }
});
const observed = (value: number, basis: TimingLocalResult['uses']['totals']['objectCount']['basis'] = 'canonical_use_records') => ({ value, status: 'observed' as const, basis, evidenceRefs: [] });
const objectRef = `use:${'a'.repeat(64)}`;
const knownUseTotals = { methodVersion: 4 as const, sourceCoverage: 'complete' as const, objectCount: observed(1), recordCount: observed(3), unboundTargetRecords: observed(0),
  unassignedSkillRecords: observed(0, 'unassigned_use_index'), unassignedMcpRecords: observed(0, 'unassigned_use_index'),
  coverage: { dispatchGaps: observed(0), identityGaps: observed(0), targetGaps: observed(0), timeGaps: observed(0), associatedTurnGaps: observed(0) } };
const useObject = { objectRef, kind: 'skill' as const, state: 'used' as const, path: '/synthetic/skill/SKILL.md', server: null, project: null,
  associatedUseCount: observed(2, 'canonical_use_identity'), useCount: observed(2, 'canonical_use_identity'), recordCount: observed(3),
  unassignedTurnRecords: observed(0, 'unassigned_use_index'), coverage: knownUseTotals.coverage };
const useRecord = { reference: `use:${'b'.repeat(64)}`, objectRef, kind: 'skill_read' as const, state: 'used' as const, outcome: 'failed' as const,
  timestampMs: 0, timeBasis: 'source_operation_time' as const, nativeDurationMs: 0, tool: null, exitCode: 1, identityKnown: true, replayOf: null, targetConflict: false, gapCodes: [] };
const usePage = { outputVersion: 8, action: 'evidence' as const, methodVersion: local.methodVersion, profile: 'local' as const,
  snapshotId: local.basis.snapshotId, scope, totals: knownUseTotals, total: observed(1), nextCursor: null };
test('nonempty canonical object and record pages validate and bind collection object and use method', async () => {
  const objects = { ...usePage, collection: 'use_objects' as const, rows: [useObject] };
  const records = { ...usePage, collection: 'use_records' as const, objectRef, total: observed(3), rows: [useRecord] };
  const objectRequest: TimingRequest = { action: 'evidence', collection: 'use_objects', threadId: 'thread', turnId: 'turn'  };
  const recordRequest: TimingRequest = { ...objectRequest, collection: 'use_records', objectRef };
  assert.equal(await client(objects).timing!(objectRequest), objects);
  assert.equal(await client(records).timing!(recordRequest), records);
  for(const methodVersion of [1,2,3]) await assert.rejects(client({...objects,totals:{...knownUseTotals,methodVersion}}).timing!(objectRequest),{code:'PROTOCOL_ERROR'});
  const declined = { ...records, rows: [{ ...useRecord, outcome: 'declined', exitCode: null }] };
  assert.equal(await client(declined).timing!(recordRequest), declined);
  const conflicting = { ...records, rows: [{ ...useRecord, exitCode: null, gapCodes: ['operation_result_conflict'] }] };
  assert.equal(await client(conflicting).timing!(recordRequest), conflicting);
  const allRecordsRequest: TimingRequest = { ...objectRequest, collection: 'use_records' };
  const { objectRef: _filteredObject, ...unfilteredRecords } = records;
  assert.equal(await client(unfilteredRecords).timing!(allRecordsRequest), unfilteredRecords);
  const nullObjectRecords = { ...unfilteredRecords, objectRef: null };
  assert.equal(await client(nullObjectRecords).timing!(allRecordsRequest), nullObjectRecords);
  await assert.rejects(client(records).timing!(objectRequest), { code: 'PROTOCOL_ERROR' });
  await assert.rejects(client(objects).timing!(recordRequest), { code: 'PROTOCOL_ERROR' });
  for (const invalid of [
    { ...records, objectRef: `use:${'c'.repeat(64)}` },
    { ...records, rows: [{ ...useRecord, objectRef: `use:${'c'.repeat(64)}` }] },
    ...[1, 2, 3, 99].map(methodVersion => ({ ...records, totals: { ...knownUseTotals, methodVersion } })),
    { ...records, rows: Array.from({ length: 201 }, () => useRecord) },
    { ...records, rows: [{ ...useRecord, nativeDurationMs: Number.MAX_SAFE_INTEGER + 1 }] },
    { ...records, rows: [{ ...useRecord, outcome: 'invented_success' }] },
  ]) await assert.rejects(client(invalid).timing!(recordRequest), { code: 'PROTOCOL_ERROR' });
  await assert.rejects(client({ ...objects, rows: Array.from({ length: 201 }, () => useObject) }).timing!(objectRequest), { code: 'PROTOCOL_ERROR' });
  let calls = 0;
  const reader = createUsageClient({ query: async () => null, timing: async () => { calls++; return objects; } });
  for (const invalid of [{ ...objectRequest, objectRef }, { ...recordRequest, objectRef: '/synthetic/skill/SKILL.md' }, { ...recordRequest, privacyProfile: 'share-v1' }])
    await assert.rejects(reader.timing!(invalid as TimingRequest), { code: 'INVALID_ARGUMENT' });
  assert.equal(calls, 0);
});
test('sharing allows numeric use coverage and rejects local object record and cursor fields', async () => {
  const numeric = { ...share, uses: knownUseTotals };
  assert.equal(validateShare(numeric), true);
  assert.equal(await client(numeric).timing!({ ...summary, privacyProfile: 'share-v1' }), numeric);
  for(const methodVersion of [1,2,3]) await assert.rejects(client({...numeric,uses:{...knownUseTotals,methodVersion}}).timing!({...summary,privacyProfile:'share-v1'}),{code:'PROTOCOL_ERROR'});
  for (const fields of [{ objects: [useObject] }, { path: '/synthetic/skill/SKILL.md' }, { server: 'private-service' },
    { nextCursor: { token: 'local-cursor' } }, { rows: [useRecord] }]) {
    const invalid = { ...numeric, uses: { ...knownUseTotals, ...fields } };
    assert.equal(validateShare(invalid), false);
    await assert.rejects(client(invalid).timing!({ ...summary, privacyProfile: 'share-v1' }), { code: 'PROTOCOL_ERROR' });
  }
});

test('work metrics preserve canonical identities, reported paths and unknown user origin in both whitelists', () => {
  const work = {
    ...local.work,
    operationCandidates: { value: 3, status: 'observed', basis: 'canonical_operation_identity', evidenceRefs: [] },
    changedFiles: { value: 2, status: 'derived', basis: 'reported_file_paths', evidenceRefs: [] },
    userBoundaryRecords: { value: null, status: 'unavailable', basis: 'unknown_message_origin', evidenceRefs: [] },
  };
  const quality = { ...local.quality, reasonCodes: ['unknown_message_origin', 'source_partial'] };
  assert.equal(validateLocal({ ...local, work, quality }), true);
  assert.equal(validateShare({ ...share, work, quality }), true);
  for (const value of [-1, 9007199254740992]) {
    assert.equal(validateLocal({ ...local, work: { ...work, changedFiles: { ...work.changedFiles, value } } }), false);
  }
  assert.equal(validateShare({ ...share, work: { ...work, changedFiles: { ...work.changedFiles, paths: ['/private/path'] } } }), false);
  assert.equal(validateLocal({ ...local, work: { ...work, userBoundaryRecords: { ...work.userBoundaryRecords, basis: 'guessed_user' } } }), false);
});

test('method v7 accepts MCP relative tracks and all sixteen core masks; stale masks and share identities fail', async () => {
  const mcp = structuredClone(local);
  mcp.time.timeline.tracks = [{intervalAlias:'interval-1',category:'mcp',startMs:2000,endMs:7000,clipped:false,evidenceScope:'turn_collection',evidenceRefs:[]}];
  mcp.time.mcp.unionMs = observed(5000, 'lifecycle_union');
  mcp.time.mcp.sumMs = observed(7000, 'lifecycle_sum');
  mcp.time.intersectionMasksMs = [observed(5000, 'interval_mask'), observed(0, 'interval_mask'), observed(0, 'interval_mask'), observed(0, 'interval_mask'), observed(0, 'interval_mask'), observed(0, 'interval_mask'), observed(0, 'interval_mask'), observed(0, 'interval_mask'), observed(5000, 'interval_mask'), observed(0, 'interval_mask'), observed(0, 'interval_mask'), observed(0, 'interval_mask'), observed(0, 'interval_mask'), observed(0, 'interval_mask'), observed(0, 'interval_mask'), observed(0, 'interval_mask')];
  assert.equal(validateLocal(mcp), true);
  assert.equal(await client(mcp).timing!(summary), mcp);
  const shared = {...share,time:mcp.time};
  assert.equal(validateShare(shared), true);
  assert.equal(await client(shared).timing!({...summary,privacyProfile:'share-v1'}), shared);
  assert.doesNotMatch(JSON.stringify(shared), /server|tool|threadId|snapshotId/);
  const {mcp: omittedMcp, ...oldTime} = mcp.time;
  assert.equal(omittedMcp.unionMs.value, 5000);
  assert.equal(validateLocal({...mcp,time:oldTime}), false);
  assert.equal(validateShare({...shared,time:oldTime}), false);
  await assert.rejects(client({...shared,time:oldTime}).timing!({...summary,privacyProfile:'share-v1'}), {code:'PROTOCOL_ERROR'});
  for (const length of [0, 8, 15, 17]) await assert.rejects(client({...mcp,time:{...mcp.time,intersectionMasksMs:Array.from({length}, count)}}).timing!(summary), {code:'PROTOCOL_ERROR'});
  for (const field of ['server','tool','nativeId']) {
    const leaked = {...shared,time:{...shared.time,timeline:{...shared.time.timeline,tracks:[{...shared.time.timeline.tracks[0],[field]:'synthetic-private'}]}}};
    assert.equal(validateShare(leaked), false);
    await assert.rejects(client(leaked).timing!({...summary,privacyProfile:'share-v1'}), {code:'PROTOCOL_ERROR'});
  }
});

test('operation coverage validates independent totals, pairing counts, relative ranges and versions', async () => {
  const fixture=structuredClone(local), c=fixture.time.operationCoverage;
  const measure=(value:number,basis:typeof c.residualMs.basis='operation_residual')=>({value,status:'derived' as const,basis,evidenceRefs:[]});
  fixture.time.observedWindowMs=measure(30,'explicit_boundary');
  c.candidateOperations=measure(2,'canonical_operation_identity');c.pairedOperations=measure(1,'explicit_boundary');c.conflictingOperations=measure(1,'canonical_operation_identity');
  c.coveredMs=measure(10,'operation_union');c.residualMs=measure(20);c.residualRangeCount=measure(2);
  c.detail={support:'supported',reason:'operation_residual'};c.residualRanges=[{startMs:0,endMs:2},{startMs:12,endMs:30}];
  assert.equal(await client(fixture).timing!(summary),fixture);
  const bad=[
    {...c,methodVersion:2},{...c,endpointMethodVersion:2},
    {...c,pairedOperations:measure(3)},{...c,conflictingOperations:measure(2)},
    {...c,coveredMs:measure(11,'operation_union')},
    {...c,residualRangeCount:measure(1)},
    {...c,residualRanges:[{startMs:0,endMs:0},{startMs:10,endMs:30}]},
    {...c,residualRanges:[{startMs:0,endMs:12},{startMs:10,endMs:18}]},
    {...c,residualRanges:[{startMs:12,endMs:30},{startMs:0,endMs:2}]},
    {...c,residualRanges:[{startMs:0,endMs:2},{startMs:12,endMs:31}]},
    {...c,detail:{support:'unavailable',reason:'resource_limit'}},
  ];
  for(const coverage of bad)await assert.rejects(client({...fixture,time:{...fixture.time,operationCoverage:coverage}}).timing!(summary),{code:'PROTOCOL_ERROR'});
  const limited={...c,detail:{support:'unavailable' as const,reason:'resource_limit' as const},residualRanges:[],residualRangeCount:measure(250)};
  assert.equal((await client({...fixture,time:{...fixture.time,operationCoverage:limited}}).timing!(summary)).action,'summary');
  for(const response of [capabilityResult,fixture,share,{...usePage,collection:'use_objects' as const,rows:[]}]) {
    const request=response.action==='capabilities'?{action:'capabilities' as const}:response.action==='evidence'?{...summary,action:'evidence' as const,collection:'use_objects' as const}:{...summary,privacyProfile:response.profile};
    assert.equal((await client(response).timing!(request)).action,response.action);
    for(const outputVersion of [1,2,3,4,5,6])await assert.rejects(client({...response,outputVersion}).timing!(request),{code:'PROTOCOL_ERROR'});
  }
});

test('repeat aggregate methods, safe measures and cross-field bounds reject inconsistent results',async()=>{
 const fixture=structuredClone(local),r=fixture.time.repeatedBehavior;
 const m=(value:number)=>({value,status:'derived' as const,basis:'repeat_after_failure' as const,evidenceRefs:[]});
 r.support={support:'partial',reason:'repeat_after_failure'};
 r.afterFailure={count:m(2),duration:{knownSumMs:m(40),recordedCount:m(1),calculatedCount:m(0),missingCount:m(1)}};
 r.repeatedRead={count:m(1),duration:{knownSumMs:m(40),recordedCount:m(1),calculatedCount:m(0),missingCount:m(0)}};
 r.coverage.candidateOperations=m(3);r.coverage.eligibleCommands=m(3);r.combinedOperationCount=m(2);
 r.combinedMissingIntervalCount=m(1);r.missingRecoverySpanCount=m(1);r.combinedUnionMs=m(30);fixture.time.observedWindowMs=m(100);
 assert.equal((await client(fixture).timing!(summary)).action,'summary');
 const bad=[{...r,failureMethod:'future'}, {...r,readMethod:'future'}, {...r,endpointMethodVersion:2},
   {...r,afterFailure:{...r.afterFailure,duration:{...r.afterFailure.duration,missingCount:m(0)}}},
   {...r,afterFailure:{...r.afterFailure,duration:{...r.afterFailure.duration,recordedCount:m(0),missingCount:m(2)}}},
   {...r,combinedOperationCount:m(0)}, {...r,combinedOperationCount:m(4)}, {...r,combinedMissingIntervalCount:m(3)},
   {...r,combinedUnionMs:m(101)}, {...r,missingRecoverySpanCount:m(3)},
   {...r,coverage:{...r.coverage,eligibleCommands:m(4)}},
   {...r,recoverySpanSumMs:m(Number.MAX_SAFE_INTEGER+1)}, {...r,requestFingerprint:'private'},
 ];
 for(const repeatedBehavior of bad)await assert.rejects(client({...fixture,time:{...fixture.time,repeatedBehavior}}).timing!(summary),{code:'PROTOCOL_ERROR'});
 for(const outputVersion of [1,2,3,4,5,6])await assert.rejects(client({...fixture,outputVersion}).timing!(summary),{code:'PROTOCOL_ERROR'});
 const noTime=structuredClone(fixture);noTime.time.repeatedBehavior.combinedUnionMs={...count(),basis:'missing_time'};
 noTime.time.observedWindowMs={...count(),basis:'missing_time'};
 assert.equal((await client(noTime).timing!(summary)).action,'summary');
});

test('local repeated-call proof pages bind complete rows and reject partial or leaked navigation',async()=>{
 const fixture=structuredClone(local),m=(value:number)=>({value,status:'derived' as const,basis:'canonical_operation_identity' as const,evidenceRefs:[]});
 const r=fixture.time.repeatedBehavior;
 for(const group of [r.afterFailure,r.repeatedRead]){group.count=m(1);group.duration={knownSumMs:m(40),recordedCount:m(1),calculatedCount:m(0),missingCount:m(0)};}
 r.combinedOperationCount=m(1);r.coverage.candidateOperations=m(3);
 const proof=(operationAlias:string,reference:string)=>({operationAlias,pages:[{cursor:null,limit:200,evidenceRefs:[reference]}]});
 const entry={later:proof('repeat:0:later','event:later'),afterFailure:proof('repeat:0:failure','event:failure'),successfulReads:[proof('repeat:0:read:0','event:read')],repeatedReadTargetCount:1,laterDurationMs:m(40),recoverySpanMs:m(80)};
 const navigation={detail:{support:'supported',reason:'exact_event_page'},candidateOperationCount:m(1),locatedOperationCount:m(1),pageCount:m(3),limitBytes:65536,entries:[entry]};
 const response={...fixture,evidence:{...fixture.evidence,repeatPages:navigation}};
 assert.equal((await client(response).timing!(summary)).action,'summary');
 const bad=[{...navigation,pageCount:m(2)},{...navigation,locatedOperationCount:m(0)},{...navigation,entries:[]},
 {...navigation,detail:{support:'unavailable',reason:'resource_limit'}},
 {...navigation,entries:[{...entry,later:proof('repeat:1:later','event:later')}]},
 {...navigation,entries:[{...entry,afterFailure:null}]},
 {...navigation,entries:[{...entry,successfulReads:[]}]},
 {...navigation,entries:[{...entry,repeatedReadTargetCount:0}]},
 {...navigation,entries:[{...entry,later:{...entry.later,pages:[{cursor:null,limit:200,evidenceRefs:['collection:turn']}]}}]},
 {...navigation,entries:[{...entry,later:{...entry.later,pages:[{cursor:null,limit:200,evidenceRefs:['event:x','event:x']}]}}]},
 {...navigation,entries:[{...entry,later:{...entry.later,pages:[{cursor:null,limit:200,evidenceRefs:['event:x']},{cursor:null,limit:200,evidenceRefs:['event:y']}]}}]},
 {...navigation,entries:[{...entry,requestFingerprint:'PRIVATE'}]},
 ];
 for(const repeatPages of bad)await assert.rejects(client({...fixture,evidence:{...fixture.evidence,repeatPages}}).timing!(summary),{code:'PROTOCOL_ERROR'});
 const limited={...navigation,detail:{support:'unavailable',reason:'resource_limit'},entries:[]};
 assert.equal((await client({...fixture,evidence:{...fixture.evidence,repeatPages:limited}}).timing!(summary)).action,'summary');
 await assert.rejects(client({...share,repeatPages:navigation}).timing!({...summary,privacyProfile:'share-v1'}),{code:'PROTOCOL_ERROR'});
 await assert.rejects(client({...response,outputVersion:3}).timing!(summary),{code:'PROTOCOL_ERROR'});
});


test('outcome statistics preserve a useful recorded subset and reject changed denominators or future methods',async()=>{
 const {outcomeStatistics}=await import('../../tests/fixtures/outcomes.js');
 const fixture=structuredClone(local);fixture.coverage.sourceStatus='complete';fixture.work.outcomes=outcomeStatistics();
 const request:TimingRequest={action:'summary',threadId:fixture.scope.threadId,turnId:fixture.scope.turnId,mode:'cached'};
 const read=(value:unknown)=>createUsageClient({query:async()=>{throw Error('No scan');},timing:async()=>value}).timing!(request);
 await read(fixture);
 const o=fixture.work.outcomes;
 for(const outcomes of [{...o,method:'future'}, {...o,determinateOperations:{...o.determinateOperations,value:4}},
   {...o,failureRatio:{...o.failureRatio,value:0.25}}, {...o,partial:false},
   {...o,failed:{...o.failed,status:'observed'}},{...o,failed:{...o.failed,value:null}},
   {...o,determinateOperations:{...o.determinateOperations,value:0},failed:{...o.failed,value:0},succeeded:{...o.succeeded,value:0}}]){
  await assert.rejects(read({...fixture,work:{...fixture.work,outcomes}}),{code:'PROTOCOL_ERROR'});
 }
 const zero={...o,determinateOperations:{...o.determinateOperations,value:0},failed:{...o.failed,value:0},succeeded:{...o.succeeded,value:0},failureRatio:{...o.failureRatio,value:null,status:'unavailable' as const,basis:'no_candidates' as const}};
 await read({...fixture,work:{...fixture.work,outcomes:zero}});
 for(const ratio of [0,1,NaN,Infinity])await assert.rejects(read({...fixture,work:{...fixture.work,outcomes:{...zero,failureRatio:{...zero.failureRatio,value:ratio}}}}),{code:'PROTOCOL_ERROR'});
 await assert.rejects(read({...fixture,outputVersion:4}),{code:'PROTOCOL_ERROR'});
});

test('input comparisons validate endpoint arithmetic, candidate coverage and complete maxima',async()=>{
 const {inputChangeFixture}=await import('../../tests/fixtures/input-change.js');
 for(const delta of [19900,0,-50]){
  const inputChange=inputChangeFixture(delta),response={...local,context:{...local.context,inputChange}};
  assert.equal(await client(response).timing!(summary),response);
 }
 const change=inputChangeFixture(),s=change.statistics!;
 for(const statistics of [{...s,candidates:3},{...s,increasingStages:2},{...s,maximumIncrease:{...s.maximumIncrease,value:19899}},
  {...s,stages:[{...s.stages[0],delta:20000}]},{...s,stages:[{...s.stages[0],factor:null}]},
  {...s,stages:[{...s.stages[0],firstRef:s.stages[0].lastRef}]},{...s,stages:[],detailsOmitted:false}]){
  await assert.rejects(client({...local,context:{...local.context,inputChange:{...change,statistics}}}).timing!(summary),{code:'PROTOCOL_ERROR'});
 }
 const zero={...s,comparableStages:0,increasingStages:0,largestIncrease:null,stages:[],maximumIncrease:{value:null,status:'unavailable' as const,basis:'no_candidates' as const,evidenceRefs:[]}};
 await client({...local,context:{...local.context,inputChange:{...change,statistics:zero}}}).timing!(summary);
});
