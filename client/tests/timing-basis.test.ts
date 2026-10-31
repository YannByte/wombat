import test from 'node:test';
import assert from 'node:assert/strict';
import type { Basis } from '../src/generated/timing-local-response.js';
import { locale, t } from '../src/locale/index.js';
import { timingBasisText, timingMissingValueText, timingSourceStatusText } from '../src/locale/timing-basis.js';

const bases = [
  'determinate_terminal_outcomes', 'native_record', 'explicit_boundary', 'lifecycle_union', 'lifecycle_sum', 'interval_mask',
  'request_input', 'historical_window', 'type7', 'safe_message_record', 'safe_message_delay',
  'safe_event_count', 'response_gap_v1', 'not_recorded', 'adapter_not_mapped', 'unsupported_method',
  'missing_identity', 'missing_time', 'running_turn', 'exact_event_page', 'boundary_conflict',
  'source_partial', 'resource_limit', 'numeric_range', 'no_candidates', 'missing_batch_cycle',
  'missing_repository_baseline', 'unknown_message_origin', 'canonical_operation_identity',
  'reported_file_paths', 'canonical_use_identity', 'canonical_use_records', 'unassigned_use_index',
  'dispatch_not_proven', 'missing_target', 'missing_turn', 'target_conflict', 'outcome_conflict',
] as const satisfies readonly Basis[];

test('every timing basis has localized provenance text, while null values only use gap explanations', () => {
  const previous = locale.getLocale();
  try {
    for (const language of ['zh', 'en'] as const) {
      locale.setLocale(language);
      for (const basis of bases) {
        const text = timingBasisText(basis);
        assert.ok(text.length > 0, `${language}: ${basis}`);
        assert.notEqual(text, basis, `${language}: ${basis} must not leak the protocol value`);
        assert.notEqual(text, t('timing.basis.evidenceInsufficient'), `${language}: ${basis} has its own explanation`);
      }
      assert.equal(timingMissingValueText('native_record'), t('timing.basis.evidenceInsufficient'));
      assert.notEqual(timingMissingValueText('missing_time'), t('timing.basis.evidenceInsufficient'));
      assert.notEqual(timingMissingValueText('not_recorded'), timingMissingValueText('missing_time'));
      assert.equal(timingSourceStatusText('future_status'), t('timing.sourceStatus.unconfirmed'));
    }
  } finally {
    locale.setLocale(previous);
  }
});

test('timing explanations follow the active locale for each call', () => {
  const previous = locale.getLocale();
  try {
    locale.setLocale('en');
    const english = timingMissingValueText('missing_time');
    locale.setLocale('zh');
    const chinese = timingMissingValueText('missing_time');
    assert.notEqual(english, chinese);
    assert.match(english, /usable timing records/i);
    assert.match(chinese, /时间记录/);
  } finally {
    locale.setLocale(previous);
  }
});

test('conflicting targets and outcomes retain usable timing in both languages', () => {
  const previous = locale.getLocale();
  try {
    for (const language of ['zh', 'en'] as const) {
      locale.setLocale(language);
      for (const basis of ['target_conflict', 'outcome_conflict'] as const) {
        const text = timingBasisText(basis);
        assert.equal(timingMissingValueText(basis), text);
        assert.notEqual(text, t('timing.basis.evidenceInsufficient'));
        assert.notEqual(text, timingBasisText('boundary_conflict'));
        assert.match(text, language === 'zh' ? /有效的起止记录仍可用于计时/ : /valid start and end records remain usable for timing/);
      }
      assert.notEqual(timingBasisText('target_conflict'), timingBasisText('outcome_conflict'));
    }
  } finally {
    locale.setLocale(previous);
  }
});
