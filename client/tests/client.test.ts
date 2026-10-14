import { withTokenAnalysis } from '../../tests/fixtures/token-analysis.js';
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { CoreError, createUsageClient, type UsageRequest, type UsageResult } from '@wombat/client';

const response: UsageResult = {
  outputVersion: 5,
  action: 'usage',
  snapshotRef: { snapshotId: 'synthetic', createdAt: '2026-09-30T00:00:00Z' },
  scope: {},
  availableRange: {},
  summary: withTokenAnalysis({
    tokens: { input: 10, output: 2, total: 12 },
    measurementCount: 1,
    price: {
      currency: 'USD',
      policy: 'synthetic',
      priceRevision: 'synthetic',
      cost: '0.1',
      knownCost: '0.1',
      status: 'priced',
      components: [],
      basis: [],
      issues: [],
    },
  }),
  items: [],
  page: { offset: 0, limit: 50, total: 0 },
  quality: { status: 'complete', issues: [], sources: [] },
};

test('live batches reject missing, reordered or differently pinned results',async()=>{
 const base={outputVersion:1,result:response,freshness:{status:'current',revision:'synthetic'},additional:[{...response,action:'threads'}]};
 const request={query:{action:'usage'},mode:'cached',also:[{action:'threads'}]} as const;
 for(const additional of [[],[{...response,action:'usage'}],[{...response,action:'threads',snapshotRef:{...response.snapshotRef,snapshotId:'other'}}]]){
  const client=createUsageClient({query:async()=>response,live:async()=>({...base,additional})});
  await assert.rejects(client.live!({...request,also:[...request.also]}),{code:'PROTOCOL_ERROR'});
 }
 const client=createUsageClient({query:async()=>response,live:async()=>base});
 assert.deepEqual(await client.live!({...request,also:[...request.also]}),base);
});

test('a cancelled usage read discards a valid late transport response', async () => {
  const controller = new AbortController();
  let started!: () => void, finish!: (value: UsageResult) => void;
  const ready = new Promise<void>((resolve) => {
    started = resolve;
  });
  const result = new Promise<UsageResult>((resolve) => {
    finish = resolve;
  });
  const client = createUsageClient({
    query: async () => {
      started();
      return result;
    },
  });
  const pending = client.query({ action: 'usage' }, { signal: controller.signal });
  await ready;
  controller.abort();
  finish(response);
  await assert.rejects(pending, { code: 'CANCELLED' });
});

test('portable read interfaces share late cancellation semantics', async () => {
  const reads = [
    (signal: AbortSignal, late: () => Promise<unknown>) =>
      createUsageClient({ query: late, config: late }).config!({ action: 'list' }, { signal }),
    (signal: AbortSignal, late: () => Promise<unknown>) =>
      createUsageClient({ query: late, live: late }).live!(
        { query: { action: 'usage' } },
        { signal },
      ),
    (signal: AbortSignal, late: () => Promise<unknown>) =>
      createUsageClient({ query: late, prices: late }).prices({ action: 'status' }, { signal }),
    (signal: AbortSignal, late: () => Promise<unknown>) =>
      createUsageClient({ query: late, account: late }).account!({ action: 'read' }, { signal }),
    (signal: AbortSignal, late: () => Promise<unknown>) =>
      createUsageClient({ query: late, directories: late }).directories!(
        { action: 'list' },
        { signal },
      ),
    (signal: AbortSignal, late: () => Promise<unknown>) =>
      createUsageClient({ query: late, handoff: late }).handoff!({ action: 'preview' }, { signal }),
    (signal: AbortSignal, late: () => Promise<unknown>) =>
      createUsageClient({ query: late, preferences: late }).preferences!(
        { action: 'get' },
        { signal },
      ),
  ];
  for (const read of reads) {
    const controller = new AbortController();
    await assert.rejects(
      read(controller.signal, async () => {
        controller.abort();
        return undefined;
      }),
      { code: 'CANCELLED' },
    );
  }
});

test('a completed preference write retains its receipt when cancellation arrives late', async () => {
  const controller = new AbortController();
  const result = { outputVersion: 1, action: 'set' as const, language: 'en' as const };
  const client = createUsageClient({
    query: async () => response,
    preferences: async () => {
      controller.abort();
      return result;
    },
  });
  assert.equal(
    await client.preferences!({ action: 'set', language: 'en' }, { signal: controller.signal }),
    result,
  );
});

