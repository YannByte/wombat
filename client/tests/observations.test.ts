import { test } from 'node:test';
import assert from 'node:assert/strict';
import type { ConfigItem, ConfigResult, OptimizeSuggestion, UsageResult } from '@wombat/client';
import { inventoryRecordState, observedCount, sourceReadLabel, locale, reviewFindingCount, reviewFindingNote, reviewPresentation } from '../src/locale/index.js';
const coverage: ConfigResult['coverage'] = { status: 'complete', historyStatus: 'current', absenceObservable: true, issues: [], supportedEvidence: ['file_read', 'tool_call', 'skill_invocation', 'hook_execution'] };
function item(kind: ConfigItem['kind'], count: number | null = 0): ConfigItem { return { kind, usageCount: count, counts: { fileReads: 0 }, observation: 'unknown', lastRecordAt: null } as ConfigItem; }

test('zero requires complete selected-range coverage; loaded-only evidence stays distinct from observed Skill use', () => {
  for (const language of ['zh', 'en'] as const) {
    locale.setLocale(language);
    assert.equal(inventoryRecordState(item('mcp'), coverage).kind, 'absent');
    assert.match(inventoryRecordState(item('rule'), coverage).text!, /无读取记录|No recorded reads/);
    for (const c of [{ ...coverage, status: 'partial' }, { ...coverage, absenceObservable: false }, { ...coverage, supportedEvidence: [] }]) assert.equal(inventoryRecordState(item('mcp'), c).kind, 'unknown');
    const unknownRule = inventoryRecordState(item('rule'), { ...coverage, status: 'partial' });
    assert.match(unknownRule.text!, language === 'zh' ? /已发现，是否加载无法确认/ : /Discovered; load unconfirmed/);
    assert.match(unknownRule.hint!, language === 'zh' ? /不代表 AGENTS\.md 未被使用/ : /does not mean AGENTS\.md was unused/);
    assert.equal(inventoryRecordState(item('skill', null), coverage).kind, 'unknown');
    assert.equal(inventoryRecordState({ ...item('skill'), observation: 'loaded_only', lastRecordAt: '2026-10-03T00:00:00Z' }, coverage).kind, 'unknown');
    assert.equal(observedCount(3, { ...coverage, status: 'partial' }, 'tool_call'), 3);
    assert.equal(observedCount(0, { ...coverage, status: 'partial' }, 'tool_call'), undefined);
  }
});

test('source states and finding quantities stay distinct and natural in both languages', () => {
  for (const language of ['zh', 'en'] as const) {
    locale.setLocale(language);
    const labels = ['notFound', 'failed', 'partial', 'unsupported', 'complete'].map(status => sourceReadLabel({ status } as UsageResult['quality']['sources'][number]));
    assert.equal(new Set(labels).size, 5);
    for (const count of [1, 2]) {
      const f = { rule: 'localReference', observed: count, evidenceCodes: [] } as OptimizeSuggestion['findings'][number];
      assert.equal(reviewFindingCount(f), language === 'zh' ? `${count} 处失效引用` : `${count} missing reference${count === 1 ? '' : 's'}`);
      const format = { ...f, rule: 'skillFormat', evidenceCodes: Array.from({ length: count }, (_, i) => String(i)) };
      assert.equal(reviewFindingCount(format), language === 'zh' ? `${count} 处格式错误` : `${count} format error${count === 1 ? '' : 's'}`);
    }

  }
});


test('a partial zero remains an observed count without becoming an absence label', async () => {
  const { useBasisCount } = await import('../src/locale/index.js');
  const basis: NonNullable<ConfigItem['useBasis']> = { methodVersion: 3, status: 'partial', unit: 'object_use', capturedAt: '2026-10-05T00:00:00Z', snapshotId: 'synthetic', scope: { sourceInstanceIds: ['synthetic'], project: null, threadId: null, agentKind: null, window: { kind: 'all_history' } }, timeBasis: 'source_operation_time', coverage: { targetGaps: 1 }, sourceCompleteness: 'complete' };
  const saved = locale.getLocale();
  try { for (const language of ['zh', 'en'] as const) {
    locale.setLocale(language);
    const row = { ...item('mcp'), useBasis: { ...basis, status: 'partial' as const } };
    assert.equal(useBasisCount(row.usageCount, row.useBasis), 0);
    const state = inventoryRecordState(row, coverage);
    assert.equal(state.kind, 'unknown');
    assert.match(state.text!, /部分观察|partially observed/);
    assert.match(state.hint!, /0.*不证明|0 does not prove/);
    assert.doesNotMatch(state.text!, /无使用|No recorded use/);
  } } finally { locale.setLocale(saved); }
});
