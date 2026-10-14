import type { Request as ConversationRequest } from './generated/conversation-request.js';
import type { Response as ConversationResult } from './generated/conversation-response.js';
export type { Request as ConversationRequest } from './generated/conversation-request.js';
export type { Response as ConversationResult } from './generated/conversation-response.js';
export type ConversationTransport = (request: ConversationRequest, options: QueryOptions) => Promise<unknown>;
import type { Request as MonitorRequest } from './generated/monitor-request.js';
import type { Response as MonitorResult } from './generated/monitor-response.js';
export type { Request as MonitorRequest } from './generated/monitor-request.js';
export type { Response as MonitorResult } from './generated/monitor-response.js';
export type MonitorTransport = (request: MonitorRequest, options: QueryOptions) => Promise<unknown>;
import type { Request as SetupRequest } from './generated/setup-request.js';
import type { Response as SetupResult } from './generated/setup-response.js';
export type { Request as SetupRequest } from './generated/setup-request.js';
export type { Response as SetupResult } from './generated/setup-response.js';
export type SetupTransport = (request: SetupRequest, options: QueryOptions) => Promise<unknown>;
import type { Request as CollectionRequest } from './generated/collection-request.js';
import type { Response as CollectionResult } from './generated/collection-response.js';
export type { Request as CollectionRequest } from './generated/collection-request.js';
export type { Response as CollectionResult } from './generated/collection-response.js';
export type CollectionTransport = (
  request: CollectionRequest,
  options: QueryOptions,
) => Promise<unknown>;
import type { ShareResponse as TimingShareResult } from './generated/timing-share-response.js';
import type { LocalResponse as TimingLocalResult } from './generated/timing-local-response.js';
import type { Request as TimingRequest } from './generated/timing-request.js';
import type { Response as TimingResult } from './generated/timing-response.js';
export type { Request as TimingRequest } from './generated/timing-request.js';
export type { Response as TimingResult } from './generated/timing-response.js';
export type { LocalResponse as TimingLocalResult } from './generated/timing-local-response.js';
export type { ShareResponse as TimingShareResult } from './generated/timing-share-response.js';
export type TimingTransport = (request: TimingRequest, options: QueryOptions) => Promise<unknown>;
import type { Request as LiveRequest } from './generated/live-request.js';
import type { Response as LiveResult } from './generated/live-response.js';
import type { Request as AccountRequest } from './generated/account-request.js';
import type { Response as AccountResult } from './generated/account-response.js';
export type { Request as AccountRequest } from './generated/account-request.js';
export type { Response as AccountResult } from './generated/account-response.js';
export type AccountTransport = (request: AccountRequest, options: QueryOptions) => Promise<unknown>;
import type { Request as HandoffRequest } from './generated/handoff-request.js';
import type { Response as HandoffResult } from './generated/handoff-response.js';
export type { Request as HandoffRequest } from './generated/handoff-request.js';
export type { Response as HandoffResult } from './generated/handoff-response.js';
export type HandoffTransport = (request: HandoffRequest, options: QueryOptions) => Promise<unknown>;
export interface HostTransports {
  account?: AccountTransport;
  handoff?: HandoffTransport;
}
export type { Request as LiveRequest } from './generated/live-request.js';
export type { Response as LiveResult } from './generated/live-response.js';
import { CoreError } from './errors.js';
import type { Request as DirectoriesRequest } from './generated/directories-request.js';
import type { Response as DirectoriesResult } from './generated/directories-response.js';
export type { Request as DirectoriesRequest } from './generated/directories-request.js';
export type { Response as DirectoriesResult } from './generated/directories-response.js';
export type DirectoriesTransport = (
  request: DirectoriesRequest,
  options: QueryOptions,
) => Promise<unknown>;
import type { Request as PreferencesRequest } from './generated/preferences-request.js';
import type { Response as PreferencesResult } from './generated/preferences-response.js';
export type { Request as PreferencesRequest } from './generated/preferences-request.js';
export type { Response as PreferencesResult } from './generated/preferences-response.js';
export type PreferencesTransport = (
  request: PreferencesRequest,
  options: QueryOptions,
) => Promise<unknown>;
import type { Request as OptimizeRequest } from './generated/optimize-request.js';
import type { Response as OptimizeResult } from './generated/optimize-response.js';
export type { Request as OptimizeRequest } from './generated/optimize-request.js';
export type {
  Response as OptimizeResult,
  Suggestion as OptimizeSuggestion,
} from './generated/optimize-response.js';
export type OptimizeTransport = (
  request: OptimizeRequest,
  options: QueryOptions,
) => Promise<unknown>;
import type { Request as ConfigRequest } from './generated/config-request.js';
import type { Response as ConfigResult } from './generated/config-response.js';
export type { Request as ConfigRequest } from './generated/config-request.js';
export type { Response as ConfigResult, Item as ConfigItem } from './generated/config-response.js';
export type ConfigTransport = (request: ConfigRequest, options: QueryOptions) => Promise<unknown>;
import type { Request } from './generated/usage-request.js';
import type { Response } from './generated/usage-app.js';
import type { Request as PricingRequest } from './generated/pricing-request.js';
import type { Response as PricingResult } from './generated/pricing-response.js';
export type { Request as PricingRequest } from './generated/pricing-request.js';
export type { Response as PricingResult } from './generated/pricing-response.js';

export type { Request as UsageRequest } from './generated/usage-request.js';
export type {
  Response as UsageResult,
  Scope as UsageScope,
  Item as UsageItem,
  UsageSummary,
} from './generated/usage-app.js';

export interface QueryOptions {
  signal?: AbortSignal;
  onProgress?: (stage: string) => void;
}

/** Only generated product operations can cross the host boundary. */
export type UsageTransport = (request: Request, options: QueryOptions) => Promise<unknown>;
export type PricingTransport = (request: PricingRequest, options: QueryOptions) => Promise<unknown>;

export type LiveTransport = (request: LiveRequest, options: QueryOptions) => Promise<unknown>;