test('portable comparison binds returned periods, dimensions and snapshot to the request', async () => {
  const scope = { since: '2026-09-23', until: '2026-09-30', timezone: 'UTC' };
  const request: UsageRequest = {
    action: 'compare',
    snapshotId: 'synthetic',
    scope,
    comparison: {
      kind: 'periods',
      baselineSince: '2026-09-16',
      baselineUntil: '2026-09-23',
      dimension: 'project',
    },
  };
  const comparison: NonNullable<UsageResult['comparison']> = {
    kind: 'periods',
    dimension: 'project',
    baseline: {
      scope: { ...scope, since: '2026-09-16', until: '2026-09-23' },
      usage: response.summary,
      partial: false,
    },
    current: { scope, usage: response.summary, partial: false },
    delta: { tokens: 0, cost: '0', tokenRatio: 0 },
    drivers: [],
    remaining: { tokens: 0, cost: '0', tokenRatio: 0 },
    undatedRecords: 0,
  };
  const value: UsageResult = { ...response, action: 'compare', scope, comparison };
  assert.equal(await createUsageClient({ query: async () => value }).query(request), value);
  const invalid = [
    { ...value, snapshotRef: { ...value.snapshotRef, snapshotId: 'another' } },
    { ...value, comparison: { ...comparison, dimension: 'model' } },
    { ...value, comparison: { ...comparison, baseline: { ...comparison.baseline, scope } } },
    {
      ...value,
      comparison: {
        ...comparison,
        current: { ...comparison.current, scope: { ...scope, until: '2026-10-01' } },
      },
    },
  ];
  for (const result of invalid)
    await assert.rejects(createUsageClient({ query: async () => result }).query(request), {
      code: 'PROTOCOL_ERROR',
    });
});

test('portable client validates requests before invoking any host', async () => {
  let called = false;
  const client = createUsageClient({
    query: async () => {
      called = true;
      return response;
    },
  });
  await assert.rejects(
    client.query({ action: 'shell' } as unknown as UsageRequest),
    (error: unknown) => error instanceof CoreError && error.code === 'INVALID_ARGUMENT',
  );
  await assert.rejects(client.query({ action: 'usage', limit: -1 }), /查询参数/);
  assert.equal(called, false);
});

test('portable client keeps generated results and forwards cancellation and progress', async () => {
  const controller = new AbortController();
  const stages: string[] = [];
  const client = createUsageClient({
    query: async (request, options) => {
      assert.deepEqual(request, { action: 'usage' });
      assert.equal(options.signal, controller.signal);
      options.onProgress?.('读取快照');
      return response;
    },
  });
  assert.equal(
    await client.query(
      { action: 'usage' },
      { signal: controller.signal, onProgress: (stage) => stages.push(stage) },
    ),
    response,
  );
  assert.deepEqual(stages, ['读取快照']);
  controller.abort();
  await assert.rejects(
    client.query({ action: 'usage' }, { signal: controller.signal }),
    (error: unknown) => error instanceof CoreError && error.code === 'CANCELLED',
  );
});

test('portable client rejects wrong version, malformed result and mismatched operation', async () => {
  for (const invalid of [
    { ...response, outputVersion: 3 },
    { ...response, outputVersion: 3 },
    { ...response, outputVersion: 4 },
    { ...response, action: 'refresh' },
    { ...response, summary: {} },
    null,
  ]) {
    await assert.rejects(
      createUsageClient({ query: async () => invalid }).query({ action: 'usage' }),
      (error: unknown) => error instanceof CoreError && error.code === 'PROTOCOL_ERROR',
    );
  }
});

test('configuration review and preference transports reject broad commands, bad languages and aborted operations', async () => {
  let calls = 0;
  const transport = async () => {
    calls++;
    return { outputVersion: 1, action: 'get', language: 'en' };
  };
  const client = createUsageClient({
    query: async () => response,
    optimize: transport,
    preferences: transport,
  });
  await assert.rejects(client.optimize!({ action: 'execute' } as never), {
    code: 'INVALID_ARGUMENT',
  });
  await assert.rejects(client.preferences!({ action: 'set', language: 'fr' } as never), {
    code: 'INVALID_ARGUMENT',
  });
  assert.equal(calls, 0);
  const c = new AbortController();
  c.abort();
  await assert.rejects(client.preferences!({ action: 'get' }, { signal: c.signal }), {
    code: 'CANCELLED',
  });
  assert.equal(calls, 0);
  assert.equal((await client.preferences!({ action: 'get' })).language, 'en');
  await assert.rejects(client.preferences!({ action: 'set', language: 'zh' }), {
    code: 'PROTOCOL_ERROR',
  });
});

