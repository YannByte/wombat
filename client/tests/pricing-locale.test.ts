import test from 'node:test';
import assert from 'node:assert/strict';
import { locale, pricingIssueText } from '../src/locale/index.js';

test('pricing explanations distinguish count conflicts, unmatched models, tiers and missing rates in both languages', () => {
  const previous = locale.getLocale();
  try {
    for (const language of ['zh', 'en'] as const) {
      locale.setLocale(language);
      const codes = ['inconsistentTokenCounts', 'unverifiedModel', 'modelContextConflict', 'requestContextUnknown', 'catalogPriceMissing',
        'inputPriceUnknown', 'cacheReadPriceUnknown', 'cacheCreatePriceUnknown', 'outputPriceUnknown'];
      const explanations = codes.map(pricingIssueText);
      assert.equal(new Set(explanations).size, codes.length);
      explanations.forEach((text, index) => {
        assert.notEqual(text, codes[index]);
        assert.doesNotMatch(text, /usage\.priceReason|\$0|免费|free/i);
      });
      assert.doesNotMatch(pricingIssueText('untrusted-source-sentinel'), /untrusted-source-sentinel/);
      assert.match(pricingIssueText('requestContextUnknown'), language === 'zh' ? /价格档位/ : /pricing tier/);
      assert.match(pricingIssueText('inconsistentTokenCounts'), language === 'zh' ? /矛盾/ : /conflict/);
    }
    locale.setLocale('en'); const english = pricingIssueText('catalogPriceMissing');
    locale.setLocale('zh'); assert.notEqual(pricingIssueText('catalogPriceMissing'), english);
  } finally { locale.setLocale(previous); }
});
