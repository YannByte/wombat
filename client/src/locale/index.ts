import {operationOutcomeText} from './outcome-statistics.js';
import {timingMissingValueText} from './timing-basis.js';
/** Shared presentation-only locale service. Never localize protocol values or source content. */
import { zh } from './zh.js';
import { en } from './en.js';
export type Locale = 'zh' | 'en';
export type MessageKey = keyof typeof zh;
type Slots<S extends string> = S extends `${string}{${infer P}}${infer Rest}` ? P | Slots<Rest> : never;
type Arguments<K extends MessageKey> = [Slots<(typeof zh)[K]>] extends [never]
  ? [] : [params: Record<Slots<(typeof zh)[K]>, string | number>];
export type Translate = <K extends MessageKey>(key: K, ...args: Arguments<K>) => string;
const dictionaries = { zh, en };
const plurals: Partial<Record<Locale, Intl.PluralRules>> = {};
export function matchLocale(value: string | undefined): Locale | undefined {
  const language = value?.trim().toLowerCase().split(/[-_.@]/)[0];
  return language === 'zh' || language === 'en' ? language : undefined;
}
/** Explicit CLI choice, environment choice, ordered system languages, then Chinese default. */
export function resolveLocale(options: { explicit?: string; environment?: string; languages?: readonly string[] } = {}): Locale {
  for (const value of [options.explicit, options.environment]) {
    if (value !== undefined) {
      const matched = matchLocale(value);
      if (!matched) throw new RangeError(`Unsupported language: ${value}; expected zh or en`);
      return matched;
    }
  }
  for (const value of options.languages ?? []) {
    const matched = matchLocale(value);
    if (matched) return matched;
  }
  return 'zh';
}
export class LocaleRuntime {
  private snapshot: Readonly<{ locale: Locale; revision: number }>;
  private listeners = new Set<() => void>();
  constructor(locale: Locale = 'zh') { this.snapshot = Object.freeze({ locale, revision: 0 }); }
  getSnapshot = () => this.snapshot;
  subscribe = (listener: () => void): (() => void) => {
    this.listeners.add(listener);
    return () => { this.listeners.delete(listener); };
  };
  setLocale(locale: Locale): void {
    if (locale !== 'zh' && locale !== 'en') throw new RangeError(`Unsupported language: ${locale}`);
    if (locale === this.snapshot.locale) return;
    this.snapshot = Object.freeze({ locale, revision: this.snapshot.revision + 1 });
    // Complete notification even if one subscriber fails; report the first failure to the caller.
    let failure: unknown;
    for (const listener of [...this.listeners]) { try { listener(); } catch (error) { failure ??= error; } }
    if (failure !== undefined) throw failure;
  }
  /** Stable function identity; reads the active locale on every invocation. */
  t: Translate = (key, ...args) => {
    const params = args[0] as Record<string, string | number> | undefined;
    const language = this.snapshot.locale;
    const dictionary: Readonly<Record<string, string>> = dictionaries[language];
    // Full-sentence variants keep interpolation literal and unknown counts distinct.
    const variant = typeof params?.count === 'number' ? `${key}.${(plurals[language] ??= new Intl.PluralRules(language === 'zh' ? 'zh-CN' : 'en-US')).select(params.count)}` : key;
    const template = dictionary[variant] ?? dictionary[key] ?? en[key] ?? key;
    return template.replace(/\{(\w+)\}/g, (match, name: string) => params && Object.hasOwn(params, name) ? String(params[name]) : match);
  };
}
/** One presentation session per CLI process; independent runtimes are available to other hosts. */
export const locale = new LocaleRuntime();
export const t = locale.t;
/** Presentation numbers follow the active language, independent of the host locale. */
export function numberLabel(value: number | null | undefined, options?: Intl.NumberFormatOptions): string {
  return value == null ? '—' : new Intl.NumberFormat(locale.getSnapshot().locale === 'zh' ? 'zh-CN' : 'en-US', options).format(value);
}
export function compactNumberLabel(value: number | null | undefined): string {
  return numberLabel(value, { notation: 'compact', maximumFractionDigits: 2 });
}
/** Two independent counts require independent agreement in the complete sentence. */
export function relatedActivityText(turns: number | string, count: number | string): string {
  return t(turns === 1 ? 'optimize.tasks.turnOne' : 'optimize.tasks', { turns, count });
}
/** Format calendar buckets without applying the host time zone to date-only evidence. */
export function monthLabel(value: string): string {
  if (!/^\d{4}-(0[1-9]|1[0-2])(?:-\d{2})?$/.test(value)) return value;
  const language = locale.getSnapshot().locale;
  const date = new Date(`${value.slice(0, 7)}-01T00:00:00Z`);
  const month = language === 'zh' ? Number(value.slice(5, 7)) : new Intl.DateTimeFormat('en-US', { month: 'short', timeZone: 'UTC' }).format(date);
  return t('date.month', { year: value.slice(0, 4), month });
}
/** Compact display only; callers retain exact bytes for sorting and evidence. */
export function bytesLabel(value: number | null | undefined): string {
  if (value == null) return '—';
  if (value > 0 && value < 10.24) return '<0.01 KiB';
  return numberLabel(value / 1024, { maximumFractionDigits: 2 }) + ' KiB';
}
/** One finding supplies the headline and key metric; all findings remain in the detail. */
export function reviewPresentation(s: import('../generated/optimize-response.js').Suggestion): {title:string;value:string;metric:number|null|undefined;label:string;metricText?:string} {
  const finding = ['skillFormat','hookTarget','localReference','descriptionStandard','declaredCopyDrift','exactInstructionBlocks','descriptionSize','bodyTokens','fileSize','missingInstruction','skillInactivity','mcpInactivity']
    .map(rule => s.findings.find(f => f.rule === rule)).find(Boolean);
  const name = s.item.name;
  switch (finding?.rule) {
    case 'missingInstruction':return {title:t('optimize.missingInstruction'),value:t('optimize.missingValue'),metric:undefined,label:t('webui.unknown')};
    case 'descriptionStandard': case 'descriptionSize': return {title:t('optimize.descriptionTitle',{name}),value:t(finding.rule==='descriptionStandard'?'optimize.standardValue':'optimize.descriptionValue'),metric:finding.observed,label:t('config.descriptionCharacters')};
    case 'skillFormat': return {title:t('optimize.formatTitle',{name}),value:t('optimize.formatValue'),metric:finding.evidenceCodes.length || undefined,label:t('optimize.formatErrors'),metricText:finding.evidenceCodes.length?t('optimize.formatCountFull',{count:finding.evidenceCodes.length}):undefined};
    case 'localReference':return {title:t('optimize.referenceTitle',{name}),value:t('optimize.referenceValue'),metric:finding.observed,label:t('optimize.referenceCount'),metricText:finding.observed==null?undefined:t('optimize.referenceCountFull',{count:finding.observed})};
    case 'bodyTokens': return {title:t('optimize.bodyTitle',{name}),value:t('optimize.bodyValue'),metric:finding.observed,label:t('config.bodyTokens')};
    case 'declaredCopyDrift': {const count=s.findings.filter(f=>f.rule==='declaredCopyDrift').length;return {title:t('optimize.copyTitle',{name}),value:t('optimize.copyValue'),metric:count,label:t('optimize.copyDrift'),metricText:t('optimize.copyCountFull',{count})};}
    case 'skillInactivity':case 'mcpInactivity':return {title:t('optimize.idleTitle',{name}),value:t('optimize.idleValue'),metric:finding.observed,label:t('optimize.organize'),metricText:finding.observed==null?undefined:t('optimize.idleDays',{count:finding.observed})};
    case 'exactInstructionBlocks': return {title:t('optimize.blocksTitle',{name}),value:t('optimize.blocksValue'),metric:finding.observed,label:t('optimize.blockPositions')};
    case 'hookTarget': return {title:t('optimize.hookTargetTitle',{name}),value:t('optimize.hookTargetValue'),metric:undefined,label:t('optimize.repair')};
    case 'fileSize': return {title:t('optimize.fileTitle',{name}),value:t('optimize.fileValue'),metric:finding.observed,label:t('config.size')+' · B'};
    default: return {title:t('optimize.genericTitle',{name}),value:t('optimize.genericValue'),metric:undefined,label:t('webui.unknown')};
  }
}
export function reviewFindingLabel(rule:string):string {
  const messages = labels({localReference:'optimize.localReference',instructionSelection:'optimize.instructionSelection',skillDependency:'optimize.skillDependency',hookTarget:'optimize.hookTarget',runtimeDuplicateInjection:'optimize.runtimeDuplicateInjection',skillInactivity:'optimize.skillInactivity',mcpInactivity:'optimize.mcpInactivity',mcpFault:'optimize.mcpFault'});
  if(Object.hasOwn(messages,rule))return messages[rule as keyof typeof messages];
  if(rule==='missingInstruction')return t('optimize.missingInstruction');
  switch(rule){case 'exactInstructionBlocks':return t('optimize.exactBlocks');case 'declaredCopyDrift':return t('optimize.copyDrift');case 'fileSize':return t('optimize.fileSize');case 'descriptionSize':return t('optimize.descriptionSize');case 'descriptionStandard':return t('optimize.descriptionStandard');case 'bodyTokens':return t('config.bodyTokens');case 'skillFormat':return t('optimize.skillFormat');default:return t('optimize.genericRule');}
}
export function reviewFindingNote(rule:string,project?:string):string {
  if(rule==='skillInactivity')return t('optimize.idleSkillNote');
  if(rule==='mcpInactivity')return project?t('optimize.idleMcpNote',{project}):t('optimize.idleMcpScope');
  if(rule==='hookTarget')return t('optimize.hookTargetNote');
  if(rule==='localReference')return t('optimize.referenceValue');
  if(rule==='missingInstruction')return t('optimize.missingValue');
  switch(rule){case 'exactInstructionBlocks':return t('optimize.blocksValue');case 'declaredCopyDrift':return t('optimize.copyValue');case 'fileSize':return t('optimize.agentsReminder');case 'descriptionSize':return t('optimize.descriptionReminder');case 'descriptionStandard':return t('optimize.descriptionStandardNote');case 'bodyTokens':return t('config.bodyNote');default:return t('optimize.formatNote');}
}
export function reviewFindingCount(f: import('../generated/optimize-response.js').Finding): string | undefined {
  if(f.rule==='declaredCopyDrift')return t('optimize.copyCountFull',{count:1});
  if(f.rule==='skillFormat'&&f.evidenceCodes.length)return t('optimize.formatCountFull',{count:f.evidenceCodes.length});
  if(f.rule==='localReference'&&f.observed!=null)return t('optimize.referenceCountFull',{count:f.observed});
  return undefined;
}
/** Locale-sensitive labels in module-level maps remain live across switches. */
type PlainMessageKey = { [K in MessageKey]: Slots<(typeof zh)[K]> extends never ? K : never }[MessageKey];
export function labels<K extends string>(keys: Record<K, PlainMessageKey>): Record<K, string> {
  return Object.defineProperties({}, Object.fromEntries(Object.entries(keys).map(([name, key]) => [name, {
    enumerable: true, get: () => dictionaries[locale.getSnapshot().locale][key as MessageKey],
  }]))) as Record<K, string>;
}

