/** Presentation of published signals only; detection and statistics belong to Rust. */
import type {UsageResult} from '../index.js';
import {locale,t} from './index.js';

type Candidate=NonNullable<UsageResult['inspection']>['candidates'][number];
type Activity=NonNullable<NonNullable<UsageResult['inspection']>['activity']>;
type Finding=Activity['findings'][number];

export function inspectionActivityPolicyText(p:Activity['policy']):string{
 return t('inspection.activity.policyText',{slow:p.slowDurationMs,slowCount:p.minimumSlowOperations,current:p.minimumCurrentOutcomes,baseline:p.minimumBaselineOutcomes,failures:p.minimumSpikeFailures,share:p.minimumFailureShare*100,factor:p.failureShareMultiplier,floor:p.baselineFailureShareFloor*100,durations:p.minimumBaselineDurations,median:p.minimumBaselineMedianMs,durationFactor:p.durationMultiplier,increase:p.minimumDurationIncreaseMs,workflow:p.minimumWorkflowOperations,tasks:p.minimumWorkflowTasks,repeated:p.minimumFailuresInTask,rejections:p.minimumRejectionsInTask});
}

export function inspectionActivityText(activity:Activity):string[]{
 const c=activity.currentCoverage,b=activity.baselineScope;
 return [t('inspection.activity.coverage',{matched:c.matchedOperations,total:c.observedOperations,durations:c.durationSamples,gaps:c.outcomeGaps}),
  b?t('inspection.activity.baseline',{since:b.since!,until:b.until!}):t('inspection.activity.noBaseline'),
  ...(activity.baselineCoverage?[t('inspection.activity.baselineCoverage',{matched:activity.baselineCoverage.matchedOperations,total:activity.baselineCoverage.observedOperations,durations:activity.baselineCoverage.durationSamples,gaps:activity.baselineCoverage.outcomeGaps})]:[]),
  t('inspection.activity.count',{shown:activity.findings.length,total:activity.findingCount}),
  t('inspection.activity.note')];
}

export function inspectionActivityFindingText(finding:Finding):Array<{title:string;observed:string;advice:string}>{
 const c=finding.current,b=finding.baseline;
 const number=(v:number|null|undefined)=>v==null?t('webui.unknown'):new Intl.NumberFormat(locale.getLocale(),{maximumFractionDigits:1}).format(v);
 const share=(v:number|null|undefined)=>v==null?t('webui.unknown'):new Intl.NumberFormat(locale.getLocale(),{style:'percent',maximumFractionDigits:1}).format(v);
 return finding.signals.map(signal=>{
  let observed:string;
  switch(signal){
   case 'repeated_slow_request':observed=t('inspection.activity.observed.repeated_slow_request',{count:number(c.slowOperations),maximum:number(c.maximumDurationMs)});break;
   case 'failure_spike':observed=t('inspection.activity.observed.failure_spike',{failed:number(c.failedOperations),total:number(c.determinateOperations),share:share(c.failureShare),baseline:share(b?.failureShare),samples:number(b?.determinateOperations)});break;
   case 'duration_spike':observed=t('inspection.activity.observed.duration_spike',{maximum:number(c.maximumDurationMs),median:number(b?.medianDurationMs),samples:number(b?.durationSamples)});break;
   case 'recurring_workflow':observed=t('inspection.activity.observed.recurring_workflow',{count:number(c.operations),tasks:number(c.tasks)});break;
   case 'repeated_failure':observed=t('inspection.activity.observed.repeated_failure',{count:number(c.maximumFailuresInTask)});break;
   case 'repeated_rejection':observed=t('inspection.activity.observed.repeated_rejection',{count:number(c.maximumRejectionsInTask)});break;
  }
  return {title:t(`inspection.activity.signal.${signal}`),observed,advice:t(`inspection.activity.advice.${signal}`)};
 });
}

export function inspectionCandidateText(candidate:Pick<Candidate,'signals'|'cacheShare'|'largestUncachedJump'|'failedOperations'|'determinateOperations'|'repeatedRequests'>):Array<{title:string;observed:string;advice:string}>{
 const number=(value:number|null|undefined)=>value==null?t('webui.unknown'):new Intl.NumberFormat(locale.getLocale()).format(value);
 return candidate.signals.map(signal=>{
  let observed:string,advice:string;
  switch(signal){
   case 'high_usage':
    observed=t('inspection.observed.high_usage');advice=t('inspection.advice.high_usage');break;
   case 'low_cache_reuse':
    observed=t('inspection.observed.low_cache_reuse',{share:candidate.cacheShare==null?t('webui.unknown'):new Intl.NumberFormat(locale.getLocale(),{style:'percent',maximumFractionDigits:1}).format(candidate.cacheShare)});
    advice=t('inspection.advice.low_cache_reuse');break;
   case 'input_jump':
    observed=t('inspection.observed.input_jump',{tokens:number(candidate.largestUncachedJump)});advice=t('inspection.advice.input_jump');break;
   case 'failure_share':
    observed=t('inspection.observed.failure_share',{failed:number(candidate.failedOperations),total:number(candidate.determinateOperations)});advice=t('inspection.advice.failure_share');break;
   case 'repeated_request':
    observed=t('inspection.observed.repeated_request',{count:number(candidate.repeatedRequests)});advice=t('inspection.advice.repeated_request');break;
  }
  return {title:t(`inspection.signal.${signal}`),observed,advice};
 });
}

type Opportunities=NonNullable<NonNullable<UsageResult['inspection']>['opportunities']>;
type OpportunityCheck=Opportunities['checks'][number];
export function inspectionOpportunityCheckText(check:OpportunityCheck):{title:string;status:string;gaps:string[];advice:string}{
 return {title:t(`inspection.opportunities.rule.${check.rule}`),status:t(`inspection.opportunities.status.${check.status}`),gaps:check.gaps.map(g=>t(`inspection.opportunities.gap.${g}`)),advice:t(`inspection.opportunities.advice.${check.rule}`)};
}
export function inspectionOpportunityFindingText(finding:OpportunityCheck['findings'][number]):string[]{
 return [...finding.safetyLabels.map(label=>t(`inspection.opportunities.label.${label}`)),...finding.metrics.map(m=>{const unit=m.unit==='usd'?'USD':m.unit==='milliseconds'?'ms':m.unit==='token'?'Token':t(`inspection.opportunities.unit.${m.unit}`);return `${t(`inspection.opportunities.metric.${m.name}`)}: ${m.value??t('webui.unknown')} ${unit}`;})];
}
export function inspectionOpportunityPolicyText(policy:Opportunities['policy']):string[]{
 return (Object.keys(policy) as Array<keyof typeof policy>).map(key=>`${t(`inspection.opportunities.policy.${key}`)}: ${policy[key]}`);
}
