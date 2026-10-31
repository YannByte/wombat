import test from 'node:test';
import assert from 'node:assert/strict';
import type { OptimizeResult, OptimizeSuggestion } from '@wombat/client';
import { locale, t } from '@wombat/client/locale';
import { formatOptimizeText } from '../src/optimize-cli.js';
import {activityResult} from '../../tests/fixtures/activity.js';

const at = '2026-10-01T00:00:00Z';
const item: OptimizeSuggestion['item'] = {
  id: 'synthetic-item', name: 'sample', kind: 'skill', sourceInstanceId: 'synthetic', path: '/synthetic/SKILL.md',
  authorizedProjects: [], sourceContexts: [], configuredState: 'discovered', contentHash: 'current-content', observedAt: at,
  current: true, stale: false, estimateStatus: 'unknown', measurementStatus: 'complete', bodyEstimateStatus: 'unknown',
  observation: 'unknown', counts: { fileReads: 0, toolCalls: 0, resourceReads: 0, succeeded: 0, failed: 0, outcomeUnknown: 0 },
  relatedTurns: 0, relatedTasks: 0,
};
const scope: OptimizeSuggestion['checks'][number]['basis']['scope'] = {
  sourceInstanceId: 'synthetic', itemProject: null, global: true, project: null, sourceInstances: ['synthetic'],
  authorizedProjects: [], roots: ['/synthetic'], projectRoots: [], sourceRoots: ['/synthetic'], complete: true,
};
function check(outcome: OptimizeSuggestion['checks'][number]['outcome'] = 'hit', comparison: OptimizeSuggestion['checks'][number]['comparison']['status'] = 'not_requested'): OptimizeSuggestion['checks'][number] {
  return {
    assessmentId: 'synthetic-assessment', identityGap: null, ruleSemanticsVersion: 1,
    methodVersions: [{ method: 'synthetic-measurement', version: 1 }], rule: 'bodyTokens', ruleVersion: 'synthetic-rules',
    itemId: item.id, contentVersion: item.contentHash, checkedAt: at, outcome, reason: null, findings: [],
    basis: { version: 1, dependencyRevision: 'synthetic-dependency', scope, cutoff: at, applicability: 'synthetic',
      measurement: { kind: 'numeric', basis: 'agentSkillsRecommendation', observed: 6000, threshold: 5000, inclusive: true, suppressedByStandard: false }, gaps: [] },
    comparison: { status: comparison, baselineAssessmentId: comparison === 'not_requested' ? null : 'original-assessment', reason: null },
  };
}
function result(): OptimizeResult {
  const current = check();
  return {
    outputVersion: 5, action: 'detail', capabilities: { staticChecks: true, manualEditReview: true, decisions: true, exactInstructionBlocks: true, declaredCopyDrift: true,
      hookSupport: { effectiveRegistry: false, status: 'no_verified_adapter' } },
    configRevision: 'synthetic-config', usageRevision: null, readView: 'synthetic-fixed-view', decisionRevision: 'synthetic-decision', checkedAt: at,
    suggestions: [{ reviewFormatVersion: 1, id: 'synthetic-suggestion', item: structuredClone(item), category: 'trim',
      checks: [current], findings: [], checkedAt: at, ruleVersion: 'synthetic-rules',
      baselineId: "synthetic-baseline" }],
    pending: 1, page: { offset: 0, limit: 50, total: 1 }, issues: [], resultStatus: 'complete',
    ruleParameters: { version: 'synthetic-rules', agentsBytesDefault: 16384, descriptionCharactersDefault: 500, overrides: {},
      bodyTokens: 5000, descriptionStandardMax: 1024, applicability: 'synthetic' }, ruleCatalog: [], checks: [],
  };
}
function bilingual(run: () => void) {
  const saved = locale.getLocale();
  try { for (const language of ['zh', 'en'] as const) { locale.setLocale(language); run(); } }
  finally { locale.setLocale(saved); }
}