/** Translate known core and host progress signals; retain unknown diagnostics verbatim. */
export function progressText(stage: string): string {
  const key = (['progress.read_logs', 'progress.save_usage', 'progress.sync', 'progress.fetch_prices', 'progress.save_prices'] as const).find(key => zh[key] === stage);
  return key ? t(key) : stage;
}

/** Automatic price checks describe download state separately from log freshness. */
export function automaticPriceText(result: import('../generated/usage-app.js').Response): string | undefined {
  const state = result.priceUpdate;
  if (!state) return undefined;
  if (state.status === 'checking') return t('prices.auto.checking');
  if (state.status === 'failed') return t('prices.auto.failed', { code: state.errorCode ?? 'PRICE_UPDATE_FAILED' });
  return result.summary.price.issues.includes('catalogPriceMissing') ? t('prices.auto.unresolved') : t('prices.auto.updated');
}

export function eventStatusLabel(status: string): string {
  switch(status){case 'completed':case 'succeeded':return t('event.completed');case 'observed':return t('config.skillUsed');case 'failed':return t('event.failed');case 'cancelled':case 'canceled':return t('event.cancelled');case 'running':return t('common.running');case 'interrupted':return t('common.interrupted');case 'unknown':return t('webui.unknown');default:return status;}
}
export function followUpText(observation: import('../client.js').OptimizeResult['followUps'][number]): string {
  if(observation.status!=='read_content_matched'&&observation.status!=='loaded_content_matched'&&observation.useBasis?.status==='partial'&&observation.observedRecords!=null)return t('optimize.followUp.partialRecords',{count:observation.observedRecords});
  switch(observation.status){
    case 'no_observed_records':return t('optimize.followUp.noRecords');
    case 'loaded_content_matched':return t('optimize.followUp.loadedContentMatched',{count:observation.matchingLoadRecords??0});
    case 'read_content_matched':return t('optimize.followUp.readContentMatched',{count:observation.matchingReadRecords??0});
    case 'version_unknown':return observation.observedRecords!=null&&observation.observedRecords>0?t('optimize.followUp.versionUnknown',{count:observation.observedRecords}):t('optimize.followUp.unavailable');
    case 'unavailable':return t('optimize.followUp.unavailable');
  }
}
export function operationTypeLabel(kind: string): string {
  switch(kind){case 'instructionLoad':return t('event.instructionLoad');case 'skillCatalog':return t('event.skillCatalog');case 'skillAvailable':return t('event.skillAvailable');case 'skillUse':return t('event.skillUse');case 'skillRead':return t('event.skillRead');case 'mcp':case 'mcpTool':return t('event.mcp');case 'mcpResource':return t('event.mcpResource');case 'mcpDiscovery':return t('event.mcpDiscovery');case 'mcpUnclassified':return t('event.mcpUnclassified');case 'mcpConflict':return t('event.mcpConflict');case 'tool':return t('event.tool');default:return kind;}
}

