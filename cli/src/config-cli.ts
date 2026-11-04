import { type ConfigRequest, type ConfigResult, type ConfigItem } from '@wombat/client';
import { t, useBasisCount, useBasisPresentation, configUsageText, configEvidenceLabel, eventStatusLabel } from '@wombat/client/locale';
import { terminalText } from './display-text.js';

export function formatConfigText(result: ConfigResult,request:ConfigRequest={action:'list'},options:{explain?:boolean}={}):string {
  const out:string[]=[];
  const explain = options.explain || request.action === 'explain' || request.action === 'detail' || request.action === 'evidence';
  out.push(t('config.reportProject', {project: terminalText(result.scope.project ?? t('common.all'))}) + '\n');
  out.push((result.scope.allTime ? t('useBasis.allHistory') : t('config.review.window', { since: result.scope.since ?? '—', until: result.scope.until ?? '—', timezone: result.scope.timezone ?? '—' })) + '\n');
  if (explain) out.push(t('config.scopeNote') + '\n');
  if (result.coverage.historyReadComplete === false || result.coverage.historyReadComplete == null && result.coverage.historyStatus === 'partial') out.push(t('config.scanIncomplete') + '\n');
  else if (result.coverage.historyReadComplete !== true && !['current', 'fixed', 'complete','cached'].includes(result.coverage.historyStatus)) out.push(t('config.statisticsUnavailable') + '\n');
  if (result.coverage.historyReadComplete !== true && !['current', 'fixed', 'complete','cached'].includes(result.coverage.historyStatus)) {
    for (const issue of result.coverage.issues.filter(i => !/^(usageCount|historyCoverageUnknown|effectiveConfigUnknown|hookEffectiveRegistryUnavailable)/.test(i.code))) out.push(`  ${terminalText(issue.code)}${issue.path ? ' · ' + terminalText(issue.path) : ''}\n`);
  }
  if (result.extensionReview) {
    out.push(t('config.review.summary', { enabled: result.extensionReview.enabledItems, disabled: result.extensionReview.disabledItems, unknown: result.extensionReview.unconfirmedItems, measured: result.extensionReview.estimatedItems, unmeasured: result.extensionReview.unmeasuredItems }) + '\n');
    if (explain) out.push(t('config.review.note') + '\n');
  }
  if (explain && result.extensionActivity && !result.extensionReview) {
    const activity = result.extensionActivity;
    out.push(t('config.activity.counts', {used:activity.observedUse,quiet:activity.noObservedUse,unknown:activity.unavailable}) + '\n');
    for (const row of activity.items) if (row.noObservedUseDays != null) out.push(`${terminalText(result.items.find(i=>i.id===row.itemId)?.name??row.itemId)} · ${t('config.activity.days',{days:row.noObservedUseDays})}\n`);
    out.push(t('config.activity.note') + '\n');
  }
  for (const item of result.items) {
    out.push(`${item.kind}\t${terminalText(item.name)}\t${configUsageText(item,result.coverage)}\t${terminalText(item.path)}\n`);
    if (item.contextFootprint) {
      const f = item.contextFootprint;
      const state = f.contexts.some(c => c.enabled) ? t('config.hookEnabled') : f.contexts.length ? t('config.hookDisabled') : t('config.review.unconfirmed');
      out.push('  ' + t('config.review.compactRow', { state, tokens: f.estimate?.tokens ?? t('config.review.unmeasured'), last: item.lastRecordAt ?? '—' }) + '\n');
      if (explain) for (const c of f.contexts) out.push(`  ${terminalText(c.project)} · ${terminalText(c.name)} · ${terminalText(c.scope)} · ${t(c.enabled ? 'config.hookEnabled' : 'config.hookDisabled')}\n`);
    }
    if (explain && item.kind !== 'hook') {
      for (const line of configUseBasisLines(item)) out.push(`  ${line}\n`);
    }
  }
  for (const context of result.hookRegistry.contexts) for (const hook of context.registrations.filter(h => result.items.some(i => i.id === h.itemId))) {
    const plugin = hook.pluginId ? '\t' + t('config.hookPlugin', { name: terminalText(hook.pluginId) }) : '';
    out.push(`${terminalText(context.project)}\t${terminalText(hook.itemId)}\t${t(hook.enabled ? 'config.hookEnabled' : 'config.hookDisabled')}\t${t(`config.hookTrust.${hook.trust}`)}${plugin}\n`);
  }
  for (const row of result.evidence) out.push(`${row.timestamp ?? '—'}\t${row.threadId}\t${configEvidenceLabel(row.eventType)}\t${eventStatusLabel(row.outcome)}\n`);
  for (const row of result.relatedScopes) out.push(`${terminalText(row.project ?? '—')}\t${row.evidenceCount}\n`);
  if(result.explanation) {
    const explanation=result.explanation;
    out.push(t('config.explainSummary',{count:explanation.baseline.counted,records:explanation.baseline.totalRecords})+'\n');
    for(const reason of explanation.baseline.reasons)out.push(`  ${t(`config.reason.${reason.reason}`)} · ${reason.records}\n`);
    if(explanation.sourceIssues.length) {
      out.push(t('config.sourceIssues',{count:explanation.sourceIssuesTotal})+'\n');
      for(const issue of explanation.sourceIssues)out.push(`  ${terminalText(issue.code)}${issue.evidence?' · '+terminalText(issue.evidence.file)+':'+issue.evidence.line:''}\n`);
    }
    for(const row of explanation.records) {
      out.push(`${row.timestamp??'—'}\t${terminalText(row.itemId??row.operationId)}\t${t(`config.reason.${row.reason}`)}\t${row.countDelta??'—'}\n`);
      out.push(`  ${terminalText(row.threadId)}${row.turnId?' · '+terminalText(row.turnId):''}${row.evidence?' · '+terminalText(row.evidence.file)+':'+row.evidence.line:''}\n`);
      if(row.sourceObservationCount>1)out.push('  '+t('config.sourceObservations',{count:row.sourceObservationCount})+'\n');
      for(const position of row.additionalEvidence)out.push(`  ${terminalText(position.file)}:${position.line}\n`);
    }
    if(explanation.comparison) {
      const comparison=explanation.comparison;
      out.push(t(`config.comparison.${comparison.status}`)+'\n');
      for(const change of comparison.changes)out.push(`  ${terminalText(change.operationId)} · ${change.before?t(`config.reason.${change.before}`):'—'} → ${change.after?t(`config.reason.${change.after}`):'—'} · ${change.beforeDelta??'—'} → ${change.afterDelta??'—'}\n`);
      if(!comparison.changesComplete&&comparison.status==='changed')out.push(t('config.comparison.partial')+'\n');
    }
    out.push(t('config.inputRevision',{revision:explanation.baseline.inputRevision})+'\n');
  }
  if (explain) out.push(t('config.coverageNote') + '\n');
  else out.push(t('config.explainHint') + '\n');
  return out.join('');
}

export function configUseBasisLines(item: ConfigItem): string[] {
  const basis = useBasisPresentation(item.useBasis);
  const count = useBasisCount(item.kind === 'rule' ? item.counts.fileReads : item.usageCount, item.useBasis);
  return [...(count == null ? [] : [`${t(item.kind === 'rule' ? 'useBasis.ruleLoadOrRead' : 'useBasis.objectUse')}: ${count}`]), basis.summary, ...basis.notes, ...basis.details].map(terminalText);
}
