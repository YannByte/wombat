import type {TimingLocalResult,TimingShareResult} from '@wombat/client';
const metric = { value: null, status: 'unavailable', basis: 'not_recorded', evidenceRefs: [] } as const;
const unavailable = { support: 'unavailable', reason: 'not_recorded' } as const;
const capabilities = {
  objectUses: unavailable, wallClock: unavailable, nativeTtft: unavailable, firstContentRecordDelay: unavailable,
  lifecycleIntervals: unavailable, operationIntervals: unavailable, contextPressure: unavailable, strictResponseGap: unavailable,
  exploratoryGap: unavailable, commandLabels: unavailable, fileChanges: unavailable, messageRecords: unavailable,
};
export const capabilityResult = { outputVersion: 7, action: 'capabilities', methodVersion: 'safe_event_turn_v7', profile: 'local', capabilities } as const;
const scope = { sourceInstanceId: 'source', threadId: 'thread', turnId: 'turn', agentKind: 'codex', wholeTurn: true };
const count = () => ({ ...metric, evidenceRefs: [] });
const repeatDuration=()=>({knownSumMs:count(),recordedCount:count(),calculatedCount:count(),missingCount:count()});
const repeatedBehavior=()=>({failureMethod:'same_operation_after_failure_v1' as const,readMethod:'same_target_read_v1' as const,endpointMethodVersion:1,support:unavailable,readLayer:'same_path_range_unconfirmed' as const,afterFailure:{count:count(),duration:repeatDuration()},repeatedRead:{count:count(),duration:repeatDuration()},sameRequestObservationCount:count(),repeatedReadRequestCount:count(),recoverySpanSumMs:count(),missingRecoverySpanCount:count(),combinedOperationCount:count(),combinedUnionMs:count(),combinedMissingIntervalCount:count(),coverage:{candidateOperations:count(),eligibleCommands:count(),missingIdentityRecords:count(),excludedReceivers:count(),missingMatching:count(),conflictingOperations:count(),missingStart:count(),indeterminateOutcomes:count(),orderGaps:count(),contextBoundaries:count(),crossedContext:count(),missingClockDomain:count(),sourceMetadataGaps:count(),durationConflicts:count(),partial:true,reasonCodes:[]}});
const category = () => ({ candidates: count(), closed: count(), unionMs: count(), sumMs: count() });
const distribution = () => ({ samples: count(), median: count(), p90: count() });
export const useTotals = () => ({
  methodVersion: 3, sourceCoverage: 'unknown' as const, objectCount: count(), recordCount: count(),
  unboundTargetRecords: count(), unassignedSkillRecords: count(), unassignedMcpRecords: count(),
  coverage: { dispatchGaps: count(), identityGaps: count(), targetGaps: count(), timeGaps: count(), associatedTurnGaps: count() },
});
export const local: TimingLocalResult = {
  outputVersion: 7, action: 'summary', methodVersion: 'safe_event_turn_v7', profile: 'local',
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
      inputChange:{method:'request_input_observation_change_v1',availability:{support:'unavailable',reason:'not_recorded'},statistics:null},
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
  uses: { totals: useTotals(), detail: unavailable, limit: 50, objects: [], nextCursor: null },
  findings: [], coverage: {
    facts: count(), bytes: count(), metadata: count(), eventBlocks: count(), scopedEvents: count(), scopedMeasurements: count(),
    boundaryCandidates: count(), lifecycleCandidates: [count(), count(), count(), count()], linkedLifecycles: [count(), count(), count(), count()], conflictingLifecycles: count(), missingIdentityLifecycles: count(),
    contentCandidates: count(), domainCount: count(), missingWatermarks: count(), generationMismatches: count(), incompleteDomains: count(),
    snapshotUnassignedTotal: count(), threadUnassignedTotal: count(), sourceStatus: 'unknown',
  },
  quality: { partial: true, running: false, censored: true, reasonCodes: [], factLimit: 100000, summaryLimitBytes: 262144 },
  freshness: { status: 'fixed' }, evidence: { repeatPages: {detail:unavailable,candidateOperationCount:count(),locatedOperationCount:count(),pageCount:count(),limitBytes:65536,entries:[]}, intervalPages: { detail: unavailable, candidateIntervalCount: count(), locatedIntervalCount: count(), missingEventRefCount: count(), pageCount: count(), limitBytes: 65536, entries: [] }, collections: [], available: false, limit: 50, snapshotId: 'live:scope:fixed', refs: [], method: 'synthetic' },
};
export const share: TimingShareResult = {
  outputVersion: 7, action: 'summary', methodVersion: local.methodVersion, profile: 'share-v1',
  privacy: { profile: 'share-v1', omittedFields: ['local_ids'], aliases: 'package' },
  scope: { taskAlias: 'task-1', turnAlias: 'turn-1', wholeTurn: true }, capabilities, relativeAnchors: local.anchors,
  uses: useTotals(), time: local.time, context: local.context, work: local.work, findings: [], coverage: local.coverage, quality: local.quality,
  freshness: { status: 'fixed' }, basisCollections: [],
};
