import { type ConfigRequest, type ConfigResult, type ConfigItem } from '@wombat/client';
import { t, useBasisCount, useBasisPresentation, configEvidenceLabel, eventStatusLabel } from '@wombat/client/locale';
import { terminalText } from './display-text.js';

export function formatConfigText(result: ConfigResult,request:ConfigRequest={action:'list'}):string {
  const out:string[]=[];
  out.push(t('config.scopeNote') + '\n');
  if (result.extensionReview) {
    out.push(t('config.review.window', { since: result.scope.since ?? '—', until: result.scope.until ?? '—', timezone: result.scope.timezone ?? '—' }) + '\n');
    out.push(t('config.review.summary', { enabled: result.extensionReview.enabledItems, disabled: result.extensionReview.disabledItems, unknown: result.extensionReview.unconfirmedItems, measured: result.extensionReview.estimatedItems, unmeasured: result.extensionReview.unmeasuredItems }) + '\n');
    out.push(t('config.review.note') + '\n');
  }
  if(result.extensionActivity && !result.extensionReview){const a=result.extensionActivity;out.push(t('config.activity.counts',{used:a.observedUse,quiet:a.noObservedUse,unknown:a.unavailable})+'\n');for(const item of a.items)if(item.noObservedUseDays!=null)out.push(`${terminalText(result.items.find(i=>i.id===item.itemId)?.name??item.itemId)} · ${t('config.activity.days',{days:item.noObservedUseDays})}\n`);out.push(t('config.activity.note')+'\n');}
  for (const item of result.items) {
    out.push(`${item.kind}\t${terminalText(item.name)}\t${t(`config.${item.observation}`)}\t${terminalText(item.path)}\n`);
    if (item.contextFootprint) {
      const f = item.contextFootprint;
      const state = f.contexts.some(c => c.enabled) ? t('config.hookEnabled') : f.contexts.length ? t('config.hookDisabled') : t('config.review.unconfirmed');
      const count = useBasisCount(item.usageCount, item.useBasis);
      const frequency = count == null ? '—' : item.useBasis?.status === 'partial' ? t('config.review.partialCount', {count}) : count;
      out.push('  ' + t('config.review.row', { state, tokens: f.estimate?.tokens ?? t('config.review.unmeasured'), count: frequency, last: item.lastRecordAt ?? '—' }) + '\n');
      for (const c of f.contexts) out.push(`  ${terminalText(c.project)} · ${terminalText(c.name)} · ${terminalText(c.scope)} · ${t(c.enabled ? 'config.hookEnabled' : 'config.hookDisabled')}\n`);
    }
    if (item.kind !== 'hook' && (!result.extensionReview || request.action === 'detail')) {
      for (const line of configUseBasisLines(item)) out.push(`  ${line}\n`);
    }
  }
  for (const context of result.hookRegistry.contexts) for (const hook of context.registrations.filter(h => result.items.some(i => i.id === h.itemId))) {
    const plugin = hook.pluginId ? '\t' + t('config.hookPlugin', { name: terminalText(hook.pluginId) }) : '';
    out.push(`${terminalText(context.project)}\t${terminalText(hook.itemId)}\t${t(hook.enabled ? 'config.hookEnabled' : 'config.hookDisabled')}\t${t(`config.hookTrust.${hook.trust}`)}${plugin}\n`);
  }
  for (const row of result.evidence) out.push(`${row.timestamp ?? '—'}\t${row.threadId}\t${configEvidenceLabel(row.eventType)}\t${eventStatusLabel(row.outcome)}\n`);
  for (const row of result.relatedScopes) out.push(`${terminalText(row.project ?? '—')}\t${row.evidenceCount}\n`);
  out.push(t('config.readVersion', { version: result.readView ?? '—' }) + '\n');
  out.push(t('config.coverageNote') + '\n');
  return out.join('');
}

export function configUseBasisLines(item: ConfigItem): string[] {
  const basis = useBasisPresentation(item.useBasis);
  const count = useBasisCount(item.kind === 'rule' ? item.counts.fileReads : item.usageCount, item.useBasis);
  return [...(count == null ? [] : [`${t(item.kind === 'rule' ? 'useBasis.ruleLoadOrRead' : 'useBasis.objectUse')}: ${count}`]), basis.summary, ...basis.notes, ...basis.details].map(terminalText);
}
