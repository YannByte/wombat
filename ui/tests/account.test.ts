import { test } from 'node:test';
import assert from 'node:assert/strict';
import { createElement } from 'react';
import { renderToStaticMarkup } from 'react-dom/server';
import type { AccountResult } from '@wombat/client';
import { locale } from '@wombat/client/locale';
import { AllowanceDetails, AllowanceWindows } from '../src/account/Allowance.js';
import { AccountCompactButton, AccountSummaryCard, allowanceDuration, summarizeAllowance } from '../src/account/Summary.js';

const section = { status: 'available', checkedAt: '2026-10-03T00:00:00Z', errorCode: null };
const data: AccountResult = {
  outputVersion: 1, action: 'read', nativeVersion: '0.160.0', account: section, allowance: section, activity: section,
  identity: null, ordinaryUsageAllowed: null, modelRestriction: null, summary: null,
  windows: [{ id: 'a:primary', bucketId: 'a', bucketName: 'Synthetic', model: 'other-model', durationMinutes: 17, usedPercent: 100, resetsAt: '2000-01-01T00:00:00Z', status: 'current' }],
  buckets: [{ id: 'a', name: 'Synthetic', model: 'other-model', status: 'current', credits: { balance: '0.0000000000000000001', hasCredits: true, unlimited: false }, individualLimit: { limit: '100.000', used: '0', remainingPercent: 100, resetsAt: null, status: 'current' }, spendControlReached: false, rateLimitReachedType: null }],
  resetCredits: { availableCount: 7, credits: null, detailsTruncated: false },
};
test('allowance displays native precision and read-only reset counts separately in both locales', () => {
  const saved = locale.getSnapshot().locale;
  try { for (const language of ['zh', 'en'] as const) {
    locale.setLocale(language);
    const html = renderToStaticMarkup(createElement(AllowanceDetails, { data, timezone: 'UTC' }));
    assert.match(html, /0\.0000000000000000001/); assert.match(html, /100\.000/); assert.match(html, /other-model/);
    assert.match(html, /17/); assert.match(html, /7/); assert.doesNotMatch(html, /USD|\$|<button/);
    assert.match(html, language === 'en' ? /Reset time has passed/ : /重置时间已到|重置时间已过|重置时间已到达/);
    assert.match(html, language === 'en' ? /Credit details were not provided/ : /未提供权益明细/);
    assert.match(html, language === 'en' ? /availability is unknown/ : /普通使用.*未知/);
  } } finally { locale.setLocale(saved); }
});
test('stale allowance labels, zero counts, empty details and capped overview remain distinct', () => {
  const saved = locale.getSnapshot().locale; locale.setLocale('en');
  try {
    const stale = { ...data, allowance: { ...section, status: 'stale' }, resetCredits: { availableCount: 0, credits: [], detailsTruncated: true } };
    const html = renderToStaticMarkup(createElement(AllowanceDetails, { data: stale, timezone: 'UTC' }));
    assert.match(html, /Previously read information/); assert.match(html, /count: 0/); assert.match(html, /detail list is empty/); assert.match(html, /first 128 rows/);
    const many = { ...data, windows: [0, 1, 2].map(i => ({ ...data.windows[0], id: `window-${i}` })) };
    const compact = renderToStaticMarkup(createElement(AllowanceWindows, { data: many, timezone: 'UTC', compact: true }));
    assert.equal((compact.match(/<meter /g) ?? []).length, 2); assert.match(compact, /1 more window/);
  } finally { locale.setLocale(saved); }
});

test('handoff allowance copy follows the shared assessment and expires to unknown in both locales',async()=>{
 const {HandoffAllowance}=await import('../src/optimize/Allowance.js');
 const saved=locale.getSnapshot().locale;
 try{for(const language of ['zh','en'] as const){locale.setLocale(language);
  const assessment={status:'blocked' as const,model:'synthetic-model',provider:'openai',checkedAt:'2026-10-04T00:00:00Z',validUntil:'2026-10-04T00:01:00Z',bucketId:'synthetic',windowId:null,reason:'native_window_limit'};
  const current=renderToStaticMarkup(createElement(HandoffAllowance,{assessment,now:Date.parse('2026-10-04T00:00:20Z')}));
  assert.match(current,language==='en'?/Refresh your allowance/:/请刷新额度/);
  assert.match(current,language==='en'?/local checks remain available/:/本地检查仍可查看/);
  const old=renderToStaticMarkup(createElement(HandoffAllowance,{assessment,now:Date.parse('2026-10-04T00:01:00Z')}));
  assert.match(old,language==='en'?/sending is not blocked/:/不因此阻止发送/);
  const low=renderToStaticMarkup(createElement(HandoffAllowance,{assessment:{...assessment,status:'low'},now:Date.parse('2026-10-04T00:00:20Z')}));
  assert.match(low,language==='en'?/also uses allowance/:/也会使用额度/);
 }}finally{locale.setLocale(saved);}
});