test('usage v5 requires per-field token analysis and its exact method and scope', async () => {
  const { tokenAnalysis, ...oldSummary } = response.summary;
  const variations = [
    { ...response, summary: oldSummary },
    ...[0, 2].map((methodVersion) => ({
      ...response,
      summary: { ...response.summary, tokenAnalysis: { ...tokenAnalysis, methodVersion } },
    })),
    {
      ...response,
      summary: { ...response.summary, tokenAnalysis: { ...tokenAnalysis, scope: 'allLogs' } },
    },
    {
      ...response,
      summary: {
        ...response.summary,
        tokenAnalysis: {
          ...tokenAnalysis,
          fields: {
            ...tokenAnalysis.fields,
            total: { ...tokenAnalysis.fields.total, coveredRecords: -1 },
          },
        },
      },
    },
  ];
  for (const result of variations)
    await assert.rejects(
      createUsageClient({ query: async () => result }).query({ action: 'usage' }),
      { code: 'PROTOCOL_ERROR' },
    );
});

test('usage v5 accepts separate calculation analysis and rejects unsupported methods', async () => {
  const totalAnalysis = {
    methodVersion: 1,
    subtotal: 12,
    coveredRecords: 1,
    recordedRecords: 0,
    calculatedRecords: 1,
    unavailableRecords: 0,
    overflowRecords: 0,
  };
  const value = {
    ...response,
    summary: {
      ...response.summary,
      tokenAnalysis: { ...response.summary.tokenAnalysis, totalAnalysis },
    },
  };
  assert.deepEqual(
    await createUsageClient({ query: async () => value }).query({ action: 'usage' }),
    value,
  );
  for (const invalid of [
    { ...totalAnalysis, methodVersion: 2 },
    { ...totalAnalysis, subtotal: 9007199254740992 },
    { ...totalAnalysis, overflowRecords: -1 },
    { ...totalAnalysis, unexpected: true },
  ]) {
    await assert.rejects(
      createUsageClient({
        query: async () => ({
          ...value,
          summary: {
            ...value.summary,
            tokenAnalysis: { ...value.summary.tokenAnalysis, totalAnalysis: invalid },
          },
        }),
      }).query({ action: 'usage' }),
      { code: 'PROTOCOL_ERROR' },
    );
  }
});