export function storageFailureText(code?:string):string|undefined {switch(code){case 'STORAGE_FULL':return t('webui.storageFull');case 'STORAGE_UNAVAILABLE':return t('webui.storageUnavailable');case 'INDEX_UNSUPPORTED_VERSION':return t('webui.indexUnsupported');case 'INDEX_UNAVAILABLE':return t('webui.indexUnavailable');default:return undefined;}}

export { observedCount, inventoryRecordState, sourceReadLabel } from "./observations.js";

export function configEvidenceLabel(type: string): string {
  switch(type){case 'instruction_load':return t('config.instruction_load');case 'skill_available':return t('config.skill_available');case 'skill_use':return t('config.skill_use');case 'file_read':return t('config.file_read');case 'tool_call':return t('config.tool_call');case 'resource_read':return t('config.resource_read');default:return t('webui.unknown');}
}

export { useBasisCount, useBasisPresentation, type PublicUseBasis } from "./use-basis.js";

/** Explain core assessment reason codes without interpreting unknown source details. */
export function assessmentReason(code: string): string {
  switch (code) {
    case 'verifiedHostAdapterUnavailable': return t('optimize.hostEvidenceMissing');
    case 'continuousCoverageUnavailable': return t('optimize.coverageMissing');
    case 'runtimeInjectionUnavailable': return t('optimize.injectionMissing');
    case 'copyRelationNotDeclared': return t('optimize.copyUndeclared');
    case 'ruleParametersOrMethodChanged': return t('optimize.assessment.reason.methodChanged');
    case 'assessmentScopeChanged': return t('optimize.assessment.reason.scopeChanged');
    case 'baselineAssessmentUnavailable': return t('optimize.assessment.reason.baselineMissing');
    case 'assessmentIdentityUnavailable': case 'problemIdentityUnavailable':
    case 'objectIdentityUnavailable': case 'problemLocationContextUnavailable':
    case 'reliableProblemIdentityUnavailable': case 'decisionApplicabilityUnavailable': return t('optimize.assessment.reason.identityMissing');
    case 'dependencyIdentityBudgetExceeded': case 'scopeIdentityBudgetExceeded':
    case 'assessmentIdentityBudgetExceeded': return t('optimize.assessment.reason.resourceLimit');
    case 'checkScopeUnavailable': return t('optimize.assessment.reason.scopeMissing');
    case 'currentVersionUnavailable': return t('optimize.assessment.reason.versionMissing');
    case 'analysisUnavailable': return t('optimize.assessment.reason.analysisMissing');
    case 'invalidAnalysisEvidence': return t('optimize.assessment.reason.analysisInvalid');
    default: return t('optimize.assessment.reason.evidenceMissing');
  }
}

