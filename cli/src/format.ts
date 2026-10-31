import { numberLabel, compactNumberLabel, automaticPriceText, pricingIssueText, tokenSummaryText, tokenSummaryPresentation, type SummaryTokenField } from '@wombat/client/locale';
import { t, locale, labels, monthLabel, inspectionCandidateText, inspectionActivityText, inspectionActivityFindingText, inspectionActivityPolicyText, inspectionOpportunityCheckText, inspectionOpportunityFindingText } from '@wombat/client/locale';
import stringWidth from 'string-width';
import { terminalText } from './display-text.js';
import type { UsageItem, UsageResult, UsageSummary } from '@wombat/client';
export function fit(text: string, width: number): string {
  const safe = terminalText(text);
  if (stringWidth(safe) <= width)
    return safe;
  let result = '';
  for (const { segment } of new Intl.Segmenter('zh-CN', { granularity: 'grapheme' }).segment(safe)) {
    if (stringWidth(result + segment) > width - 1)
      break;
    result += segment;
  }
  return width > 0 ? result + '…' : '';
}
export function money(value: string | null | undefined, places = 2): string {
  if (value == null || !/^\d+(?:\.\d+)?$/.test(value))
    return '—';
  const [whole, fraction = ''] = value.split('.');
  const unit = 10n ** BigInt(places);
  const original = BigInt(whole + fraction);
  if (original > 0n && BigInt(whole) === 0n && fraction.slice(0, places).padEnd(places, '0') === '0'.repeat(places))
    return `<$0.${'0'.repeat(places - 1)}1`;
  let scaled = BigInt(whole) * unit + BigInt(fraction.slice(0, places).padEnd(places, '0') || '0');
  if (Number(fraction[places] ?? '0') >= 5)
    scaled += 1n;
  const digits = (scaled / unit).toString().replace(/\B(?=(\d{3})+(?!\d))/g, ',');
  return `$${digits}.${(scaled % unit).toString().padStart(places, '0')}`;
}
export function tokens(value: number | null | undefined): string { return numberLabel(value); }
export function usageLabel(usage: UsageSummary, detail = false, compact = false): string {
  if (usage.measurementCount === 0) return t("common.no_usage_records");
  const price = usage.price.status === 'unknown' ? t("common.cost_unknown") : money(usage.price.cost ?? usage.price.knownCost, detail ? 4 : 2) + (usage.price.status === 'partial' ? '*' : '');
  const count = tokenSummaryText(usage, 'total', compact);
  return `${count} Token · ${price}`;
}
function dateParts(value: string, timezone: string): Record<string, string> { return Object.fromEntries(new Intl.DateTimeFormat('zh-CN', { timeZone: timezone, year: 'numeric', month: 'numeric', day: 'numeric', hour: '2-digit', minute: '2-digit', second: '2-digit', hourCycle: 'h23' }).formatToParts(new Date(value)).map(p => [p.type, p.value])); }
export function dateLabel(value: string | null | undefined, reference: string, timezone = 'UTC', withTime = false, precision?: string): string {
  if (!value)
    return '—';
  if (/^\d{4}-\d{2}-\d{2}$/.test(value)) {
    const [year, month, day] = value.split('-');
    return t("date.month_day", { p0: year === dateParts(reference, timezone).year ? '' : year + t("date.year_suffix"), p1: Number(month), p2: Number(day) });
  }
  if (!Number.isFinite(Date.parse(value)))
    return terminalText(value);
  const p = dateParts(value, timezone);
  const year = p.year === dateParts(reference, timezone).year ? '' : p.year + t("date.year_suffix");
  const showTime = withTime && precision !== 'date' && precision !== 'unknown';
  const showSeconds = precision ? ['second', 'millisecond', 'microsecond', 'nanosecond'].includes(precision) : /T\d{2}:\d{2}:\d{2}/.test(value);
  const fractionalDigits = ({ millisecond: 3, microsecond: 6, nanosecond: 9 } as Record<string, number>)[precision ?? ''] ?? 0;
  const fraction = value.match(/T\d{2}:\d{2}:\d{2}\.(\d+)/)?.[1];
  return t("date.month_day_time", { p0: year, p1: p.month, p2: p.day, p3: showTime ? ` ${p.hour}:${p.minute}${showSeconds ? ':' + p.second : ''}${fractionalDigits && fraction ? '.' + fraction.slice(0, fractionalDigits) : ''}` : '' });
}
export function calendarDateShift(value: string, days: number): string {
  if (!/^\d{4}-\d{2}-\d{2}$/.test(value) || !Number.isFinite(Date.parse(value)) || new Date(value).toISOString().slice(0, 10) !== value)
    return value;
  return new Date(Date.parse(value) + days * 86400000).toISOString().slice(0, 10);
}
export function rangeLabel(since: string | null | undefined, until: string | null | undefined, reference: string, timezone = 'UTC'): string {
  if (!since && !until)
    return t("common.all_dates");
  const end = until && /^\d{4}-\d{2}-\d{2}$/.test(until) ? new Date(Date.parse(until) - 86400000).toISOString().slice(0, 10) : until;
  if (since === end)
    return dateLabel(since, reference, timezone);
  const a = dateLabel(since, reference, timezone);
  const b = dateLabel(end, reference, timezone);
  if (since && end && since.slice(0, 7) === end.slice(0, 7))
    return t("date.same_month_range", { p0: a, p1: Number(end.slice(8, 10)) });
  return `${since ? a : t("common.start")}—${end ? b : t("common.now")}`;
}
function activityRange(start: string | null | undefined, end: string | null | undefined, reference: string, timezone: string, withTime = false): string {
  if (!start || !end) return dateLabel(start ?? end, reference, timezone, withTime);
  const a = dateLabel(start, reference, timezone, withTime), b = dateLabel(end, reference, timezone, withTime);
  if (a === b) return a;
  const date = dateLabel(start, reference, timezone);
  return a + '—' + (withTime && date === dateLabel(end, reference, timezone) ? b.slice(date.length + 1) : b);
}
const effortLabels: Record<string, string> = labels({ none: "common.none", minimal: "common.minimal", low: "common.low", medium: "common.medium", high: "common.high", xhigh: "common.extra_high", max: "common.maximum", ultra: "common.ultra" });
export function effort(value: string | null | undefined): string { return value ? effortLabels[value] ?? terminalText(value) : '—'; }
function modelLabel(model: string | null | undefined, reasoning: string | null | undefined): string { return `${model ?? t("common.unknown_model")} · ${effort(reasoning)}`; }
function statusLabel(status: string): string { return ({ completed: t("common.completed"), complete: t("common.completed"), success: t("common.completed"), failed: t("common.failed"), error: t("common.failed"), running: t("common.running"), interrupted: t("common.interrupted"), observed: t("common.recorded"), pending: t("common.pending"), started: t("common.start"), aborted: t("common.interrupted"), cancelled: t("common.cancelled"), unknown: '', in_progress: t("common.running") } as Record<string, string>)[status] ?? terminalText(status); }
function percent(value: number | null | undefined): string { return value == null ? '' : `${(value * 100).toFixed(1)}%`; }
function tableCount(value: number | null | undefined): string {
  const exact = tokens(value);
  if (exact.length <= 12 || value == null)
    return exact;
  return compactNumberLabel(value);
}
function pad(value: string, width: number, right = false): string {
  const truncated = fit(value, width);
  const spaces = ' '.repeat(Math.max(0, width - stringWidth(truncated)));
  return right ? spaces + truncated : truncated + spaces;
}
function usageColumns(width: number): number[] {
  // Preserve the model and amount at every width; detailed counters need a wide terminal.
  const costWidth = Math.max(width >= 110 ? 12 : 13, stringWidth(t('common.cost_unknown')));
  return width >= 110 ? [12, Math.max(18, width - 82 - costWidth), 8, 11, 10, 10, 11, 12, costWidth] : [12, Math.max(17, width - 37 - costWidth), 8, 13, costWidth];
}
function usageCells(values: string[], width: number): string {
  return values.map((value, i) => pad(value, usageColumns(width)[i], i >= 3)).join(' ');
}
export function usageTableHeader(width = 116): string {
  if (width < 68) return pad(t("cli.format.date_model_effort"), width - 16) + pad(t("cli.format.tokens_cost"), 16, true);
  return usageCells(width >= 110 ? [t("common.date"), t("common.model"), t("common.effort"), t("common.input"), t("common.output"), t("common.cache_write"), t("common.cache_read"), t("common.total_tokens"), t("common.cost_usd")] : [t("common.date"), t("common.model"), t("common.effort"), 'Token', t("common.cost_usd")], width);
}
function tokenTableCell(summary: UsageSummary, field: SummaryTokenField): string {
  const value = tokenSummaryPresentation(summary, field);
  return `${tableCount(value.value)}${value.qualifier ? '*' : ''}`;
}
function tokenCoverageLines(summary: UsageSummary, width: number, fields: readonly SummaryTokenField[] = ['total']): string[] {
  return fields.flatMap(field => {
    const value = tokenSummaryPresentation(summary, field);
    return value.qualifier || value.state === 'unavailable'
      ? wrapDisplay(`${field === 'total' ? 'Token' : categoryLabels[field] ?? field}: ${value.qualifier || value.unavailable} · ${value.description}`, width)
      : [];
  });
}
function reportCost(summary: UsageSummary): string {
  return summary.price.status === 'unknown' ? t("common.cost_unknown") : money(summary.price.cost ?? summary.price.knownCost) + (summary.price.status === 'partial' ? '*' : '');
}
function headline(left: string, right: string, width: number, preserveLeft = false): string[] {
  const available = width - stringWidth(right) - 2;
  return available >= 12 && (!preserveLeft || stringWidth(left) <= available) ? [pad(left, available) + '  ' + right] : [left, '  ' + right];
}
export function itemLines(item: UsageItem, result: UsageResult, width: number, group?: 'day' | 'week' | 'month'): string[] {
  const reference = result.snapshotRef.createdAt;
  const timezone = result.scope.timezone ?? 'UTC';
  if (item.kind === 'usage') {
    if (!result.distribution && item.date == null && item.isSubtotal) return [...headline(item.scope.project ?? (item.scope.projectUnknown ? t('webui.unassigned') : item.model ?? t('common.unknown_model')), usageLabel(item.usage, false, true), width), ...tokenCoverageLines(item.usage, width)];
    const date = item.date ? group==='month'?monthLabel(item.date):rangeLabel(item.scope.since, item.scope.until, reference, timezone) : t("common.unknown_date");
    const name = item.isSubtotal ? `${date} ›` : `↳ ${modelLabel(item.model, item.reasoningEffort)}`;
    if (width < 68) return [name, `  ${usageLabel(item.usage, false, true)}`, ...tokenCoverageLines(item.usage, width)];
    const cells = [item.isSubtotal ? date + ' ›' : '  ↳', item.isSubtotal ? '' : item.model ?? t("common.unknown_model"), item.isSubtotal ? '' : effort(item.reasoningEffort)];
    if (width >= 110) cells.push(...(['input', 'output', 'cacheCreate', 'cacheRead'] as const).map(field => tokenTableCell(item.usage, field)));
    cells.push(tokenTableCell(item.usage, 'total'), reportCost(item.usage));
    const widths = usageColumns(width);
    const wrapped = cells.map((cell, index) => wrapDisplay(cell, widths[index]));
    return [...Array.from({ length: Math.max(...wrapped.map(lines => lines.length)) }, (_, line) => usageCells(wrapped.map(lines => lines[line] ?? ''), width)), ...tokenCoverageLines(item.usage, width, width >= 110 ? ['input', 'output', 'cacheCreate', 'cacheRead', 'total'] : ['total'])];
  }
  if (item.kind === 'thread') {
    const lines = [...headline(item.title ?? t("common.untitled_thread"), usageLabel(item.threadUsage, false, true), width),
      `  ${item.project?.split('/').filter(Boolean).at(-1) ?? t("common.unknown_project")} · ${activityRange(item.startedAt, item.lastActivityAt, reference, timezone)}`,
      `  ${item.models.join(' / ') || t("common.unknown_model")} · ${item.reasoningEfforts.map(effort).join(' / ') || '—'}`];
    if (item.matchedUsage.measurementCount !== item.threadUsage.measurementCount) lines.push(t("cli.format.selected_range_value", { p0: usageLabel(item.matchedUsage, false, true) }));
    return [...lines, ...tokenCoverageLines(item.threadUsage, width), ...(item.matchedUsage.measurementCount !== item.threadUsage.measurementCount ? tokenCoverageLines(item.matchedUsage, width) : [])];
  }
  if (item.kind === 'turn') {
    const lines = [...headline(item.ordinal == null ? t("common.other_records") : t("common.turn_value", { p0: item.ordinal }), usageLabel(item.usage, false, true), width - 2),
      t("cli.format.thread_share_value_value_value", { p0: percent(item.share) || '—', p1: item.models.join(' / ') || t("common.unknown_model"), p2: item.reasoningEfforts.map(effort).join(' / ') || '—' }),
      `  ${activityRange(item.startedAt, item.endedAt, reference, timezone, true)}  ${statusLabel(item.status)}`];
    if (width >= 68 && item.share != null) {
      const size = Math.max(1, width - 2), filled = Math.round(Math.max(0, Math.min(1, item.share)) * size);
      lines.push('  ' + '━'.repeat(filled) + '─'.repeat(size - filled));
    }
    if (item.matchedUsage.measurementCount !== item.usage.measurementCount) lines.push(t("cli.format.filtered_value", { p0: usageLabel(item.matchedUsage, false, true) }));
    return [...lines, ...tokenCoverageLines(item.usage, width), ...(item.matchedUsage.measurementCount !== item.usage.measurementCount ? tokenCoverageLines(item.matchedUsage, width) : [])];
  }
  if (item.kind === 'measurement')
    return [...headline(`${dateLabel(item.timestamp, reference, timezone, true, item.timePrecision)} · ${modelLabel(item.model, item.reasoningEffort)}`, `${usageLabel(item.usage, true, true)} · ${percent(item.share)}`, width, true), ...tokenCoverageLines(item.usage, width)];
  const extras = [statusLabel(item.status), item.exitCode != null ? t("common.exit_value", { p0: item.exitCode }) : '', item.durationMs != null ? `${item.durationMs} ms` : ''].filter(Boolean).join(' · ');
  return [...headline(`${dateLabel(item.timestamp, reference, timezone, true, item.timePrecision)}  ${item.name}`, extras, width), ...([item.server, item.tool, item.path].filter(Boolean).length ? ['  ' + [item.server, item.tool, item.path].filter(Boolean).join(' · ')] : [])];
}
const categoryLabels: Record<string, string> = labels({ input: "common.uncached_input", cacheRead: "common.cache_read", cacheCreate: "common.cache_write", output: "common.output", reasoning: "common.reasoning_portion", cache_read: "common.cache_read", cache_create: "common.cache_write" });
export function summaryDetails(summary: UsageSummary): string[] {
  const categories = ['input', 'cacheRead', 'cacheCreate', 'output'] as const;
  return [...categories.map(category => { const component = summary.price.components.find(part => part.category === category || part.category === ({ cacheRead: 'cache_read', cacheCreate: 'cache_create' } as Record<string, string>)[category]); return `${categoryLabels[category]}  ${tokenSummaryText(summary, category)} Token · ${component?.status === 'partial' ? money(component.knownCost, 4) + '*' : component?.cost == null ? t("common.cost_unknown") : money(component.cost, 4)}`; }), t("cli.format.reasoning_portion_value_tokens", { p0: tokenSummaryText(summary, 'reasoning') })];
}
function qualityLine(result: UsageResult): string | undefined { return result.quality.status === 'partial' ? t("cli.format.data_status_value", { p0: result.quality.issues[0]?.message ?? t("cli.format.see_data_notes") }) : undefined; }
export function renderUsageResult(result: UsageResult, width = 120, group?: 'day' | 'week' | 'month'): string {
  if(result.statistics){const s=result.statistics,p=s.population;return [t('statistics.title'),t('statistics.population',{complete:p.completeTasks,total:p.measuredTasks}),`${t('statistics.mean')}: ${tokens(p.meanTokens)} · ${t('statistics.median')}: ${tokens(p.medianTokens)} · P90: ${tokens(p.p90Tokens)}`, ...(s.selectedTask?[t('statistics.selected',{rank:s.selectedTask.percentileRank==null?'—':(s.selectedTask.percentileRank*100).toFixed(1),tokens:tokens(s.selectedTask.tokens)})]:[]),...s.groups.map(g=>`${terminalText(g.key??t('webui.unknown'))} · ${g.population.measuredTasks} · ${tokens(g.population.meanTokens)} / ${tokens(g.population.medianTokens)} / ${tokens(g.population.p90Tokens)}`),...(s.growth?[t('statistics.growth',{count:s.growth.taskCountDelta,mean:tokens(s.growth.meanTokensDelta)}),t('statistics.contributions',{count:tokens(s.growth.taskCountContribution),intensity:tokens(s.growth.perTaskContribution)})]:[]),t('statistics.note')].join('\n')+'\n';}

  const title = { statistics:t('statistics.title'),context:t('inspection.context'),investigate:t('inspection.investigate'),trajectory:t('inspection.trajectory'),resources:t('inspection.resources'),review:t('inspection.review'),compare: t('comparison.title'), refresh: t("cli.format.updated"), usage: t("cli.format.usage"), threads: t("common.threads"), turns: t("cli.format.turns"), steps: t("cli.format.records") }[result.action];
  const lines = [`Wombat · ${title}`, t("common.updated_value", { p0: dateLabel(result.snapshotRef.createdAt, result.snapshotRef.createdAt, result.scope.timezone ?? 'UTC', true) }), rangeLabel(result.scope.since, result.scope.until, result.snapshotRef.createdAt, result.scope.timezone ?? 'UTC'), usageLabel(result.summary), ...tokenCoverageLines(result.summary, width), ''];
  if (result.inspection) lines.push(...inspectionLines(result));
  if (result.comparison) lines.push(...comparisonLines(result));
  if (result.freshness?.publicationChange) { const c=result.freshness.publicationChange; lines.push(t('comparison.refreshCounts',{added:c.measurementsAdded,changed:c.measurementsChanged,removed:c.measurementsRemoved,threads:c.threadsAdded,turns:c.turnsChanged}),t('comparison.refreshBasis',{prices:t(c.pricesChanged?'comparison.changed':'comparison.unchanged'),coverage:t(c.coverageChanged?'comparison.changed':'comparison.unchanged')})); }
  if (result.action === 'usage' && result.distribution && width >= 110)
    lines.push(usageTableHeader(width));
  for (const item of result.items)
    lines.push(...itemLines(item, result, width, group));
  if (!result.items.length && !result.comparison && !result.inspection)
    lines.push(result.action === 'refresh' ? t("cli.format.records_saved") : result.page.total > 0 ? t("cli.format.no_records_on_this_page_value", { p0: result.page.total }) : t("common.no_records"));
  if (result.items.length > 0 && result.page.total > result.items.length)
    lines.push(`${result.page.offset + 1}—${result.page.offset + result.items.length} / ${result.page.total}`);
  if (result.summary.price.issues.length) lines.push(...result.summary.price.issues.map(pricingIssueText));
  const priceUpdate = automaticPriceText(result);
  if (priceUpdate) lines.push('', priceUpdate);
  const quality = qualityLine(result);
  if (quality)
    lines.push('', quality);
  if (result.summary.price.status === 'partial')
    lines.push(t("cli.format.amount_is_the_priced_subtotal"));
  return lines.map(line => fit(line, width)).join('\n');
}
function wrapDisplay(text: string, width: number): string[] {
  const lines: string[] = [];
  let line = '';
  for (const { segment } of new Intl.Segmenter('zh-CN', { granularity: 'grapheme' }).segment(terminalText(text))) {
    if (line && stringWidth(line + segment) > width) { lines.push(line); line = ''; }
    line += segment;
  }
  return [...lines, line];
}