const emptyOpportunities = {
  methodVersion: 1,
  policy: {
    minimumUnpricedTokens: 50000,
    minimumAmountUsd: '1',
    concentrationShare: 0.4,
    outlierMultiple: 2,
    increaseMultiple: 2,
    minimumIncreaseUsd: '1',
    minimumCacheCreated: 100000,
    maximumReadCreateRatio: 0.3,
    modelShare: 0.6,
    maximumMedianOperations: 15,
    minimumModelTasks: 5,
    minimumOutlierTasks: 5,
    minimumPermissionRequests: 5,
    permissionRequestShare: 0.1,
    longInteractionMs: 30000,
    outboundWindowMs: 300000,
    minimumRiskyDeclines: 2,
    minimumPolls: 5,
    maximumPollWaitMs: 2000,
    minimumObservedTasks: 5,
    minimumPollingTasks: 3,
    minimumPollingDays: 2,
    pollingWindowDays: 7,
  },
  checks: [
    { rule: 'unpriced_usage', status: 'miss', gaps: [], findingCount: 0, findings: [] },
    { rule: 'estimate_concentration', status: 'miss', gaps: [], findingCount: 0, findings: [] },
    { rule: 'estimate_outlier', status: 'miss', gaps: [], findingCount: 0, findings: [] },
    { rule: 'estimate_increase', status: 'miss', gaps: [], findingCount: 0, findings: [] },
    { rule: 'cache_creation_reuse', status: 'miss', gaps: [], findingCount: 0, findings: [] },
    { rule: 'model_review', status: 'miss', gaps: [], findingCount: 0, findings: [] },
    { rule: 'sensitive_read', status: 'miss', gaps: [], findingCount: 0, findings: [] },
    { rule: 'sensitive_change', status: 'miss', gaps: [], findingCount: 0, findings: [] },
    { rule: 'outside_project_change', status: 'miss', gaps: [], findingCount: 0, findings: [] },
    { rule: 'risky_command', status: 'miss', gaps: [], findingCount: 0, findings: [] },
    { rule: 'secret_exposure', status: 'miss', gaps: [], findingCount: 0, findings: [] },
    { rule: 'sensitive_outbound', status: 'miss', gaps: [], findingCount: 0, findings: [] },
    { rule: 'repeated_risky_decline', status: 'miss', gaps: [], findingCount: 0, findings: [] },
    { rule: 'permission_friction', status: 'miss', gaps: [], findingCount: 0, findings: [] },
    { rule: 'unanswered_question', status: 'miss', gaps: [], findingCount: 0, findings: [] },
    { rule: 'long_interaction', status: 'miss', gaps: [], findingCount: 0, findings: [] },
    { rule: 'frequent_polling', status: 'miss', gaps: [], findingCount: 0, findings: [] },
  ],
  limitPerCheck: 3,
};
const emptyActivity = {
  methodVersion: 1,
  policy: {
    slowDurationMs: 30000,
    minimumSlowOperations: 2,
    minimumCurrentOutcomes: 5,
    minimumBaselineOutcomes: 20,
    minimumSpikeFailures: 3,
    minimumFailureShare: 0.2,
    failureShareMultiplier: 3,
    baselineFailureShareFloor: 0.01,
    minimumBaselineDurations: 8,
    minimumBaselineMedianMs: 2000,
    durationMultiplier: 10,
    minimumDurationIncreaseMs: 20000,
    minimumWorkflowOperations: 5,
    minimumWorkflowTasks: 2,
    minimumFailuresInTask: 3,
    minimumRejectionsInTask: 2,
  },
  baselineScope: null,
  currentCoverage: {
    observedOperations: 0,
    matchedOperations: 0,
    outcomeGaps: 0,
    durationSamples: 0,
  },
  baselineCoverage: null,
  findings: [],
  findingCount: 0,
  limit: 30,
};

test('inspection evidence cannot escape the returned fixed view or original scope', async () => {
  const scope = { allTime: true, timezone: 'UTC' };
  const proof = {
    view: 'task',
    methodVersion: 1,
    snapshotId: response.snapshotRef.snapshotId,
    scope,
    threadId: 'exact-thread',
    turnId: null,
    operationId: null,
  };
  const inspection = {
    methodVersion: 3,
    kind: 'investigate',
    opportunities: emptyOpportunities,
    activity: emptyActivity,
    policy: {
      minimumTokens: 1000000,
      minimumInput: 100000,
      maximumCacheShare: 0.2,
      minimumInputJump: 100000,
      minimumDeterminateOperations: 5,
      minimumFailures: 2,
      minimumFailureShare: 0.4,
      minimumRepeatedRequests: 3,
    },
    partial: false,
    limitations: [],
    candidates: [
      {
        threadId: 'exact-thread',
        title: null,
        signals: ['high_usage'],
        usage: response.summary,
        input: 1000000,
        cacheShare: 0,
        largestUncachedJump: null,
        determinateOperations: 0,
        failedOperations: 0,
        outcomeGaps: 0,
        repeatedRequests: 0,
        evidence: [proof],
      },
    ],
    resources: [],
    trajectory: [],
    review: null,
    candidateCount: 1,
    resourceCount: 0,
    unlocatedOperations: 0,
  };
  const valid = { ...response, action: 'investigate', scope, inspection };
  assert.equal(
    (await createUsageClient({ query: async () => valid }).query({ action: 'investigate', scope }))
      .action,
    'investigate',
  );
  for (const evidence of [
    { ...proof, snapshotId: 'other-view' },
    { ...proof, scope: { ...scope, project: '/other' } },
    { ...proof, methodVersion: 2 },
    { ...proof, threadId: 'different-thread' },
  ]) {
    const invalid = {
      ...valid,
      inspection: {
        ...inspection,
        candidates: [{ ...inspection.candidates[0], evidence: [evidence] }],
      },
    };
    await assert.rejects(
      createUsageClient({ query: async () => invalid }).query({ action: 'investigate', scope }),
      { code: 'PROTOCOL_ERROR' },
    );
  }
});