export interface UsageClient {
  conversation?(request: ConversationRequest, options?: QueryOptions): Promise<ConversationResult>;
  monitor?(request: MonitorRequest, options?: QueryOptions): Promise<MonitorResult>;
  setup?(request: SetupRequest, options?: QueryOptions): Promise<SetupResult>;
  collection?(request: CollectionRequest, options?: QueryOptions): Promise<CollectionResult>;
  timing?(request: TimingRequest, options?: QueryOptions): Promise<TimingResult>;
  handoff?(request: HandoffRequest, options?: QueryOptions): Promise<HandoffResult>;
  account?(request: AccountRequest, options?: QueryOptions): Promise<AccountResult>;
  directories?(request: DirectoriesRequest, options?: QueryOptions): Promise<DirectoriesResult>;
  preferences?(request: PreferencesRequest, options?: QueryOptions): Promise<PreferencesResult>;
  optimize?(request: OptimizeRequest, options?: QueryOptions): Promise<OptimizeResult>;
  config?(request: ConfigRequest, options?: QueryOptions): Promise<ConfigResult>;
  live?(request: LiveRequest, options?: QueryOptions): Promise<LiveResult>;
  query(request: Request, options?: QueryOptions): Promise<Response>;
  prices(request: PricingRequest, options?: QueryOptions): Promise<PricingResult>;
}

export interface ClientTransports extends HostTransports {
  conversation?: ConversationTransport;
  monitor?: MonitorTransport;
  setup?: SetupTransport;
  collection?: CollectionTransport;
  query: UsageTransport;
  prices?: PricingTransport;
  live?: LiveTransport;
  config?: ConfigTransport;
  optimize?: OptimizeTransport;
  preferences?: PreferencesTransport;
  directories?: DirectoriesTransport;
  timing?: TimingTransport;
}

function throwIfCancelled(options: QueryOptions): void {
  if (options.signal?.aborted) throw new CoreError('CANCELLED', 'Cancelled');
}

