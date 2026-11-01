/** Verification transport: human arguments or a generated request, never legacy command aliases. */
import { spawnSync, type SpawnSyncOptionsWithStringEncoding, type SpawnSyncReturns } from 'node:child_process';
import type { AgentRequest, UsageRequest, LiveRequest } from '@wombat/client';

export type CliInput = string[] | AgentRequest;
/** Saved snapshots use the snapshot method; live views use the live wrapper. */
export function queryInvocation(query: UsageRequest, options: Omit<LiveRequest, 'query'> = {}): AgentRequest {
  return query.snapshotId && !query.snapshotId.startsWith('live:')
    ? { method: 'snapshot', params: query }
    : { method: 'usage', params: { query, ...options } };
}
export function cliInvocation(input: CliInput): { args: string[]; input?: string } {
  return Array.isArray(input) ? { args: input } : { args: ['system','call'], input: JSON.stringify(input) };
}
/** Domain assertions compare the same usage result; raw Agent-envelope tests invoke call directly. */
export function spawnProductCli(entry: string, input: CliInput, options: SpawnSyncOptionsWithStringEncoding, nodeArgs: string[] = [], runtime = process.execPath): SpawnSyncReturns<string> {
  const invocation = cliInvocation(input);
  const result = spawnSync(runtime, [...nodeArgs, entry, ...invocation.args], { ...options, ...(invocation.input === undefined ? {} : { input: invocation.input }) });
  if (!Array.isArray(input) && input.method === 'usage' && result.stdout?.trim()) {
    try {
      const value = JSON.parse(result.stdout);
      if (value.result) result.stdout = JSON.stringify(value.result) + '\n';
    } catch { /* Leave malformed output for the caller's assertion. */ }
  }
  return result;
}
