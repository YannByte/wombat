import { CoreError } from '../../errors.js';
import type { AccountRequest, AccountResult, QueryOptions } from '../../client.js';
import { invokeOperation, type CoreProcessOptions } from '../core.js';
import type { connectCodex } from './rpc.js';
export type CodexConnection = Awaited<ReturnType<typeof connectCodex>>;
const code = (e: unknown) => e instanceof CoreError ? e.code : 'CODEX_UNAVAILABLE';
export function unavailableCapture(error?: unknown) {
  return { checkedAt: new Date().toISOString(), before: null as unknown, after: null as unknown, rates: null as unknown, activity: null as unknown,
    accountError: error ? code(error) : null as string | null, ratesError: null as string | null, activityError: null as string | null };
}
/** Read identity on both sides; a capture never reuses the display cache. */
export async function captureAccount(rpc: CodexConnection, activity: boolean, query: QueryOptions) {
  const capture = unavailableCapture();
  try {
    capture.before = await rpc.request('account/read', { refreshToken: false });
    capture.checkedAt = new Date().toISOString();
    const sections = await Promise.allSettled([rpc.request('account/rateLimits/read', {}), activity ? rpc.request('account/usage/read', {}) : Promise.resolve(null)]);
    if (sections[0].status === 'fulfilled') capture.rates = sections[0].value; else capture.ratesError = code(sections[0].reason);
    if (sections[1].status === 'fulfilled') capture.activity = sections[1].value; else capture.activityError = code(sections[1].reason);
    capture.after = await rpc.request('account/read', { refreshToken: false });
  } catch (error) { if (query.signal?.aborted) throw new CoreError('CANCELLED', 'Cancelled'); capture.accountError = code(error); }
  return capture;
}
export async function normalizeAccount(capture: ReturnType<typeof unavailableCapture>, action: AccountRequest['action'], version: string | null, query: QueryOptions, options: CoreProcessOptions): Promise<AccountResult> {
  return await invokeOperation('native_account', { ...capture, action: action ?? 'read', nativeVersion: version }, query, options) as AccountResult;
}