export function createUsageClient(transports: ClientTransports): UsageClient {
  const {
    query: transport,
    prices: pricingTransport,
    live: liveTransport,
    config: configTransport,
    optimize: optimizeTransport,
    preferences: preferencesTransport,
    directories: directoriesTransport,
  } = transports;
  const hosts = transports;
  return {
    ...(transports.monitor
      ? {
          async monitor(
            request: MonitorRequest,
            options: QueryOptions = {},
          ): Promise<MonitorResult> {
            const [{ validate: input }, { validate: output }] = await Promise.all([
              import('./generated/validate-monitor-request.js'),
              import('./generated/validate-monitor-response.js'),
            ]);
            throwIfCancelled(options);
            if (!input(request)) throw new CoreError('INVALID_ARGUMENT', 'Invalid monitor request');
            const result = await transports.monitor!(request, options);
            throwIfCancelled(options);
            if (
              !output(result) ||
              result.outputVersion !== 1 ||
              result.action !== request.action ||
              (request.action === 'check'
                ? result.snapshotId !== request.snapshotId
                : result.snapshotId != null)
            )
              throw new CoreError('PROTOCOL_ERROR', 'Invalid monitor response');
            return result;
          },
        }
      : {}),
    ...(transports.setup
      ? {
          async setup(request: SetupRequest, options: QueryOptions = {}): Promise<SetupResult> {
            const [{ validate: input }, { validate: output }] = await Promise.all([
              import('./generated/validate-setup-request.js'),
              import('./generated/validate-setup-response.js'),
            ]);
            throwIfCancelled(options);
            if (!input(request)) throw new CoreError('INVALID_ARGUMENT', 'Invalid setup request');
            const result = await transports.setup!(request, options);
            throwIfCancelled(options);
            if (
              !output(result) ||
              result.outputVersion !== 1 ||
              result.project !== (request.project ?? null)
            )
              throw new CoreError('PROTOCOL_ERROR', 'Invalid setup response');
            return result;
          },
        }
      : {}),
    ...(transports.collection
      ? {
          async collection(
            request: CollectionRequest,
            options: QueryOptions = {},
          ): Promise<CollectionResult> {
            const [{ validate: input }, { validate: output }] = await Promise.all([
              import('./generated/validate-collection-request.js'),
              import('./generated/validate-collection-response.js'),
            ]);
            throwIfCancelled(options);
            if (!input(request))
              throw new CoreError('INVALID_ARGUMENT', 'Invalid collection request');
            const result = await transports.collection!(request, options);
            throwIfCancelled(options);
            if (
              !output(result) ||
              result.outputVersion !== 1 ||
              result.action !== (request.action ?? 'status')
            )
              throw new CoreError('PROTOCOL_ERROR', 'Invalid collection response');
            return result;
          },
        }
      : {}),
    ...(transports.conversation ? {
      async conversation(request: ConversationRequest, options: QueryOptions = {}): Promise<ConversationResult> {
        const [{validate: input}, {validate: output}] = await Promise.all([import('./generated/validate-conversation-request.js'), import('./generated/validate-conversation-response.js')]);
        throwIfCancelled(options);
        if (!input(request)) throw new CoreError('INVALID_ARGUMENT', 'Invalid conversation request');
        const result = await transports.conversation!(request, options);
        throwIfCancelled(options);
        if (!output(result) || !matchesConversationSelection(request, result) || result.outputVersion !== 1 || result.action !== request.action || result.snapshotId !== request.snapshotId || result.baselineSnapshotId !== (request.baselineSnapshotId ?? null) || (request.action === 'read' && result.items.length !== 0) || (request.action !== 'read' && result.fragments.length !== 0) || result.scanComplete !== (result.nextCursor == null)) throw new CoreError('PROTOCOL_ERROR', 'Invalid conversation response');
        return result;
      },
    } : {}),
    ...(transports.timing
      ? {
          async timing(request: TimingRequest, options: QueryOptions = {}): Promise<TimingResult> {
            const [{ validate: input }, { validate: output }] = await Promise.all([
              import('./generated/validate-timing-request.js'),
              import('./generated/validate-timing-response.js'),
            ]);
            throwIfCancelled(options);
            if (
              !input(request) ||
              (request.action === 'evidence' &&
                (request.privacyProfile === 'share-v1' ||
                  (request.objectRef != null &&
                    (request.collection !== 'use_records' ||
                      !/^use:[0-9a-fA-F]{64}$/.test(request.objectRef)))))
            )
              throw new CoreError('INVALID_ARGUMENT', 'Invalid timing request');
            const result = await transports.timing!(request, options);
            throwIfCancelled(options);
            if (!output(result) || !matchesTiming(request, result))
              throw new CoreError('PROTOCOL_ERROR', 'Invalid timing response');
            return result;
          },
        }
      : {}),
    ...(hosts.handoff
      ? {
          async handoff(
            request: HandoffRequest,
            options: QueryOptions = {},
          ): Promise<HandoffResult> {
            const [{ validate: input }, { validate: output }] = await Promise.all([
              import('./generated/validate-handoff-request.js'),
              import('./generated/validate-handoff-response.js'),
            ]);
            throwIfCancelled(options);
            if (!input(request)) throw new CoreError('INVALID_ARGUMENT', 'Invalid handoff request');
            const result = await hosts.handoff!(request, options);
            if (request.action !== 'send') throwIfCancelled(options);
            if (
              !output(result) ||
              result.outputVersion !== 1 ||
              result.action !== (request.action ?? 'preview')
            )
              throw new CoreError('PROTOCOL_ERROR', 'Invalid handoff response');
            return result;
          },
        }
      : {}),
    ...(hosts.account
      ? {
          async account(
            request: AccountRequest,
            options: QueryOptions = {},
          ): Promise<AccountResult> {
            const [{ validate: input }, { validate: output }] = await Promise.all([
              import('./generated/validate-account-request.js'),
              import('./generated/validate-account-response.js'),
            ]);
            throwIfCancelled(options);
            if (!input(request)) throw new CoreError('INVALID_ARGUMENT', 'Invalid account request');
            const result = await hosts.account!(request, options);
            throwIfCancelled(options);
            if (
              !output(result) ||
              result.outputVersion !== 1 ||
              result.action !== (request.action ?? 'read') ||
              (request.action === 'history' && result.history == null)
            )
              throw new CoreError('PROTOCOL_ERROR', 'Invalid account response');
            return result;
          },
        }
      : {}),
    ...(directoriesTransport
      ? {
          async directories(
            request: DirectoriesRequest,
            options: QueryOptions = {},
          ): Promise<DirectoriesResult> {
            const [
              { validate: validateDirectoriesRequest },
              { validate: validateDirectoriesResult },
            ] = await Promise.all([
              import('./generated/validate-directories-request.js'),
              import('./generated/validate-directories-response.js'),
            ]);
            throwIfCancelled(options);
            if (!validateDirectoriesRequest(request))
              throw new CoreError('INVALID_ARGUMENT', 'Invalid directory request');
            const result = await directoriesTransport(request, options);
            if (request.action == null || ['list', 'choose'].includes(request.action))
              throwIfCancelled(options);
            if (
              !validateDirectoriesResult(result) ||
              result.outputVersion !== 1 ||
              result.action !== (request.action ?? 'list')
            )
              throw new CoreError('PROTOCOL_ERROR', 'Invalid directory response');
            return result;
          },
        }
      : {}),
    ...(preferencesTransport
      ? {
          async preferences(
            request: PreferencesRequest,
            options: QueryOptions = {},
          ): Promise<PreferencesResult> {
            const [
              { validate: validatePreferencesRequest },
              { validate: validatePreferencesResult },
            ] = await Promise.all([
              import('./generated/validate-preferences-request.js'),
              import('./generated/validate-preferences-response.js'),
            ]);
            throwIfCancelled(options);
            if (!validatePreferencesRequest(request))
              throw new CoreError('INVALID_ARGUMENT', '语言偏好参数无效');
            const result = await preferencesTransport(request, options);
            if (request.action === 'get') throwIfCancelled(options);
            if (
              !validatePreferencesResult(result) ||
              result.outputVersion !== 1 ||
              result.action !== request.action
            )
              throw new CoreError('PROTOCOL_ERROR', '语言偏好响应无效');
            return result;
          },
        }
      : {}),
    ...(optimizeTransport
      ? {
          async optimize(
            request: OptimizeRequest,
            options: QueryOptions = {},
          ): Promise<OptimizeResult> {
            const [{ validate: validateOptimizeRequest }, { validate: validateOptimizeResult }] =
              await Promise.all([
                import('./generated/validate-optimize-request.js'),
                import('./generated/validate-optimize-response.js'),
              ]);
            throwIfCancelled(options);
            if (!validateOptimizeRequest(request) || !validActivityRequest(request))
              throw new CoreError('INVALID_ARGUMENT', '优化参数不符合数据协议');
            const result = await optimizeTransport(request, options);
            throwIfCancelled(options);
            if (
              !validateOptimizeResult(result) ||
              result.outputVersion !== 4 ||
              result.action !== (request.action ?? 'list') ||
              !matchesActivity(request, result)
            )
              throw new CoreError('PROTOCOL_ERROR', '优化数据格式不正确');
            return result;
          },
        }
      : {}),
    ...(configTransport
      ? {
          async config(request: ConfigRequest, options: QueryOptions = {}): Promise<ConfigResult> {
            const [{ validate: validateConfigRequest }, { validate: validateConfigResult }] =
              await Promise.all([
                import('./generated/validate-config-request.js'),
                import('./generated/validate-config-response.js'),
              ]);
            throwIfCancelled(options);
            if (!validateConfigRequest(request))
              throw new CoreError('INVALID_ARGUMENT', '配置查询参数不符合数据协议');
            const result = await configTransport(request, options);
            throwIfCancelled(options);
            if (
              !validateConfigResult(result) ||
              result.outputVersion !== 1 ||
              result.action !== (request.action ?? 'list')
            )
              throw new CoreError('PROTOCOL_ERROR', '配置数据格式不正确');
            return result;
          },
        }
      : {}),
    ...(liveTransport
      ? {
          async live(request: LiveRequest, options: QueryOptions = {}): Promise<LiveResult> {
            const { validate: validateLiveRequest } = await import('./generated/validate-live-request.js');
            throwIfCancelled(options);
            if (!validateLiveRequest(request))
              throw new CoreError('INVALID_ARGUMENT', '实时查询参数不符合数据协议');
            // Load the response validator while the core handles the validated request.
            // Response validation remains mandatory before publishing any result.
            const [result, { validate: validateLiveResult }] = await Promise.all([
              liveTransport(request, options),
              import('./generated/validate-live-response.js'),
            ]);
            throwIfCancelled(options);
            if (
              !validateLiveResult(result) ||
              result.outputVersion !== 1 ||
              result.result.action !== request.query.action ||
              !matchesComparison(request.query, result.result) ||
              !matchesInspection(request.query, result.result) ||
              !matchesStatistics(request.query, result.result) ||
              (result.additional?.length ?? 0) !== (request.also?.length ?? 0) ||
              (result.additional ?? []).some((value, index) => { const query = request.also?.[index]; return !query || value.action !== query.action || value.snapshotRef.snapshotId !== result.result.snapshotRef.snapshotId || !matchesComparison(query, value) || !matchesInspection(query, value) || !matchesStatistics(query, value); })
            )
              throw new CoreError('PROTOCOL_ERROR', '实时用量数据格式不正确');
            return result;
          },
        }
      : {}),
    async prices(request, options = {}) {
      const [{ validate: validatePricingRequest }, { validate: validatePricingResult }] =
        await Promise.all([
          import('./generated/validate-pricing-request.js'),
          import('./generated/validate-pricing-response.js'),
        ]);
      throwIfCancelled(options);
      if (!validatePricingRequest(request))
        throw new CoreError('INVALID_ARGUMENT', '价表参数不符合数据协议');
      if (!pricingTransport) throw new CoreError('PRICING_UNAVAILABLE', '当前宿主未提供价表接口');
      const result = await pricingTransport(request, options);
      if (request.action === 'status') throwIfCancelled(options);
      if (
        !validatePricingResult(result) ||
        result.outputVersion !== 1 ||
        result.action !== request.action
      )
        throw new CoreError('PROTOCOL_ERROR', '价表数据格式不正确');
      return result;
    },
    async query(request, options = {}) {
      const [{ validate: validateRequest }, { validate: validateResponse }] = await Promise.all([
        import('./generated/validate-usage-request.js'),
        import('./generated/validate-usage-app.js'),
      ]);
      throwIfCancelled(options);
      if (!validateRequest(request))
        throw new CoreError('INVALID_ARGUMENT', '查询参数不符合数据协议');
      const result = await transport(request, options);
      throwIfCancelled(options);
      if (
        !validateResponse(result) ||
        result.outputVersion !== 5 ||
        result.action !== request.action ||
        !matchesComparison(request, result) ||
        !matchesInspection(request, result) ||
        !matchesStatistics(request, result)
      ) {
        throw new CoreError('PROTOCOL_ERROR', '用量数据格式不正确');
      }
      return result;
    },
  };
}

