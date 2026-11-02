import { CoreError } from '../errors.js';
import type { LiveRequest, LiveResult, PricingResult, QueryOptions, UsageClient } from '../client.js';

/** Decorates validated live results; eligibility and cross-process retry state are core-owned. */
export function withAutomaticPrices(client: UsageClient): UsageClient {
  let cached: PricingResult | undefined;
  let failed: LiveResult["result"]["priceUpdate"];
  return { ...client, async live(request: LiveRequest, options: QueryOptions = {}): Promise<LiveResult> {
    let result = await client.live!(request, options);
    if (request.mode === 'cached' || (['compare','investigate','trajectory','resources','review','context'].includes(request.query.action) && request.mode !== 'fresh')
      || process.env.WOMBAT_AUTO_PRICES === '0'
      || !result.result.summary.price.issues.includes('catalogPriceMissing')) return result;
    if (failed && Date.parse(failed.retryAt) > Date.now()) { result.result.priceUpdate = failed; return result; }
    try {
      if (!cached?.automatic || Date.parse(cached.automatic.retryAt) <= Date.now()) {
        cached = await client.prices({ action: 'auto_update' }, options);
        failed = undefined;
      }
      const prices = cached;
      if (prices.automatic?.status === 'updated' && result.result.summary.price.priceRevision !== prices.catalog.revision) {
        // Always retain the selected scope; replay once only. Fixed views are never repriced.
        result = await client.live!({ ...request, mode: 'fresh' }, options);
      }
      result.result.priceUpdate = prices.automatic;
    } catch (error) {
      if (options.signal?.aborted || (error instanceof CoreError && error.code === 'CANCELLED')) throw error;
      failed = result.result.priceUpdate = {
        status: 'failed', attemptId: '', attemptedAt: new Date().toISOString(), retryAt: new Date(Date.now() + 60_000).toISOString(),
        errorCode: error instanceof CoreError ? error.code : 'PRICE_UPDATE_FAILED',
      };
    }
    return result;
  } };
}