test('context inventory evidence stays in its exact returned scope and snapshot', async () => {
  const scope = { allTime: true, timezone: 'UTC', threadId: 'exact-thread' };
  const proof = {
    view: 'task',
    methodVersion: 1,
    snapshotId: response.snapshotRef.snapshotId,
    scope,
    threadId: 'exact-thread',
    turnId: null,
    operationId: null,
  };
  const context = {
    observedRecords: 1,
    injectedRecords: 1,
    modelWindowRecords: 0,
    records: [
      {
        id: 'physical-record',
        kind: 'injected_context',
        timestamp: null,
        recordKind: 'response_snapshot',
        phase: 'completed',
        presence: 'non_empty',
        model: null,
        modelContextWindow: null,
        contentVersion: null,
        bytes: null,
        evidence: proof,
      },
    ],
  };
  const inspection = {
    methodVersion: 3,
    kind: 'context',
    opportunities: null,
    activity: null,
    policy: {
      minimumTokens: 1000000,
      minimumInput: 100000,
      maximumCacheShare: 0.2,
      minimumInputJump: 100000,
      minimumDeterminateOperations: 5,
      minimumFailures: 2,
      minimumFailureShare: 0.4,
      minimumRepeatedRequests: 3,
    },
    partial: false,
    limitations: ['context_metadata_unavailable', 'context_occupancy_unavailable'],
    candidates: [],
    resources: [],
    trajectory: [],
    context,
    review: null,
    candidateCount: 0,
    resourceCount: 0,
    unlocatedOperations: 0,
  };
  const valid = { ...response, action: 'context', scope, inspection };
  assert.equal(
    (await createUsageClient({ query: async () => valid }).query({ action: 'context', scope }))
      .inspection?.context?.observedRecords,
    1,
  );
  for (const evidence of [
    { ...proof, snapshotId: 'other' },
    { ...proof, scope: { allTime: true, timezone: 'UTC' } },
    { ...proof, threadId: 'other' },
    { ...proof, methodVersion: 2 },
  ]) {
    const invalid = {
      ...valid,
      inspection: {
        ...inspection,
        context: { ...context, records: [{ ...context.records[0], evidence }] },
      },
    };
    await assert.rejects(
      createUsageClient({ query: async () => invalid }).query({ action: 'context', scope }),
      { code: 'PROTOCOL_ERROR' },
    );
  }
  await assert.rejects(
    createUsageClient({
      query: async () => ({ ...valid, inspection: { ...inspection, context: null } }),
    }).query({ action: 'context', scope }),
    { code: 'PROTOCOL_ERROR' },
  );
});

test('activity baseline evidence cannot widen authorization, overlap the current period or change methods', async () => {
  const scope = {
    since: '2026-09-08',
    until: '2026-09-15',
    timezone: 'UTC',
    project: '/synthetic',
    sourceInstanceId: 'source',
  };
  const baselineScope = { ...scope, since: '2026-09-01', until: '2026-09-08' };
  const proof = {
    view: 'operation',
    methodVersion: 1,
    snapshotId: response.snapshotRef.snapshotId,
    scope,
    threadId: 'a',
    turnId: 'turn',
    operationId: 'op',
  };
  const stats = {
    operations: 2,
    tasks: 1,
    determinateOperations: 2,
    failedOperations: 0,
    rejectedOperations: 0,
    failureShare: 0,
    outcomeGaps: 0,
    durationSamples: 2,
    slowOperations: 2,
    maximumDurationMs: 30000,
    medianDurationMs: 30000,
    maximumFailuresInTask: 0,
    maximumRejectionsInTask: 0,
  };
  const finding = {
    id: 'safe-group',
    signals: ['repeated_slow_request'],
    sourceInstanceId: 'source',
    project: '/synthetic',
    tool: 'exec_command',
    current: stats,
    baseline: stats,
    evidence: [proof],
    baselineEvidence: [{ ...proof, scope: baselineScope }],
  };
  const activity = {
    ...emptyActivity,
    baselineScope,
    baselineCoverage: emptyActivity.currentCoverage,
    findings: [finding],
    findingCount: 1,
  };
  const inspection = {
    methodVersion: 3,
    kind: 'investigate',
    opportunities: emptyOpportunities,
    policy: {
      minimumTokens: 1000000,
      minimumInput: 100000,
      maximumCacheShare: 0.2,
      minimumInputJump: 100000,
      minimumDeterminateOperations: 5,
      minimumFailures: 2,
      minimumFailureShare: 0.4,
      minimumRepeatedRequests: 3,
    },
    partial: false,
    limitations: [],
    candidates: [],
    resources: [],
    trajectory: [],
    context: null,
    review: null,
    activity,
    candidateCount: 0,
    resourceCount: 0,
    unlocatedOperations: 0,
  };
  const valid = { ...response, action: 'investigate', scope, inspection };
  assert.ok(
    (await createUsageClient({ query: async () => valid }).query({ action: 'investigate', scope }))
      .inspection?.activity,
  );
  for (const invalid of [
    { ...activity, baselineScope: { ...baselineScope, project: '/other' } },
    { ...activity, baselineScope: { ...baselineScope, since: '2026-09-02' } },
    { ...activity, baselineScope: scope },
    { ...activity, methodVersion: 2 },
    { ...activity, findings: [{ ...finding, baselineEvidence: [proof] }] },
    { ...activity, findings: [{ ...finding, evidence: [{ ...proof, snapshotId: 'other' }] }] },
    { ...activity, findings: [{ ...finding, sourceInstanceId: 'foreign' }] },
    { ...activity, findings: [{ ...finding, baseline: null }] },
    { ...activity, baselineCoverage: null },
  ])
    await assert.rejects(
      createUsageClient({
        query: async () => ({ ...valid, inspection: { ...inspection, activity: invalid } }),
      }).query({ action: 'investigate', scope }),
      { code: 'PROTOCOL_ERROR' },
    );
  for (const methodVersion of [1, 2, 4])
    await assert.rejects(
      createUsageClient({
        query: async () => ({ ...valid, inspection: { ...inspection, methodVersion } }),
      }).query({ action: 'investigate', scope }),
      { code: 'PROTOCOL_ERROR' },
    );
});