function matchesTiming(request: TimingRequest, result: TimingResult): boolean {
  const profile = request.privacyProfile ?? 'local';
  if (
    result.outputVersion !== 6 ||
    result.action !== request.action ||
    result.profile !== profile ||
    result.methodVersion !== 'safe_event_turn_v7'
  )
    return false;
  if (
    ('uses' in result &&
      ('totals' in result.uses ? result.uses.totals : result.uses).methodVersion !== 3) ||
    ('totals' in result && result.totals.methodVersion !== 3)
  )
    return false;
  if (
    result.action === 'summary' &&
    (!validInputChange(result.context.inputChange) ||
      !matchesOutcomeStatistics(result) ||
      !matchesOperationCoverage(result) ||
      !matchesRepeatedBehavior(result) ||
      (result.profile === 'local' && !matchesRepeatNavigation(result)))
  )
    return false;
  if (request.action === 'capabilities') return !('scope' in result) && !('readView' in result);
  if (request.action === 'summary' && profile === 'share-v1') {
    // Sharing deliberately omits local locating identities; core selection owns
    // target binding. The separate schema validates the identifier-free branch.
    return (
      'relativeAnchors' in result && !('readView' in result) && result.privacy.profile === profile
    );
  }
  if (
    !('scope' in result) ||
    !('threadId' in result.scope) ||
    result.scope.threadId !== request.threadId ||
    result.scope.turnId !== request.turnId ||
    result.scope.wholeTurn !== true ||
    result.scope.agentKind !== (request.scope?.agentKind ?? 'codex') ||
    (request.scope?.sourceInstanceId != null &&
      result.scope.sourceInstanceId !== request.scope.sourceInstanceId)
  )
    return false;
  if (request.action === 'evidence')
    return (
      'rows' in result &&
      result.snapshotId === request.snapshotId &&
      result.collection === (request.collection ?? 'turn_events') &&
      (result.collection !== 'use_records' ||
        ((result.objectRef ?? null) === (request.objectRef ?? null) &&
          (request.objectRef == null ||
            result.rows.every((row) => row.objectRef === request.objectRef))))
    );
  return (
    'readView' in result &&
    result.privacy.profile === profile &&
    (request.snapshotId == null || result.readView.snapshotId === request.snapshotId)
  );
}

