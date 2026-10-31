import test from 'node:test';
import assert from 'node:assert/strict';
import { LocaleRuntime, resolveLocale, locale, relatedActivityText, monthLabel, eventStatusLabel, numberLabel, compactNumberLabel } from '../src/locale/index.js';
test('explicit, environment and system language precedence, including regional tags', () => {
  assert.equal(resolveLocale({ explicit: 'en-US', environment: 'zh', languages: ['zh-CN'] }), 'en');
  assert.equal(resolveLocale({ environment: 'zh_CN.UTF-8', languages: ['en'] }), 'zh');
  assert.equal(resolveLocale({ languages: ['ja', 'en-GB'] }), 'en');
  assert.equal(resolveLocale({ languages: ['ja'] }), 'zh');
  assert.throws(() => resolveLocale({ explicit: 'fr' }), /Unsupported/);
});
test('translation function stays stable across switches, interpolation leaves data literal', () => {
  const runtime = new LocaleRuntime(), translate = runtime.t;
  assert.equal(translate('cli.format.usage'), '用量');
  runtime.setLocale('en'); runtime.setLocale('en');
  assert.equal(translate, runtime.t);
  assert.equal(translate('cli.format.usage'), 'Usage');
  const value = '中文 {p0} $1';
  assert.equal(translate('common.turn_value', { p0: value }), `Turn ${value}`);
  runtime.setLocale('zh'); assert.equal(runtime.getLocale(), 'zh');
});
test('count agreement preserves zero, unknown values and each independent count', () => {
  const runtime = new LocaleRuntime('en');
  for (const [count, suffix] of [[0, 'tasks'], [1, 'task'], [2, 'tasks'], ['—', 'tasks']] as const) {
    assert.equal(runtime.t('webui.countThreads', { count }), `${count} ${suffix}`);
  }
  assert.equal(runtime.t('config.count', { count: 1 }), '1 configuration');
  assert.equal(runtime.t('webui.sourceCount', { count: 1 }), '1 source');
  assert.equal(runtime.t('webui.periodCount', { count: 1 }), '1 period');
  assert.equal(runtime.t('webui.page', { current: 1, total: 1, count: 1 }), 'Page 1 / 1 · 1 item');
  assert.equal(runtime.t('config.fileReadsCount', { count: 1 }), '1 file read');
  assert.equal(runtime.t('config.fileReadsCount', { count: 2 }), '2 file reads');
  assert.equal(runtime.t('config.toolCallsCount', { count: 1 }), '1 tool call');
  const saved = locale.getLocale();
  try {
    locale.setLocale('en');
    for (const turns of [0, 1, 2]) for (const count of [0, 1, 2]) {
      assert.equal(relatedActivityText(turns, count), `View task turns · ${turns} ${turns === 1 ? 'turn' : 'turns'}, ${count} ${count === 1 ? 'record' : 'records'}`);
    }
    assert.equal(relatedActivityText('—', '—'), 'View task turns · — turns, — records');
    assert.equal(monthLabel('2026-09-01'), 'Sep 2026');
    locale.setLocale('zh');
    assert.equal(monthLabel('2026-09-01'), '2026年9月');
    assert.match(relatedActivityText(1, 2), /1 轮，2 条/);
    assert.equal(monthLabel('literal/2026-09-file'), 'literal/2026-09-file');
  } finally { locale.setLocale(saved); }
});
test('compact byte labels retain binary units and leave exact source bytes unchanged', async()=>{
 const {bytesLabel}=await import('../src/locale/index.js');
 const saved=locale.getLocale();
 try {for(const language of ['zh','en'] as const){locale.setLocale(language);const bytes=10954;assert.equal(bytesLabel(bytes),'10.7 KiB');assert.equal(bytes,10954);assert.equal(bytesLabel(1024),'1 KiB');assert.equal(bytesLabel(0),'0 KiB');assert.equal(bytesLabel(null),'—');}}
 finally{locale.setLocale(saved);}
});

test('static evidence notes do not describe matching blocks or copy drift as malformed syntax',async()=>{
 const {reviewFindingNote}=await import('../src/locale/index.js');const saved=locale.getLocale();
 try{for(const language of ['zh','en'] as const){locale.setLocale(language);for(const rule of ['exactInstructionBlocks','declaredCopyDrift']){const note=reviewFindingNote(rule);assert.doesNotMatch(note,/字段格式|format check|invalid format/i);assert.ok(note.length>10);}}}finally{locale.setLocale(saved);}
});
test('confirmed brand copy preserves Chinese efficiency and English better with its punctuation',()=>{
 const runtime=new LocaleRuntime('zh');assert.equal(runtime.t('webui.tagline'),'让 AI 工作更高效');runtime.setLocale('en');assert.equal(runtime.t('webui.tagline'),'Make AI work better.');
});
test('operation outcomes distinguish running, interrupted and unknown in both locales',()=>{
 const saved=locale.getLocale();
 try{
  for(const [language,expected] of [['zh',['进行中','已中断','未知','已完成','失败']],['en',['Running','Interrupted','Unknown','Completed','Failed']]] as const){
   locale.setLocale(language);assert.deepEqual(['running','interrupted','unknown','completed','failed'].map(eventStatusLabel),expected);
   assert.equal(eventStatusLabel('source-defined-status'),'source-defined-status');
  }
 }finally{locale.setLocale(saved);}
});

test('number display follows language switches while zero and missing values remain distinct', () => {
  const saved = locale.getLocale();
  try {
    for (const [language, compact] of [['en', '12.35K'], ['zh', '1.23万'], ['en', '12.35K']] as const) {
      locale.setLocale(language);
      assert.equal(numberLabel(12345), '12,345');
      assert.equal(compactNumberLabel(12345), compact);
      assert.equal(numberLabel(0), '0');
      assert.equal(compactNumberLabel(0), '0');
      assert.equal(numberLabel(null), '—');
      assert.equal(compactNumberLabel(undefined), '—');
      assert.equal(numberLabel(-1234.5), '-1,234.5');
      assert.equal(numberLabel(0.25, { style: 'percent' }), '25%');
    }
  } finally { locale.setLocale(saved); }
});
test('configuration, comparison and analysis sentences agree with single counts', () => {
  const runtime = new LocaleRuntime('en');
  const variants = [
    ['config.relatedTaskCount', 'task', 'tasks'],
    ['config.findingCount', 'suggestion', 'suggestions'],
    ['comparison.members', 'session', 'sessions'],
  ] as const;
  for (const [key, one, other] of variants) {
    for (const count of [0, 1, 2, '—'] as const) {
      assert.equal(runtime.t(key, { count }), `${count} ${count === 1 ? one : other}`);
    }
  }
  assert.equal(runtime.t('inspection.coveredDuration', { count: 1 }), 'Covers 1 operation');
  assert.equal(runtime.t('comparison.undated', { count: 1 }), '1 undated record is excluded from the period comparison.');
  assert.equal(runtime.t('inspection.observed.repeated_request', { count: 1 }), '1 additional identical request observation within matching turns and receivers.');
  assert.match(runtime.t('usage.tokenAnalysis.formula', { count: 1 }), /^Totals for 1 response calculated/);
  assert.match(runtime.t('usage.tokenAnalysis.formula', { count: 2 }), /^Totals for 2 responses calculated/);
  runtime.setLocale('zh');
  assert.equal(runtime.t('config.relatedTaskCount', { count: 1 }), '1 个任务');
});
