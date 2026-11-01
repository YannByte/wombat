import { withTokenAnalysis } from '../../tests/fixtures/token-analysis.js';
import test from 'node:test';
import assert from 'node:assert/strict';
import { parseLanguageArgs } from '../src/locale.js';
import { locale } from '@wombat/client/locale';
import { usageHelp } from '../src/usage-app-cli.js';
import { dateLabel, effort, usageLabel } from '../src/format.js';
test('language selection is independent of command placement and query arguments', () => {
  for (const argv of [['--lang', 'en', 'prices', '--json'], ['prices', '--lang=en', '--json']]) {
    assert.deepEqual(parseLanguageArgs(argv, { WOMBAT_LANG: 'zh' }), { argv: ['prices', '--json'], locale: 'en' });
  }
  assert.equal(parseLanguageArgs([], { LANG: 'C.UTF-8' }).locale, 'en');
  assert.throws(() => parseLanguageArgs(['--lang']), /requires/);
  assert.throws(() => parseLanguageArgs(['--lang=zh', '--lang=en']), /repeated/);
});
test('help, module-level labels and dates follow a switch after imports', () => {
  try {
    locale.setLocale('en');
    assert.match(usageHelp(), /Usage/); assert.match(usageHelp(), /--lang/);
    assert.equal(effort('high'), 'High');
    assert.equal(dateLabel('2026-09-30', '2026-09-30T00:00:00Z'), '9/30');
    locale.setLocale('zh'); assert.equal(effort('high'), '高'); assert.match(usageHelp(), /用法/);
  } finally { locale.setLocale('zh'); }
});
test('monthly CLI buckets use natural names and unknown API cost remains a complete phrase',async()=>{
 const {itemLines}=await import('../src/format.js');
 const result={snapshotRef:{createdAt:'2026-10-02T00:00:00Z'},scope:{timezone:'America/Los_Angeles'},distribution:{}} as any;
 const item={kind:'usage',isSubtotal:true,date:'2026-09-01',scope:{since:'2026-09-01',until:'2026-10-01'},usage:withTokenAnalysis({measurementCount:1,tokens:{total:110},price:{status:'unknown'}})} as any;
 const saved=locale.getLocale();
 try{locale.setLocale('en');assert.match(itemLines(item,result,120,'month').join('\n'),/Sep 2026/);assert.match(itemLines(item,result,120,'month').join('\n'),/Not priced/);locale.setLocale('zh');assert.match(itemLines(item,result,120,'month').join('\n'),/2026年9月/);}finally{locale.setLocale(saved);}
});
test('human help uses Tasks consistently and explains the undated restriction',()=>{
 const saved=locale.getLocale();
 try{locale.setLocale('en');assert.match(usageHelp(),/Task history/);assert.doesNotMatch(usageHelp(),/conversation/i);assert.match(usageHelp(),/--all-time \| --undated/);locale.setLocale('zh');assert.match(usageHelp(),/任务历史/);assert.doesNotMatch(usageHelp(),/对话/);assert.match(usageHelp(),/--all-time \| --undated/);}finally{locale.setLocale(saved);}
});