/** Validate core-owned relationships without reconstructing operation intervals. */
function matchesOperationCoverage(result: Extract<TimingResult, { action: 'summary' }>): boolean {
  const coverage = result.time.operationCoverage;
  const window = result.time.observedWindowMs.value;
  const candidates = coverage.candidateOperations.value;
  const paired = coverage.pairedOperations.value;
  const conflicts = coverage.conflictingOperations.value;
  if (
    candidates != null &&
    ((paired != null && paired > candidates) ||
      (conflicts != null && conflicts > candidates) ||
      (paired != null && conflicts != null && paired > candidates - conflicts))
  )
    return false;
  if (
    window != null &&
    coverage.coveredMs.value != null &&
    coverage.residualMs.value != null &&
    coverage.coveredMs.value !== window - coverage.residualMs.value
  )
    return false;
  const ranges = coverage.residualRanges;
  if (coverage.detail.support !== 'supported') return ranges.length === 0;
  if (
    window == null ||
    coverage.residualMs.value == null ||
    coverage.residualRangeCount.value !== ranges.length
  )
    return false;
  let previousEnd = 0,
    total = 0;
  for (const range of ranges) {
    if (range.startMs >= range.endMs || range.startMs < previousEnd || range.endMs > window)
      return false;
    total += range.endMs - range.startMs;
    previousEnd = range.endMs;
  }
  return total === coverage.residualMs.value;
}

/** Check bounded aggregate relationships; matching and duration arithmetic remain in Rust. */
function matchesRepeatedBehavior(result: Extract<TimingResult, { action: 'summary' }>): boolean {
  const r = result.time.repeatedBehavior,
    c = r.coverage;
  for (const m of [r.afterFailure, r.repeatedRead]) {
    const n = m.count.value,
      d = m.duration;
    if (
      n != null &&
      d.recordedCount.value != null &&
      d.calculatedCount.value != null &&
      d.missingCount.value != null &&
      (d.recordedCount.value > n ||
        d.calculatedCount.value > n - d.recordedCount.value ||
        d.missingCount.value !== n - d.recordedCount.value - d.calculatedCount.value)
    )
      return false;
    if (
      d.knownSumMs.value != null &&
      n != null &&
      n > 0 &&
      d.recordedCount.value === 0 &&
      d.calculatedCount.value === 0
    )
      return false;
  }
  const candidates = c.candidateOperations.value,
    eligible = c.eligibleCommands.value;
  if (candidates != null && eligible != null && eligible > candidates) return false;
  const union = r.combinedOperationCount.value,
    failures = r.afterFailure.count.value,
    reads = r.repeatedRead.count.value;
  if (
    union != null &&
    ((failures != null && union < failures) ||
      (reads != null && union < reads) ||
      (candidates != null && union > candidates) ||
      (failures != null && reads != null && union - failures > reads))
  )
    return false;
  if (
    union != null &&
    r.combinedMissingIntervalCount.value != null &&
    r.combinedMissingIntervalCount.value > union
  )
    return false;
  if (
    failures != null &&
    r.missingRecoverySpanCount.value != null &&
    r.missingRecoverySpanCount.value > failures
  )
    return false;
  const window = result.time.observedWindowMs.value;
  if (window != null && r.combinedUnionMs.value != null && r.combinedUnionMs.value > window)
    return false;
  return true;
}

/** Verify navigation completeness and opaque locator structure, without rematching source requests. */
function matchesRepeatNavigation(result: TimingLocalResult): boolean {
  const navigation = result.evidence.repeatPages,
    r = result.time.repeatedBehavior;
  if (
    navigation.candidateOperationCount.value != null &&
    navigation.candidateOperationCount.value !== r.combinedOperationCount.value
  )
    return false;
  if (navigation.detail.support !== 'supported') return navigation.entries.length === 0;
  if (
    navigation.locatedOperationCount.value !== navigation.entries.length ||
    navigation.candidateOperationCount.value !== navigation.entries.length
  )
    return false;
  let pages = 0,
    proofs = 0,
    failed = 0,
    reads = 0;
  for (const [index, entry] of navigation.entries.entries()) {
    if (
      (!entry.afterFailure && entry.successfulReads.length === 0) ||
      (entry.successfulReads.length === 0) !== (entry.repeatedReadTargetCount === 0) ||
      entry.repeatedReadTargetCount < entry.successfulReads.length ||
      (!entry.afterFailure && entry.recoverySpanMs.value != null)
    )
      return false;
    if (entry.afterFailure) failed++;
    if (entry.successfulReads.length) reads++;
    const selected = [
      [entry.later, `repeat:${index}:later`],
      ...(entry.afterFailure ? [[entry.afterFailure, `repeat:${index}:failure`] as const] : []),
      ...entry.successfulReads.map(
        (proof, prior) => [proof, `repeat:${index}:read:${prior}`] as const,
      ),
    ] as const;
    for (const [proof, alias] of selected) {
      if (proof.operationAlias !== alias || ++proofs > 600) return false;
      const references = new Set<string>(),
        cursors = new Set<string | null>();
      for (const page of proof.pages) {
        if (cursors.has(page.cursor?.token ?? null)) return false;
        cursors.add(page.cursor?.token ?? null);
        pages++;
        for (const ref of page.evidenceRefs) {
          if (!ref.startsWith('event:') || ref.length <= 6 || references.has(ref)) return false;
          references.add(ref);
        }
      }
      if (references.size > 16) return false;
    }
  }
  return (
    pages === navigation.pageCount.value &&
    failed === r.afterFailure.count.value &&
    reads === r.repeatedRead.count.value
  );
}