function comparisonLines(result:UsageResult):string[]{
 const c=result.comparison;if(!c)return [];
 const change=(d:NonNullable<UsageResult['comparison']>['delta'])=>`${t('comparison.delta')}: ${d.tokens==null?'—':(d.tokens>0?'+':'')+tokens(d.tokens)} Token · ${d.cost==null?'—':(d.cost.startsWith('-')?'':'+')+d.cost+' USD'}`;
 if(c.kind==='sessions')return [t(c.includeDescendants?'comparison.family':'comparison.sessions'),`${c.left.title??c.left.threadId}: ${usageLabel(c.left.selected)} (${c.left.memberCount})`,`${c.right.title??c.right.threadId}: ${usageLabel(c.right.selected)} (${c.right.memberCount})`,change(c.delta),...(c.left.partial||c.right.partial?[t('comparison.partial')]:[]),t('comparison.sessionNote')];
 return [`${t('comparison.baseline')}: ${c.baseline.scope.since} — ${c.baseline.scope.until} ${usageLabel(c.baseline.usage)}`,`${t('comparison.current')}: ${c.current.scope.since} — ${c.current.scope.until} ${usageLabel(c.current.usage)}`,change(c.delta),...c.drivers.map(d=>`${d.key??t('webui.unknown')}: ${change(d.delta)}`),`${t('comparison.remaining')}: ${change(c.remaining)}`,t('comparison.page',{offset:result.page.offset,total:result.page.total}),...(c.baseline.partial||c.current.partial?[t('comparison.partial')]:[]),t('comparison.note'),t('comparison.undated',{count:c.undatedRecords})];
}