test('account summary exposes one current window without inventing unknown or expired allowance', () => {
  const current = { ...data, windows: [{ ...data.windows[0], bucketId: 'codex', durationMinutes: 10_080, usedPercent: 72, resetsAt: '2026-10-11T00:00:00Z' }] };
  assert.deepEqual(summarizeAllowance(current, Date.parse('2026-10-04T00:00:00Z')), {
    kind: 'current', remainingPercent: 28, durationMinutes: 10_080, resetsAt: '2026-10-11T00:00:00Z', low: false,
  });
  assert.equal(summarizeAllowance(current, Date.parse('2026-10-11T00:00:00Z')).kind, 'stale');
  assert.equal(summarizeAllowance({ ...current, windows: [{ ...current.windows[0], usedPercent: Number.NaN }] }).kind, 'unknown');
  assert.equal(summarizeAllowance({ ...current, windows: [current.windows[0], { ...current.windows[0], id: 'second' }] }).kind, 'multiple');
  assert.equal(summarizeAllowance(data).kind, 'unknown', 'a model-labeled non-Codex bucket cannot replace the Codex allowance');
});

test('account-wide card and compact entry use localized, responsive summary copy', (context) => {
  context.mock.timers.enable({ apis: ['Date'], now: Date.parse('2026-10-04T00:00:00Z') });
  const saved = locale.getSnapshot().locale;
  const current = { ...data, windows: [{ ...data.windows[0], bucketId: 'codex', durationMinutes: 10_080, usedPercent: 72, resetsAt: '2026-10-11T00:00:00Z' }] };
  const state = { data: current, busy: false, refresh() {} };
  try { for (const language of ['zh', 'en'] as const) {
    context.mock.timers.setTime(Date.parse('2026-10-04T00:00:00Z'));
    locale.setLocale(language);
    const card = renderToStaticMarkup(createElement(AccountSummaryCard, { state, timezone: 'UTC', open() {} }));
    const compact = renderToStaticMarkup(createElement(AccountCompactButton, { state, open() {} }));
    assert.match(card, language === 'en' ? /Codex allowance/ : /Codex 额度/);
    assert.match(card, language === 'en' ? /Account-wide/ : /账户级/);
    assert.match(card, language === 'en' ? /28% remaining/ : /剩余 28%/);
    assert.match(card, language === 'en' ? /7-day allowance/ : /7 天额度/);
    assert.match(card, language === 'en' ? /Resets/ : /重置时间/);
    assert.match(card, /<meter/);
    assert.match(compact, language === 'en' ? /Codex allowance · 28%/ : /Codex 额度 · 28%/);
    assert.equal(allowanceDuration(60), language === 'en' ? '1-hour allowance' : '1 小时额度');
    context.mock.timers.setTime(Date.parse(current.windows[0].resetsAt));
    const expiredCard = renderToStaticMarkup(createElement(AccountSummaryCard, { state, timezone: 'UTC', open() {} }));
    const expiredCompact = renderToStaticMarkup(createElement(AccountCompactButton, { state, open() {} }));
    assert.doesNotMatch(expiredCard, /<meter|28%/);
    assert.doesNotMatch(expiredCompact, /28%/);
    assert.match(expiredCard, language === 'en' ? /Awaiting update/ : /待更新/);
  } } finally { locale.setLocale(saved); }
});

test('multiple, stale and unavailable summaries never render a zero-percent progress bar', () => {
  const variants = [
    { ...data, windows: [{ ...data.windows[0], bucketId: 'codex' }, { ...data.windows[0], bucketId: 'codex', id: 'second' }] },
    { ...data, allowance: { ...section, status: 'stale' }, windows: [{ ...data.windows[0], bucketId: 'codex', usedPercent: 100 }] },
    { ...data, windows: [] },
  ];
  for (const value of variants) {
    const html = renderToStaticMarkup(createElement(AccountSummaryCard, { state: { data: value, busy: false, refresh() {} }, timezone: 'UTC', open() {} }));
    assert.doesNotMatch(html, /<meter/);
    assert.doesNotMatch(html, /0%/);
  }
});