function validActivityRequest(request: OptimizeRequest): boolean {
  if (request.action !== 'activity') return request.activity == null;
  return (
    request.activity != null &&
    [
      request.readView,
      request.projectRoots,
      request.project,
      request.decisionRevision,
      request.suggestionId,
      request.itemId,
      request.decisionReason,
      request.category,
      request.offset,
      request.limit,
      request.ruleOverrides,
    ].every((value) => value == null) &&
    (request.group == null || request.group === 'pending')
  );
}
/** Validate Rust's published subset relationships; do not choose or reconstruct a denominator. */
function matchesOutcomeStatistics(result: TimingLocalResult | TimingShareResult): boolean {
  return validOutcomeStatistics(result.work.outcomes, result.coverage.sourceStatus);
}
function validOutcomeStatistics(
  o: TimingLocalResult['work']['outcomes'],
  sourceStatus: string,
): boolean {
  if (
    o.method !== 'terminal_success_failure_subset_v1' ||
    (sourceStatus !== 'complete' && !o.partial)
  )
    return false;
  const counts = [
    o.determinateOperations,
    o.succeeded,
    o.failed,
    o.interrupted,
    o.rejected,
    o.nonterminal,
    o.indeterminate,
    o.conflicting,
    o.identityGapRecords,
    o.unclassified,
  ];
  if (counts.every((metric) => metric.value == null))
    return (
      counts.every((metric) => metric.status === 'unavailable') &&
      o.failureRatio.value == null &&
      o.failureRatio.status === 'unavailable' &&
      o.partial
    );
  if (
    counts.some(
      (metric) =>
        metric.value == null ||
        metric.status !== 'derived' ||
        metric.basis !== 'determinate_terminal_outcomes',
    )
  )
    return false;
  const denominator = o.determinateOperations.value!,
    failed = o.failed.value!,
    succeeded = o.succeeded.value!;
  if (denominator !== failed + succeeded) return false;
  if (
    [o.nonterminal, o.indeterminate, o.conflicting, o.identityGapRecords, o.unclassified].some(
      (metric) => metric.value! > 0,
    ) &&
    !o.partial
  )
    return false;
  return denominator === 0
    ? o.failureRatio.value == null &&
        o.failureRatio.status === 'unavailable' &&
        o.failureRatio.basis === 'no_candidates'
    : o.failureRatio.status === 'derived' &&
        o.failureRatio.basis === 'determinate_terminal_outcomes' &&
        o.failureRatio.value != null &&
        o.failureRatio.value === failed / denominator;
}
/** Validate bounded core-owned comparisons; do not reconstruct historical stages in the client. */
function validInputChange(change: TimingLocalResult['context']['inputChange']): boolean {
  const s = change.statistics;
  if (change.method !== 'request_input_observation_change_v1') return false;
  if (change.availability.support !== 'supported') return s == null;
  if (change.availability.reason !== 'request_input' || !s) return false;
  if (
    s.orderedSamples + s.nonRequestScoped + s.missingInput + s.unassociated + s.numericRange !==
      s.candidates ||
    s.increasingStages + s.decreasingStages + s.unchangedStages !== s.comparableStages ||
    s.comparableStages * 2 > s.orderedSamples ||
    s.stages.length > s.comparableStages ||
    (!s.detailsOmitted && s.stages.length !== s.comparableStages) ||
    (s.nonRequestScoped + s.missingInput + s.unassociated + s.numericRange > 0 && !s.partial)
  )
    return false;
  const max = s.maximumIncrease;
  if (s.comparableStages === 0) {
    if (max.value != null || max.status !== 'unavailable' || max.basis !== 'no_candidates')
      return false;
  } else if (
    max.value == null ||
    max.status !== 'derived' ||
    max.basis !== 'request_input' ||
    max.value > 0 !== s.increasingStages > 0
  )
    return false;
  const validStage = (stage: NonNullable<typeof s.largestIncrease>) =>
    stage.firstRef !== stage.lastRef &&
    stage.delta === stage.lastInput - stage.firstInput &&
    (stage.firstInput === 0
      ? stage.factor == null
      : stage.factor === stage.lastInput / stage.firstInput);
  if (
    s.stages.some((stage) => !validStage(stage)) ||
    new Set(s.stages.map((stage) => stage.id)).size !== s.stages.length ||
    s.stages.reduce((n, stage) => n + stage.samples, 0) > s.orderedSamples
  )
    return false;
  if (s.largestIncrease) {
    if (
      !validStage(s.largestIncrease) ||
      s.largestIncrease.delta <= 0 ||
      s.largestIncrease.delta !== max.value
    )
      return false;
  } else if (s.increasingStages > 0 && !s.detailsOmitted) return false;
  return s.stages.every((stage) => stage.delta <= Math.max(max.value ?? 0, 0));
}
function matchesActivity(request: OptimizeRequest, result: OptimizeResult): boolean {
  if (request.action !== 'activity') return result.activity == null;
  const activity = result.activity,
    selected = request.activity;
  if (
    !activity ||
    !selected ||
    activity.formatVersion !== 3 ||
    activity.analysisMethod !== 'safe_event_turn_v7' ||
    activity.scope.agentKind !== 'codex' ||
    !activity.scope.wholeTurn ||
    activity.readView.snapshotId !== selected.snapshotId ||
    activity.scope.threadId !== selected.threadId ||
    activity.scope.turnId !== selected.turnId ||
    (request.sourceInstanceId != null &&
      activity.scope.sourceInstanceId !== request.sourceInstanceId) ||
    result.usageRevision !== selected.snapshotId ||
    result.readView != null ||
    result.suggestions.length ||
    result.checks.length ||
    result.followUps.length
  )
    return false;
  const definitions = [
    ['inspect_calls_after_failure', 'same_operation_after_failure_v1', 'repeat_after_failure'],
    ['inspect_repeated_reads', 'same_target_read_v1', 'successful_read_repeat'],
    ['inspect_repeated_requests', 'same_request_observation_v1', 'same_request_observation'],
    ['inspect_failure_share', 'terminal_failure_inspection_v1', 'determinate_terminal_outcomes'],
    ['inspect_input_change', 'request_input_change_inspection_v1', 'request_input'],
  ] as const;
  for (const [index, check] of activity.checks.entries()) {
    const expected = definitions[index];
    if (
      !expected ||
      check.rule !== expected[0] ||
      check.method !== expected[1] ||
      check.version !== 1
    )
      return false;
    if (index === 4) {
      const change = check.inputChange,
        policy = check.inputPolicy;
      if (
        !change ||
        !policy ||
        !validInputChange(change) ||
        check.outcomes != null ||
        check.failurePolicy != null
      )
        return false;
      const stats = change.statistics,
        available = change.availability.support === 'supported';
      const outcome =
        !available || !stats || stats.comparableStages === 0
          ? 'insufficient'
          : stats.maximumIncrease.value! >= policy.minimumIncrease
            ? 'hit'
            : 'miss';
      const reason = !available
        ? 'activityMeasureUnavailable'
        : !stats || stats.comparableStages === 0
          ? 'activitySampleTooSmall'
          : null;
      if (
        check.outcome !== outcome ||
        check.reason !== reason ||
        check.partial !== (activity.sourceStatus !== 'complete' || (stats?.partial ?? true))
      )
        return false;
      if (stats) {
        const m = stats.maximumIncrease;
        if (
          check.observed.value !== m.value ||
          check.observed.status !== m.status ||
          check.observed.basis !== m.basis ||
          check.observed.evidenceRefs.length !== m.evidenceRefs.length ||
          check.observed.evidenceRefs.some((ref, i) => ref !== m.evidenceRefs[i])
        )
          return false;
      } else if (
        check.observed.value != null ||
        check.observed.status !== 'unavailable' ||
        check.observed.basis !== change.availability.reason
      )
        return false;
      continue;
    }
    if (check.inputChange != null || check.inputPolicy != null) return false;
    if (index === 3) {
      const o = check.outcomes,
        policy = check.failurePolicy;
      if (
        !o ||
        !policy ||
        !validOutcomeStatistics(o, activity.sourceStatus) ||
        check.partial !== o.partial ||
        check.observed.value !== o.failed.value ||
        check.observed.status !== o.failed.status ||
        check.observed.basis !== o.failed.basis ||
        check.observed.evidenceRefs.length !== o.failed.evidenceRefs.length ||
        check.observed.evidenceRefs.some((ref, i) => ref !== o.failed.evidenceRefs[i])
      )
        return false;
      const n = o.determinateOperations.value;
      const outcome =
        n == null || n < policy.minimumDeterminate
          ? 'insufficient'
          : o.failed.value! >= policy.minimumFailures &&
              o.failureRatio.value! >= policy.minimumRatio
            ? 'hit'
            : 'miss';
      const reason =
        n == null
          ? 'activityMeasureUnavailable'
          : n < policy.minimumDeterminate
            ? 'activitySampleTooSmall'
            : null;
      if (check.outcome !== outcome || check.reason !== reason) return false;
      continue;
    }
    if (check.outcomes != null || check.failurePolicy != null) return false;
    if (
      check.outcome === 'hit' &&
      (check.observed.value == null ||
        check.observed.value <= 0 ||
        check.observed.basis !== expected[2] ||
        check.observed.status !== 'derived' ||
        check.reason != null)
    )
      return false;
    if (
      check.outcome === 'miss' &&
      (check.observed.value !== 0 ||
        check.partial ||
        check.reason != null ||
        check.observed.basis !== expected[2] ||
        check.observed.status !== 'derived')
    )
      return false;
  }
  const confirmed = activity.checks.slice(0, 2).some((check) => check.outcome === 'hit');
  const expected = activity.checks
    .filter(
      (check) =>
        check.outcome === 'hit' && (!confirmed || check.rule !== 'inspect_repeated_requests'),
    )
    .map((check) => check.rule);
  return (
    activity.advice.length === expected.length &&
    activity.advice.every((rule, index) => rule === expected[index])
  );
}

