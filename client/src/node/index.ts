import { withAutomaticPrices } from './auto-prices.js';
import { createUsageClient, type UsageClient, type AccountTransport, type HandoffTransport } from '../client.js';
import { invokeCore, invokeOperation, type CoreProcessOptions } from './core.js';
import { queryLive } from './live.js';
import { createDirectoryTransport } from './directories.js';
import type {CodexOptions} from './codex/process.js';
export {receiveCodexHook} from './collection.js';
export {binaryPath as resolveCoreBinary} from './core.js';

export interface NodeClientOptions extends CoreProcessOptions,CodexOptions { automaticPrices?: boolean; directoryPicker?:(signal?:AbortSignal)=>Promise<string> }

/** The CLI supplies a packaged binary path; source use can locate the root build. */
export function createNodeClient(options: NodeClientOptions = {}): UsageClient {
  let account: Promise<AccountTransport> | undefined, handoff: Promise<HandoffTransport> | undefined;
  const client = createUsageClient({
    monitor: (r,q) => r.action==='check' && r.snapshotId.startsWith('live:') ? queryLive({monitor:r},q,options) : invokeOperation('monitor',r,q,options),
    setup: async (r,q) => (await import('./setup.js')).checkSetup(r,q,options),
    collection: (r, q) => invokeOperation('collection', r, q, {...options,timeoutMs:5000,maxResponseBytes:2*1024*1024}),
    query: (request, queryOptions) => invokeCore(request, queryOptions, options),
    prices: async (request, queryOptions) => (await import('./prices.js')).queryPrices(request, queryOptions, options),
    live: (request, queryOptions) => queryLive(request, queryOptions, options),
    config: (request, queryOptions) => queryLive({ config: request }, queryOptions, options),
    optimize: (request, queryOptions) => queryLive({ optimize: request }, queryOptions, options),
    preferences: (request, queryOptions) => invokeOperation('preferences', request, queryOptions, options),
    directories: createDirectoryTransport(options, options.directoryPicker),
    conversation: (r, q) => !r.snapshotId.startsWith('live:') && !(r.baselineSnapshotId?.startsWith('live:')) ? invokeOperation('conversation', r, q, options) : queryLive({conversation:r},q,options),
    timing: (request, queryOptions) => request.action === 'capabilities'
      || (request.snapshotId != null && !request.snapshotId.startsWith('live:'))
      ? invokeOperation('timing', request, queryOptions, options)
      : queryLive({ timing: request }, queryOptions, options),
    account: async (r, q) => (await (account ??= import('./codex/account.js').then(m => m.createAccountTransport(options))))(r, q),
    handoff: async (r, q) => (await (handoff ??= import('./codex/handoff.js').then(m => m.createHandoffTransport(options))))(r, q),
  });
  return options.automaticPrices === false ? client : withAutomaticPrices(client);
}

export {manageSkill,skillCapabilities,type SkillInstallationOptions} from './skill-installation.js';
export {discoverWombatSkill} from './codex/skills.js';