export { timingBasisText, timingMissingValueText, timingSourceStatusText } from './timing-basis.js';
export { pricingIssueText } from './pricing.js';

/** Display the recorded review state without deriving it from decisions or measurements. */
export function reviewStatusLabel(status: string): string {
  switch (status) {
    case 'pending': return t('optimize.pending');
    case 'verified': return t('optimize.verified');
    case 'stillNeedsReview': return t('optimize.stillNeedsReview');
    case 'recheckUnavailable': return t('optimize.recheckUnavailable');
    default: return t('optimize.statusUnavailable');
  }
}

export { timingCategories, timingCategoryText, timingIntersectionText } from './timing-categories.js';

export { tokenSummaryPresentation, tokenSummaryText, analyzedTokenSubtotal, type SummaryTokenField } from './token-summary.js';

export {operationCoverageReasonText} from './operation-coverage.js';

export {repeatedBehaviorReasonText} from './repeated-behavior.js';

type ActivityCheck=NonNullable<import('../generated/optimize-response.js').Response['activity']>['checks'][number];
type ActivityRule=ActivityCheck['rule'];
export {inspectionCandidateText,inspectionActivityText,inspectionActivityFindingText,inspectionActivityPolicyText,inspectionOpportunityCheckText,inspectionOpportunityFindingText,inspectionOpportunityPolicyText} from './inspection.js';
export function activityRuleTitle(rule:ActivityRule):string {
  switch(rule){case 'inspect_calls_after_failure':return t('activity.failure');case 'inspect_repeated_reads':return t('activity.read');case 'inspect_repeated_requests':return t('activity.request');case 'inspect_failure_share':return t('activity.failureShare');case 'inspect_input_change':return t('inputChange.title');}
}
export function activityAdviceText(rule:ActivityRule):string {
  switch(rule){case 'inspect_calls_after_failure':return t('activity.failureAdvice');case 'inspect_repeated_reads':return t('activity.readAdvice');case 'inspect_repeated_requests':return t('activity.requestAdvice');case 'inspect_failure_share':return t('activity.failureShareAdvice');case 'inspect_input_change':return t('activity.inputChangeAdvice');}
}
/** Concrete headline from the published check, without choosing thresholds or inferring causes. */
export function activityFindingTitle(check:ActivityCheck):string {
 if(check.outcome!=='hit'||check.observed.value==null)return activityRuleTitle(check.rule);
 const count=check.observed.value;
 switch(check.rule){
  case 'inspect_calls_after_failure':return t('activity.finding.failure',{count});
  case 'inspect_repeated_reads':return t('activity.finding.read',{count});
  case 'inspect_repeated_requests':return t('activity.finding.request',{count});
  case 'inspect_failure_share':return check.outcomes?operationOutcomeText(check.outcomes).headline:activityRuleTitle(check.rule);
  case 'inspect_input_change':return t('activity.finding.input',{tokens:count});
 }
}
export function activityCheckText(check:ActivityCheck):string {
  if(check.rule==='inspect_input_change'&&check.inputChange&&check.inputPolicy){
    const text=inputChangeText(check.inputChange);
    return [text.headline,t('activity.inputChangePolicy',{tokens:check.inputPolicy.minimumIncrease}),
      check.outcome==='miss'?t('activity.belowThreshold'):'',check.partial?t('activity.partial'):''].filter(Boolean).join(' ');
  }
  if(check.rule==='inspect_failure_share'&&check.outcomes&&check.failurePolicy){
    const policy=check.failurePolicy;
    return [operationOutcomeText(check.outcomes).headline,t('activity.failureSharePolicy',{minimum:policy.minimumDeterminate,failures:policy.minimumFailures,ratio:new Intl.NumberFormat(locale.getSnapshot().locale,{style:'percent',maximumFractionDigits:1}).format(policy.minimumRatio)}),
      check.reason==='activitySampleTooSmall'?t('activity.sampleSmall'):check.reason==='activityMeasureUnavailable'?timingMissingValueText(check.observed.basis):check.outcome==='miss'?t('activity.belowThreshold'):'',check.partial?t('execution.outcomes.partial'):''].filter(Boolean).join(' ');
  }
  if(check.outcome==='hit'&&check.observed.value!=null)return t('activity.observed',{count:check.observed.value})+(check.partial?` · ${t('activity.partial')}`:'');
  if(check.outcome==='miss')return t('activity.miss');
  if(check.reason==='activityCoverageIncomplete')return t('activity.coverage');
  if(check.reason==='activityBasisUnsupported')return t('activity.unsupported');
  return timingMissingValueText(check.observed.basis);
}

export {operationOutcomeText} from './outcome-statistics.js';

export function inputChangeText(change: import('../generated/timing-local-response.js').Context['inputChange']): {headline:string;details:string[];note:string} {
  const s=change.statistics;
  if(!s)return {headline:timingMissingValueText(change.availability.reason),details:[],note:t('inputChange.note')};
  const headline=s.maximumIncrease.value==null?t('inputChange.insufficient'):t('inputChange.maximum',{tokens:s.maximumIncrease.value});
  const details=[t('inputChange.coverage',{stages:s.comparableStages,increasing:s.increasingStages,decreasing:s.decreasingStages,unchanged:s.unchangedStages})];
  if(s.largestIncrease)details.push(t('inputChange.endpoints',{samples:s.largestIncrease.samples,first:s.largestIncrease.firstInput,last:s.largestIncrease.lastInput}));
  if(s.nonRequestScoped+s.missingInput+s.unassociated+s.numericRange>0)details.push(t('inputChange.exclusions',{interval:s.nonRequestScoped,missing:s.missingInput,unassociated:s.unassociated,numeric:s.numericRange}));
  if(s.detailsOmitted)details.push(t('inputChange.detailsOmitted'));
  return {headline,details,note:t('inputChange.note')};
}