function matchesComparison(request: Request, result: Response): boolean {
  if (request.action !== 'compare') return result.comparison == null;
  const input = request.comparison,
    output = result.comparison;
  if (!input || !output || input.kind !== output.kind) return false;
  if (request.snapshotId != null && result.snapshotRef.snapshotId !== request.snapshotId)
    return false;
  if (input.kind === 'periods' && output.kind === 'periods')
    return (
      output.dimension === input.dimension &&
      output.baseline.scope.since === input.baselineSince &&
      output.baseline.scope.until === input.baselineUntil &&
      output.current.scope.since === request.scope?.since &&
      output.current.scope.until === request.scope?.until
    );
  if (input.kind === 'sessions' && output.kind === 'sessions')
    return (
      output.left.threadId === input.leftThreadId &&
      output.right.threadId === input.rightThreadId &&
      output.includeDescendants === (input.includeDescendants ?? false)
    );
  return false;
}

function matchesInspection(request: Request, result: Response): boolean {
  const plans = ['investigate', 'trajectory', 'resources', 'review', 'context'];
  if (!plans.includes(request.action)) return result.inspection == null;
  const output = result.inspection;
  if (!output || output.methodVersion !== 3 || output.kind !== request.action) return false;
  if (
    (request.action === 'context') !== (output.context != null) ||
    (request.action === 'review') !== (output.review != null)
  )
    return false;
  if (['investigate', 'review'].includes(request.action) !== (output.opportunities != null))
    return false;
  if (['investigate', 'review'].includes(request.action) !== (output.activity != null))
    return false;
  if (request.snapshotId != null && result.snapshotRef.snapshotId !== request.snapshotId)
    return false;
  const thread = request.threadId ?? request.scope?.threadId;
  if (thread != null && result.scope.threadId !== thread) return false;
  for (const [key, value] of Object.entries(request.scope ?? {}))
    if (value != null && result.scope[key as keyof typeof result.scope] !== value) return false;
  if (
    [...output.candidates, ...(output.review?.topTasks ?? [])].some((c) =>
      c.evidence.some((p) => p.threadId !== c.threadId),
    )
  )
    return false;
  const proofs = [
    ...output.candidates.flatMap((c) => c.evidence),
    ...output.resources.flatMap((r) => r.evidence),
    ...output.trajectory.flatMap((p) => [
      p.evidence,
      ...(p.compactionComparison ? [p.compactionComparison.beforeEvidence] : []),
    ]),
    ...(output.context?.records.map((r) => r.evidence) ?? []),
    ...(output.review?.topTasks.flatMap((c) => c.evidence) ?? []),
  ];
  const sameScope = (a: typeof result.scope, b: typeof result.scope) =>
    Object.keys(a).length === Object.keys(b).length &&
    Object.entries(a).every(([key, value]) => value === b[key as keyof typeof b]);
  const proofMatches = (p: (typeof proofs)[number], scope: typeof result.scope) =>
    p.view === (p.operationId != null ? 'operation' : p.turnId != null ? 'turn' : 'task') &&
    p.methodVersion === 1 &&
    p.snapshotId === result.snapshotRef.snapshotId &&
    p.threadId.length > 0 &&
    (thread == null || p.threadId === thread) &&
    sameScope(p.scope, scope);
  if (!proofs.every((p) => proofMatches(p, result.scope))) return false;
  const activity = output.activity;
  if (!activity) return true;
  const dated =
    result.scope.since != null &&
    result.scope.until != null &&
    !result.scope.allTime &&
    !result.scope.undated;
  if (
    dated !== (activity.baselineScope != null) ||
    (activity.baselineScope != null) !== (activity.baselineCoverage != null)
  )
    return false;
  if (activity.baselineScope) {
    const since = Date.parse(result.scope.since! + 'T00:00:00Z'),
      until = Date.parse(result.scope.until! + 'T00:00:00Z');
    const start = new Date(2 * since - until);
    if (!Number.isFinite(start.getTime())) return false;
    if (
      !sameScope(activity.baselineScope, {
        ...result.scope,
        since: start.toISOString().slice(0, 10),
        until: result.scope.since,
      })
    )
      return false;
  }
  const opportunities = output.opportunities;
  if (
    opportunities &&
    (opportunities.methodVersion !== 1 ||
      opportunities.checks.length !== 17 ||
      new Set(opportunities.checks.map((c) => c.rule)).size !== 17 ||
      !opportunities.checks.every(
        (c) =>
          c.findings.length <= opportunities.limitPerCheck &&
          c.findingCount >= c.findings.length &&
          (c.status === 'hit') === c.findingCount > 0 &&
          (c.status !== 'miss' || c.gaps.length === 0) &&
          c.findings.every(
            (f) =>
              f.evidence.every((p) => proofMatches(p, result.scope)) &&
              f.baselineEvidence.every(
                (p) => activity.baselineScope != null && proofMatches(p, activity.baselineScope),
              ),
          ),
      ))
  )
    return false;
  return (
    activity.findings.length <= activity.limit &&
    activity.findingCount >= activity.findings.length &&
    activity.findings.every(
      (f) =>
        (result.scope.sourceInstanceId == null ||
          f.sourceInstanceId === result.scope.sourceInstanceId) &&
        (result.scope.project == null || f.project === result.scope.project) &&
        (!result.scope.projectUnknown || f.project == null) &&
        (activity.baselineScope != null) === (f.baseline != null) &&
        f.evidence.length > 0 &&
        f.evidence.every((p) => proofMatches(p, result.scope)) &&
        f.baselineEvidence.every(
          (p) => activity.baselineScope != null && proofMatches(p, activity.baselineScope),
        ),
    )
  );
}

