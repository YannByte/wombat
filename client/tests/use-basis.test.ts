import { test } from 'node:test';
import assert from 'node:assert/strict';
import { locale, useBasisCount, useBasisPresentation, type PublicUseBasis } from '../src/locale/index.js';
export const basis: PublicUseBasis = { methodVersion: 3, status: 'partial', unit: 'object_use', capturedAt: '2026-10-02T00:00:00Z', snapshotId: 'live:fixed', scope: { sourceInstanceIds: ['synthetic-source'], project: '/synthetic/project', threadId: 'task', agentKind: 'codex', window: { kind: 'date_window', since: '2026-09-01', until: '2026-10-01', timezone: 'UTC' } }, timeBasis: 'source_operation_time', coverage: { dispatchGaps: 1, identityGaps: 2, targetGaps: 3, timeGaps: 4, turnGaps: 5 }, sourceCompleteness: 'partial' };
test('public basis explains the fixed scope and five gaps in both languages without claiming dispatch or absence', () => {
  const saved = locale.getLocale();
  try { for (const language of ['zh', 'en'] as const) {
    locale.setLocale(language);
    const value = useBasisPresentation(basis), text = [value.summary, ...value.notes, ...value.details].join('\n');
    assert.match(text, /完整次数更高|full count higher/); assert.match(text, /不等同于派发|not necessarily the dispatch/);
    assert.ok(text.includes('synthetic-source')); assert.ok(text.includes('/synthetic/project'));
    assert.ok(text.includes('2026-09-01')); assert.ok(text.includes('2026-10-01')); assert.ok(text.includes('live:fixed'));
    for (const n of [1, 2, 3, 4, 5]) assert.ok(value.details.some(line => line.endsWith(`: ${n}`) || line.endsWith(`：${n}`)));
    assert.doesNotMatch(text, /\{"|dispatchGaps|source_instance/);
    const unavailable = useBasisPresentation({ ...basis, status: 'unavailable', coverage: {} });
    assert.ok(unavailable.details.slice(-5).every(line => /未提供|Not provided/.test(line)));
    assert.doesNotMatch(unavailable.details.slice(-5).join(' '), /: 0|：0/);
    assert.match(useBasisPresentation(null).summary, /没有|No fixed/);
  } } finally { locale.setLocale(saved); }
});

test('generated config validators accept observed, partial, unavailable and null bases and reject untyped time claims', async () => {
  const { validate: config } = await import('../src/generated/validate-config-response.js');
  const item = { id:'item',name:'synthetic',kind:'skill',sourceInstanceId:'synthetic-source',path:'/synthetic/SKILL.md',authorizedProjects:[],sourceContexts:[],configuredState:'enabled',contentHash:'synthetic',observedAt:basis.capturedAt,current:true,stale:false,estimateStatus:'unknown',measurementStatus:'unknown',bodyEstimateStatus:'unknown',observation:'unknown',counts:{fileReads:0,toolCalls:0,resourceReads:0,succeeded:0,failed:0,outcomeUnknown:0},relatedTurns:0,relatedTasks:0 };
  const configBase = { outputVersion:2,action:'list',capabilities:{kinds:['skill'],evidenceTypes:[],tokenEstimates:false,historicalContent:false,historicalContentHashes:true,writes:false,projectRegistry:false},configRevision:'synthetic',checkedAt:basis.capturedAt,scope:{},authorizedProjects:[],summary:{currentItems:1,historicalItems:0,observedItems:0},items:[],evidence:[],relatedScopes:[],page:{offset:0,limit:1,total:1},coverage:{status:'partial',historyStatus:'fixed',issues:[],supportedEvidence:[],absenceObservable:false},hookRegistry:{status:'unavailable',contexts:[]} };
  for(const useBasis of [basis,{...basis,status:'observed'},{...basis,status:'unavailable'},null]){
    assert.ok(config({...configBase,items:[{...item,usageCount:useBasis?.status==='partial'?2:null,useBasis}]}));
  }
  for(const useBasis of [{...basis,status:'unknown'}, {...basis,scope:{...basis.scope,window:{kind:'follow_up',after:'old',through:'old'}}}, {...basis,timeBasis:'dispatch_time'}, {...basis,coverage:{timeGaps:-1}}, {...basis,scope:{...basis.scope,window:{kind:'nearest_time'}}}]) {
    assert.equal(config({...configBase,items:[{...item,useBasis}]}),false);
  }
});


test('partial preserves observed positive and zero counts without treating collection completeness as count accuracy', () => {
  const saved = locale.getLocale();
  try { for (const language of ['zh', 'en'] as const) {
    locale.setLocale(language);
    const partial: PublicUseBasis = { ...basis, status: 'partial', sourceCompleteness: 'complete' };
    assert.equal(useBasisCount(3, partial), 3);
    assert.equal(useBasisCount(0, partial), 0);
    assert.equal(useBasisCount(null, partial), undefined);
    assert.equal(useBasisCount(3, { ...partial, status: 'unavailable' }), undefined);
    assert.equal(useBasisCount(0, null), undefined);
    assert.equal(useBasisCount(0, { ...partial, status: 'observed', sourceCompleteness: 'partial' }), 0);
    const text = useBasisPresentation(partial);
    assert.match(text.summary, /完整次数更高|full count higher/);
    assert.match(text.summary, /0.*不证明|0 does not prove/);
    assert.ok(text.notes.includes(language === 'zh' ? '所选来源采集完整' : 'Selected source collection complete'));
    assert.doesNotMatch(text.summary, /15/); // The five gap dimensions are not a record total.
  } } finally { locale.setLocale(saved); }
});
