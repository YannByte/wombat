import { withAutomaticPrices } from './auto-prices.js';
import { createUsageClient, type UsageClient, type AccountTransport } from '../client.js';
import { invokeOperation, type CoreProcessOptions } from './core.js';
import { queryLocal } from './live.js';
import type {CodexOptions} from './codex/process.js';
export {receiveCodexHook} from './collection.js';
export {binaryPath as resolveCoreBinary} from './core.js';

export interface NodeClientOptions extends CoreProcessOptions,CodexOptions { automaticPrices?: boolean }

/** The CLI supplies a packaged binary path; source use can locate the root build. */
export function createNodeClient(options: NodeClientOptions = {}): UsageClient {
  let account: Promise<AccountTransport> | undefined;
  const client = createUsageClient({
    project: (request,q) => queryLocal({project:request},q,options),
    artifacts:(request,queryOptions)=>queryLocal({artifacts:request},queryOptions,options),
    monitor: (r,q) => queryLocal({monitor:r},q,options),
    setup: async (r,q) => (await import('./setup.js')).checkSetup(r,q,options),
    collection: (r, q) => invokeOperation('collection', r, q, {...options,timeoutMs:5000,maxResponseBytes:2*1024*1024}),
    query: async (request, queryOptions) => (await queryLocal({query:request,mode:'auto'},queryOptions,options) as {result:unknown}).result,
    prices: async (request, queryOptions) => (await import('./prices.js')).queryPrices(request, queryOptions, options),
    live: (request, queryOptions) => queryLocal(request, queryOptions, options),
    config: (request, queryOptions) => queryLocal({ config: request.review && !request.scope?.timezone
      ? {...request, scope: {...request.scope, timezone: Intl.DateTimeFormat().resolvedOptions().timeZone}}
      : request }, queryOptions, options),
    optimize: (request, queryOptions) => queryLocal({ optimize: request }, queryOptions, options),
    preferences: (request, queryOptions) => invokeOperation('preferences', request, queryOptions, options),
    directories: (r,q) => invokeOperation('directories',r,q,options),
    conversation: (r,q) => queryLocal({conversation:r},q,options),
    timing: (r,q) => queryLocal({timing:r},q,options),
    account: async (r, q) => (await (account ??= import('./codex/account.js').then(m => m.createAccountTransport(options))))(r, q),
  });
  return options.automaticPrices === false ? client : withAutomaticPrices(client);
}

export {skillCapabilities} from './skill-runtime.js';
export {discoverWombatSkill} from './codex/skills.js';