function inspectionLines(result:UsageResult):string[] {
 const i=result.inspection!,lines=[t('inspection.note'),...i.limitations.map(l=>t(`inspection.limit.${l}`))];
 const p=i.policy;lines.push(t('inspection.policyText',{tokens:p.minimumTokens,input:p.minimumInput,cache:p.maximumCacheShare*100,jump:p.minimumInputJump,operations:p.minimumDeterminateOperations,failures:p.minimumFailures,share:p.minimumFailureShare*100,repeats:p.minimumRepeatedRequests}));
 if(i.activity){lines.push(t('inspection.activity.title'),...inspectionActivityText(i.activity),inspectionActivityPolicyText(i.activity.policy));for(const f of i.activity.findings){lines.push(f.tool,f.project??t('webui.unknown'),...inspectionActivityFindingText(f).flatMap(s=>[s.title,s.observed,s.advice]),t('inspection.activity.currentEvidence'),...f.evidence.map(e=>JSON.stringify(e)));if(f.baselineEvidence.length)lines.push(t('inspection.activity.baselineEvidence'),...f.baselineEvidence.map(e=>JSON.stringify(e)));}}
 if(i.opportunities){lines.push(t('inspection.opportunities.title'),t('inspection.opportunities.note'));for(const check of [...i.opportunities.checks].sort((a,b)=>Number(b.findingCount>0)-Number(a.findingCount>0))){const c=inspectionOpportunityCheckText(check);lines.push(c.title+' · '+c.status,...c.gaps,t('inspection.opportunities.count',{shown:check.findings.length,total:check.findingCount}));for(const f of check.findings){lines.push(f.object??'',...inspectionOpportunityFindingText(f),c.advice,...f.evidence.map(e=>JSON.stringify(e)));if(f.baselineEvidence.length)lines.push(t('inspection.activity.baselineEvidence'),...f.baselineEvidence.map(e=>JSON.stringify(e)));}}}
 for(const c of [...i.candidates,...(i.review?.topTasks??[])])lines.push(c.title??c.threadId,usageLabel(c.usage),...inspectionCandidateText(c).flatMap(finding=>[finding.title,finding.observed,finding.advice]),...c.evidence.map(e=>JSON.stringify(e)));
 if(i.kind==='investigate'&&!i.candidateCount&&!i.activity?.findingCount&&!i.opportunities?.checks.some(c=>c.findingCount))lines.push(t('inspection.noCandidates'));
 for(const p of i.trajectory)lines.push(`${p.timestamp??'—'} · ${t('inspection.input')}: ${p.input??'—'} · ${t('inspection.uncachedDelta')}: ${p.uncachedDelta??'—'} · ${p.boundary?t(`inspection.boundary.${p.boundary}`):''}`,JSON.stringify(p.evidence));
 for(const r of i.resources)lines.push(r.path,`${t('inspection.reads')}: ${r.reads} · ${t('inspection.proposed')}: ${r.proposedChanges} · ${t('inspection.reported')}: ${r.reportedChanges}`,`${t('inspection.duration')}: ${r.knownDurationMs??'—'} ms · ${t('inspection.coveredDuration',{count:r.durationCoveredOperations})}`,...r.evidence.map(e=>JSON.stringify(e)));
 if(i.context){lines.push(t('inspection.contextCount',{count:i.context.injectedRecords,windows:i.context.modelWindowRecords}));for(const r of i.context.records)lines.push(t(`inspection.contextKind.${r.kind}`),r.timestamp??'—',r.modelContextWindow!=null?t('inspection.modelWindow',{tokens:r.modelContextWindow}):t('inspection.contentUnknown'),JSON.stringify(r.evidence));}
 for(const point of i.trajectory)if(point.compactionComparison){const p=point.compactionComparison;lines.push(t('inspection.compactionComparison',{before:p.beforeInput,after:p.afterInput,difference:p.inputDifference}),t('inspection.compactionNote'),JSON.stringify(p.beforeEvidence));}
 if(i.review?.concentration){const c=i.review.concentration;const percent=(v:number|null|undefined)=>v==null?'—':new Intl.NumberFormat(locale.getLocale(),{style:'percent',maximumFractionDigits:1}).format(v);lines.push(t('inspection.concentration'),t('inspection.concentrationShares',{top:percent(c.topTaskShare),five:percent(c.topFiveShare),ten:percent(c.topTenShare)}),t('inspection.remainingTasks')+': '+usageLabel(c.remainingTaskUsage));}
 if(i.review){if(i.review.comparison)lines.push(...comparisonLines({...result,comparison:i.review.comparison}));for(const g of i.review.models)lines.push(g.key??t('webui.unknown'),usageLabel(g.usage));for(const tool of i.review.tools)lines.push(tool.kind+' · '+tool.operations);}
 lines.push(`${result.page.offset} / ${result.page.total}`);return lines;
}