test('opportunity evidence, versions and whole-check counts stay bound to the returned view', async () => {
  const scope = { allTime: true, timezone: 'UTC' },
    proof = {
      view: 'task',
      methodVersion: 1,
      snapshotId: response.snapshotRef.snapshotId,
      scope,
      threadId: 'a',
      turnId: null,
      operationId: null,
    };
  const finding = {
    id: 'synthetic-finding',
    object: null,
    metrics: [{ name: 'unpriced_tokens', value: '50000', unit: 'token' }],
    safetyLabels: [],
    evidence: [proof],
    baselineEvidence: [],
  };
  const checks = emptyOpportunities.checks.map((c) =>
    c.rule === 'unpriced_usage' ? { ...c, status: 'hit', findingCount: 1, findings: [finding] } : c,
  );
  const opportunities = { ...emptyOpportunities, checks };
  const inspection = {
    methodVersion: 3,
    kind: 'investigate',
    opportunities,
    activity: emptyActivity,
    policy: {
      minimumTokens: 1000000,
      minimumInput: 100000,
      maximumCacheShare: 0.2,
      minimumInputJump: 100000,
      minimumDeterminateOperations: 5,
      minimumFailures: 2,
      minimumFailureShare: 0.4,
      minimumRepeatedRequests: 3,
    },
    partial: false,
    limitations: [],
    candidates: [],
    resources: [],
    trajectory: [],
    review: null,
    candidateCount: 0,
    resourceCount: 0,
    unlocatedOperations: 0,
  };
  const valid = { ...response, action: 'investigate', scope, inspection };
  assert.ok(
    (await createUsageClient({ query: async () => valid }).query({ action: 'investigate', scope }))
      .inspection?.opportunities,
  );
  for (const invalid of [
    null,
    { ...opportunities, methodVersion: 2 },
    { ...opportunities, checks: checks.slice(1) },
    { ...opportunities, checks: [checks[0], ...checks.slice(0, -1)] },
    {
      ...opportunities,
      checks: checks.map((c) => (c.rule === 'unpriced_usage' ? { ...c, status: 'miss' } : c)),
    },
    {
      ...opportunities,
      checks: checks.map((c) =>
        c.rule === 'unpriced_usage'
          ? { ...c, findings: [{ ...finding, evidence: [{ ...proof, snapshotId: 'other' }] }] }
          : c,
      ),
    },
    {
      ...opportunities,
      checks: checks.map((c) =>
        c.rule === 'unpriced_usage'
          ? { ...c, findings: [{ ...finding, baselineEvidence: [proof] }] }
          : c,
      ),
    },
  ])
    await assert.rejects(
      createUsageClient({
        query: async () => ({ ...valid, inspection: { ...inspection, opportunities: invalid } }),
      }).query({ action: 'investigate', scope }),
      { code: 'PROTOCOL_ERROR' },
    );
});