test('activity text leads with measured findings and next steps while retaining every check',()=>bilingual(()=>{
 const fixture=activityResult(),text=formatOptimizeText(fixture);
 assert.match(text,locale.getLocale()==='zh'?/失败后再次调用 1 次/:/Calls after failure: 1/);
 const basis=text.indexOf(t('activity.checks'));
 assert.ok(basis>0);assert.ok(text.indexOf(t('activity.failureAdvice'))<basis);
 assert.ok(text.indexOf(t('activity.sampleSmall'))>basis);
 assert.match(text,/live:synthetic:fixed/);assert.match(text,/safe_event_turn_v7/);
}));

test('activity CLI requires a fixed turn and renders inspection signals in both languages',async()=>{
 const {parseOptimizeArgs}=await import('../src/optimize-cli.js');
 const {activityResult}=await import('../../tests/fixtures/activity.js');
 const parsed=parseOptimizeArgs(['activity','--snapshot','live:synthetic:fixed','--thread','task','--turn','turn','--json']);
 assert.equal(parsed.request.action,'activity');assert.equal(parsed.request.projectRoots,undefined);
 assert.deepEqual(parsed.request.activity,{snapshotId:'live:synthetic:fixed',threadId:'task',turnId:'turn'});
 for(const args of [['activity'],['activity','--snapshot','fixed','--thread','task'],['activity','--snapshot','fixed','--thread','task','--turn','turn','--read-view','config:other'],['list','--turn','turn']])assert.throws(()=>parseOptimizeArgs(args));
 const previous=locale.getLocale();
 try{for(const language of ['en','zh'] as const){locale.setLocale(language);const text=formatOptimizeText(activityResult());assert.match(text,language==='en'?/Calls after failure: 1 observed/:/失败后再次调用: 已观察到 1 次/);assert.match(text,/40%/);assert.match(text,language==='en'?/at least 5 operations/:/至少 5 次结果可判定/);assert.match(text,language==='en'?/current sample is small/:/当前样本较少/);assert.match(text,language==='en'?/same path does not establish identical content/:/同路径不证明内容/);assert.doesNotMatch(text,/activity\.|undefined|NaN|未知/);}}finally{locale.setLocale(previous);}
});

test('check text preserves outcomes, comparison gaps, exact zero and source-text safety',()=>bilingual(()=>{
 const fixture=result();fixture.action='recheck';fixture.checks=(['hit','miss','insufficient','unsupported','error'] as const).map(o=>check(o,'unknown'));fixture.suggestions[0].item.path='file\u001b[31m\npath';
 const text=formatOptimizeText(fixture);for(const o of ['hit','miss','insufficient','unsupported','error'] as const)assert.ok(text.includes(t(`optimize.check.${o}`)));assert.ok(text.includes(t('optimize.assessment.comparison.unknown')));assert.doesNotMatch(text,/\u001b|undefined|NaN/);
 fixture.checks=[{...check('miss','incomparable'),comparison:{status:'incomparable',reason:'ruleParametersOrMethodChanged'}}];assert.ok(formatOptimizeText(fixture).includes(t('optimize.assessment.comparison.incomparable')));
}));
test('formatting changes language while preserving facts and the opaque recheck baseline',()=>{const fixture=result(),before=JSON.stringify(fixture),saved=locale.getLocale();try{locale.setLocale('en');const en=formatOptimizeText(fixture);locale.setLocale('zh');const zh=formatOptimizeText(fixture);assert.notEqual(en,zh);assert.match(en,/synthetic-baseline/);assert.match(zh,/synthetic-baseline/);assert.equal(JSON.stringify(fixture),before);}finally{locale.setLocale(saved);}});
test('rechecks require original view and baseline; removed lifecycle actions reject',async()=>{const {parseOptimizeArgs}=await import('../src/optimize-cli.js');for(const a of [['recheck'],['recheck','--read-view','view'],['history'],['redisplay'],['handoff']])assert.throws(()=>parseOptimizeArgs(a));const r=parseOptimizeArgs(['recheck','--read-view','view','--baseline','baseline']);assert.equal(r.request.baselineId,'baseline');});
