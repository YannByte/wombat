import { numberLabel } from './index.js';
/** Presentation of Rust's pinned projection; never recompute membership or operation counts. */
import type { Item, Coverage } from '../generated/config-response.js';
import { t } from './index.js';
export type PublicUseBasis = NonNullable<Item['useBasis']>;
/** The core status controls availability; source collection status never hides an observed count. */
export function useBasisCount(count: number | null | undefined, basis: Item['useBasis']): number | undefined {
  return basis && (basis.status === 'observed' || basis.status === 'partial') && count != null ? count : undefined;
}

/** Short report wording; detailed projection gaps remain in the original basis. */
export function configUsageText(item: Item, coverage: Coverage): string {
  const rule = item.kind === 'rule';
  const count = useBasisCount(rule ? item.counts.fileReads : item.usageCount, item.useBasis);
  if (count != null && count > 0) return t(rule ? 'config.readsCount' : 'config.usesCount', { count });
  if (!rule && item.observation === 'used') return t('config.usedCountUnknown');
  if (coverage.historyReadComplete === false || coverage.historyReadComplete == null && (coverage.historyStatus === 'partial' || item.useBasis?.sourceCompleteness === 'partial')) return t('config.scanIncomplete');
  if (count === 0 && (coverage.historyReadComplete === true || ['current', 'fixed', 'complete','cached'].includes(coverage.historyStatus))) return t(rule ? 'config.noReadsDates' : 'config.unusedInWindow');
  return t('config.statisticsUnavailable');
}
export function useBasisPresentation(basis: Item['useBasis']) {
  if (!basis) return { summary: t('useBasis.unavailable'), notes: [], details: [] as string[] };
  const window = basis.scope.window;
  const range = window.kind === 'all_history' ? t('useBasis.allHistory')
    : t('useBasis.dateWindow', { since: window.since, until: window.until, timezone: window.timezone });
  const coverage = basis.coverage;
  const gap = (value: number | null | undefined) => value == null ? t('useBasis.gapUnknown') : numberLabel(value);
  return {
    summary: t(basis.status === 'observed' ? 'useBasis.observed' : basis.status === 'partial' ? 'useBasis.partialObservation' : 'useBasis.unavailable'),
    notes: [range, t(basis.sourceCompleteness === 'complete' ? 'useBasis.complete' : basis.sourceCompleteness === 'partial' ? 'useBasis.partial' : 'useBasis.sourceUnknown'), t('useBasis.sourceTime'), t('useBasis.notAbsence')],
    details: [t('useBasis.method', { version: basis.methodVersion }),
      t(basis.unit === 'object_use' ? 'useBasis.objectUse' : basis.unit === 'rule_read' ? 'useBasis.ruleRead' : 'useBasis.ruleLoadOrRead'),
      `${t('useBasis.capturedAt')}: ${basis.capturedAt}`, t('useBasis.snapshot', { id: basis.snapshotId ?? t('useBasis.notProvided') }),
      `${t('useBasis.source')}: ${basis.scope.sourceInstanceIds.join(', ') || t('useBasis.notProvided')}`,
      `${t('useBasis.project')}: ${basis.scope.project ?? t('common.all')}`,
      `${t('useBasis.thread')}: ${basis.scope.threadId ?? t('common.all')}`,
      `${t('useBasis.agent')}: ${basis.scope.agentKind ?? t('common.all')}`,
      t('useBasis.dispatch', { count: gap(coverage.dispatchGaps) }), t('useBasis.identity', { count: gap(coverage.identityGaps) }),
      t('useBasis.target', { count: gap(coverage.targetGaps) }), t('useBasis.time', { count: gap(coverage.timeGaps) }), t('useBasis.turn', { count: gap(coverage.turnGaps) })],
  };
}
