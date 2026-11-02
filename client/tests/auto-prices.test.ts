import { withTokenAnalysis } from '../../tests/fixtures/token-analysis.js';
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { CoreError, type UsageClient, type LiveResult, type PricingResult } from '@wombat/client';
import { automaticPriceText, locale } from '@wombat/client/locale';
import { withAutomaticPrices } from '../dist/node/auto-prices.js';

function result(): LiveResult {
  return { outputVersion: 1, freshness: { status: 'current', revision: 'one' }, result: {
    outputVersion: 5, action: 'usage', snapshotRef: { snapshotId: 'one', createdAt: '2026-09-30T00:00:00Z' }, scope: {}, availableRange: {},
    summary: withTokenAnalysis({ tokens: { total: 110 }, measurementCount: 1, price: { currency: 'USD', policy: 'synthetic', priceRevision: 'old', cost: null, knownCost: '0', status: 'unknown', components: [], basis: [], issues: ['catalogPriceMissing'] } }),
    items: [], page: { offset: 0, limit: 50, total: 0 }, quality: { issueScope: 'index', queryImpact: 'none', status: 'complete', issues: [], sources: [] },
  } };
}
const prices: PricingResult = { outputVersion: 1, action: 'auto_update', origin: 'bundled', updated: false, source: 'synthetic', catalogHash: 'same', catalog: { revision: 'old', verifiedAt: 'synthetic', policy: 'synthetic', currency: 'USD', models: [] }, automatic: { status: 'unchanged', attemptId: 'a', attemptedAt: new Date().toISOString(), retryAt: new Date(Date.now() + 86400_000).toISOString() } };

test('automatic prices honor offline/fixed views, reuse cooldown, preserve scope and expose bilingual state', async () => {
  const env = process.env.WOMBAT_AUTO_PRICES;
  process.env.WOMBAT_AUTO_PRICES = '1';
  try {
    let calls = 0;
    const queries: unknown[] = [];
    const base: UsageClient = { query: async () => result().result, live: async request => { queries.push(request); return result(); }, prices: async () => { calls++; return structuredClone(prices); } };
    const client = withAutomaticPrices(base);
    await client.live!({ query: { action: 'usage' }, mode: 'cached' });
    await client.live!({ query: { action: 'usage' }, mode:'cached' });
    // Default comparisons keep their committed view even if official prices are missing.
    const comparison = {action:'compare' as const,comparison:{kind:'sessions' as const,leftThreadId:'a',rightThreadId:'b'}};
    await client.live!({query:comparison});
    await client.live!({query:comparison,mode:'auto'});
    assert.equal(calls, 0);
    const checked = await client.live!({ query: { action: 'usage' } });
    assert.equal(checked.result.priceUpdate?.status, 'unchanged');
    await client.live!({ query: { action: 'usage' } });
    assert.equal(calls, 1);
    locale.setLocale('zh'); assert.match(automaticPriceText(checked.result)!, /仍有缺失价格/);
    locale.setLocale('en'); assert.match(automaticPriceText(checked.result)!, /remain unavailable/);
    const updated = withAutomaticPrices({ ...base, prices: async () => ({ ...prices, updated: true, catalog: { ...prices.catalog, revision: 'new' }, automatic: { ...prices.automatic!, status: 'updated' } }) });
    const request = { query: { action: 'usage' as const, scope: { model: 'synthetic-new', since: '2026-09-29' }, offset: 10 } };
    await updated.live!(request);
    assert.deepEqual(queries.at(-1), { ...request, mode: 'fresh' });
    process.env.WOMBAT_AUTO_PRICES = '0'; await client.live!({ query: { action: 'usage' } }); assert.equal(calls, 1);
  } finally { locale.setLocale('zh'); if (env === undefined) delete process.env.WOMBAT_AUTO_PRICES; else process.env.WOMBAT_AUTO_PRICES = env; }
});

test('automatic update errors retain usage and back off; cancellation stays cancellation', async () => {
  const env = process.env.WOMBAT_AUTO_PRICES; process.env.WOMBAT_AUTO_PRICES = '1';
  try {
    let calls = 0;
    const base: UsageClient = { query: async () => result().result, live: async () => result(), prices: async () => { calls++; throw new CoreError('PRICE_AUTO_STATE_INVALID', 'bad state'); } };
    const client = withAutomaticPrices(base);
    const value = await client.live!({ query: { action: 'usage' } });
    assert.equal(value.result.summary.tokens.total, 110);
    assert.equal(value.result.priceUpdate?.errorCode, 'PRICE_AUTO_STATE_INVALID');
    await client.live!({ query: { action: 'usage' } }); assert.equal(calls, 1);
    const cancelled = withAutomaticPrices({ ...base, prices: async () => { throw new CoreError('CANCELLED', 'cancelled'); } });
    await assert.rejects(cancelled.live!({ query: { action: 'usage' } }), (e: unknown) => e instanceof CoreError && e.code === 'CANCELLED');
  } finally { if (env === undefined) delete process.env.WOMBAT_AUTO_PRICES; else process.env.WOMBAT_AUTO_PRICES = env; }
});