function matchesStatistics(request: Request, result: Response): boolean {
  if (request.action !== 'statistics') return result.statistics == null;
  const s = result.statistics;
  if (
    !s ||
    s.methodVersion !== 1 ||
    s.quantileMethod !== 'type_7' ||
    s.dimension !== (request.presentation ?? null)
  )
    return false;
  if (request.snapshotId != null && request.snapshotId !== result.snapshotRef.snapshotId)
    return false;
  if (
    (request.threadId != null) !== (s.selectedTask != null) ||
    (s.selectedTask && s.selectedTask.threadId !== request.threadId)
  )
    return false;
  if ((request.comparison != null) !== (s.growth != null)) return false;
  if (
    request.comparison?.kind === 'periods' &&
    s.growth &&
    (s.growth.baselineScope.since !== request.comparison.baselineSince ||
      s.growth.baselineScope.until !== request.comparison.baselineUntil)
  )
    return false;
  return (
    s.groups.length <= result.page.limit &&
    [s.population, ...s.groups.map((g) => g.population)].every(
      (p) => p.completeTasks + p.incompleteTasks === p.measuredTasks,
    )
  );
}

function matchesConversationSelection(request: ConversationRequest, result: ConversationResult): boolean {
  const defaults = { since: null, until: null, timezone: null, includeUndated: false, sources: [], projects: [], threads: [], turns: [], origins: [], phases: [], recordKinds: [] };
  if (!Object.entries(defaults).every(([key, fallback]) => JSON.stringify((request.selection ?? {})[key as keyof NonNullable<ConversationRequest['selection']>] ?? fallback) === JSON.stringify(result.selection[key as keyof typeof result.selection] ?? fallback))) return false;
  if (Boolean(request.anchor) !== Boolean(result.window)) return false;
  if (request.anchor && result.window && (!sameConversationEvidence(request.anchor,result.window.anchor) || result.window.radius !== (request.radius ?? 20))) return false;
  const s=request.selection ?? {}, has=(values: readonly string[] | undefined, value: string | null | undefined) => !values?.length || (value != null && values.includes(value));
  if (!result.items.every(i=>has(s.sources,i.evidence.sourceInstanceId) && has(s.projects,i.project) && has(s.threads,i.evidence.threadId) && has(s.turns,i.evidence.turnId) && has(s.origins,i.origin) && has(s.phases,i.phase) && has(s.recordKinds,i.recordKind))) return false;
  if (request.action === 'read' && (result.fragments.length !== request.targets?.length || !result.fragments.every((f,i)=>sameConversationEvidence(f.evidence,request.targets![i].evidence)))) return false;
  return !result.fragments.some(f=>f.partial) || result.partial;
}
function sameConversationEvidence(a: NonNullable<ConversationRequest['targets']>[number]['evidence'], b: NonNullable<ConversationRequest['targets']>[number]['evidence']): boolean {
 return a.eventId===b.eventId && a.sourceInstanceId===b.sourceInstanceId && (a.threadId??null)===(b.threadId??null) && (a.turnId??null)===(b.turnId??null) && a.eventOffset===b.eventOffset;
}
