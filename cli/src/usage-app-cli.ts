import { commandNames, commandRoutes, firstCommandIndex } from './command-routing.js';
import { resultExitCode } from './exit-codes.js';
import { configureLanguage } from './locale.js';
import { t } from '@wombat/client/locale';
import { setTimeout as delay } from 'node:timers/promises';
import packageMetadata from '../package.json' with { type: 'json' };
import { CoreError, type UsageRequest } from '@wombat/client';
import { createNodeClient } from '@wombat/client/node';
export function usageHelp(): string {
  return t('cli.usage-app-cli.help');
}
export interface Invocation {
  request: UsageRequest;
  json: boolean;
  help: boolean;
  version: boolean;
  mode: 'auto' | 'fresh' | 'cached';
  watch: boolean;
  waitMs?: number;
  rebuild?: boolean;
  includeHistory?: boolean;
}
const valued = new Set(['wait-ms', 'root', 'snapshot', 'agent', 'source', 'timezone', 'since', 'until', 'model', 'effort', 'project', 'thread', 'group', 'presentation', 'sort', 'limit', 'offset', 'baseline-since', 'baseline-until', 'dimension', 'other-thread']);
let knownTimezones: Set<string> | undefined;
function invalid(message: string): never { throw new CoreError('INVALID_ARGUMENT', message); }
export function parseUsageArgs(argv: string[]): Invocation {
  let action: 'usage' | 'statistics' | 'compare' = 'usage';
  argv=[...argv];
  if(argv[0]==='usage') { argv.shift(); const nested: string | undefined = argv[0]; if(nested==='stats'||nested==='compare') { action=nested==='stats'?'statistics':'compare'; argv.shift(); } }
  let json = false;
  let help = false;
  let version = false;
  const liveFlags = new Set<string>();
  const unknownFlags = new Set<string>();
  const values = new Map<string, string>();
  const roots: string[] = [];
  for (let i = 0;i < argv.length;i++) {
    const arg = argv[i];
    if (['--fresh', '--cached', '--watch', '--rebuild', '--include-history'].includes(arg)) { if (liveFlags.has(arg)) invalid(t("common.value_cannot_be_repeated", { p0: arg })); liveFlags.add(arg); continue; }
    if (arg === '--version' || arg === '-v') {
      version = true;
      continue;
    }
    if (arg === 'help' || arg === '--help' || arg === '-h') {
      help = true;
      continue;
    }
    if (['--all-time', '--model-unknown', '--effort-unknown', '--undated', '--project-unknown', '--family'].includes(arg)) { if (unknownFlags.has(arg)) invalid(t("common.value_cannot_be_repeated", { p0: arg })); unknownFlags.add(arg); continue; }
    if (arg === '--compact') {
      if (liveFlags.has(arg)) invalid(t("common.value_cannot_be_repeated", {p0: arg}));
      liveFlags.add(arg);
      continue;
    }
    if (arg === '--json') {
      if (json)
        invalid(t("cli.usage-app-cli.json_cannot_be_repeated"));
      json = true;
      continue;
    }
    if (!arg.startsWith('--')) invalid(t('cli.usage-app-cli.unknown_command_value',{p0:arg}));
    const equal = arg.indexOf('=');
    const name = arg.slice(2, equal < 0 ? undefined : equal);
    if (!valued.has(name))
      invalid(t("cli.usage-app-cli.unknown_option_value", { p0: name }));
    const value = equal < 0 ? argv[++i] : arg.slice(equal + 1);
    if (!value || value.startsWith('--'))
      invalid(t("cli.usage-app-cli.value_requires_a_value", { p0: name }));
    if (name === 'root') {
      roots.push(value);
      continue;
    }
    if (values.has(name))
      invalid(t("cli.usage-app-cli.value_cannot_be_repeated", { p0: name }));
    values.set(name, value);
  }
  if (version && argv.some(arg => !['--version', '-v', '--json'].includes(arg)))
    invalid(t("cli.usage-app-cli.version_cannot_be_combined_with_query"));
  const request: UsageRequest = { action };
  if (liveFlags.has('--compact')) {
    if (!json) invalid(t('cli.usage-app-cli.compactRequiresJson'));
    request.compact = true;
  }
  const waitText = values.get('wait-ms');
  if (waitText !== undefined && (!/^[0-9]+$/.test(waitText) || Number(waitText) < 1 || Number(waitText) > 600_000)) invalid(t('cli.usage-app-cli.wait_invalid'));
  values.delete('wait-ms');
  const rebuild = liveFlags.has('--rebuild');
  if ((waitText !== undefined || rebuild) && (liveFlags.has('--cached') || liveFlags.has('--watch') || values.has('snapshot'))) invalid(t('cli.usage-app-cli.wait_invalid'));
  const scope: NonNullable<UsageRequest['scope']> = {};
  const allowed = new Set(['root','snapshot','agent','source','timezone','since','until','model','effort','project','thread','limit','offset',...(action==='usage'?['group','presentation','sort']:[]),...(action==='statistics'?['presentation','baseline-since','baseline-until']:[]),...(action==='compare'?['baseline-since','baseline-until','dimension','other-thread']:[])]);
  for (const name of [...values.keys(), ...(roots.length ? ['root'] : [])])
    if (!allowed.has(name))
      invalid(t("cli.usage-app-cli.value_does_not_support_value", { p0: action, p1: name }));
  for (const key of ['since', 'until'] as const) {
    const value = values.get(key);
    if (value) {
      if (!/^\d{4}-\d{2}-\d{2}$/.test(value) || !Number.isFinite(Date.parse(value)) || new Date(value).toISOString().slice(0, 10) !== value)
        invalid(t("cli.usage-app-cli.value_requires_a_valid_date_in", { p0: key }));
      scope[key] = value;
    }
  }
  if (unknownFlags.has('--undated')) { if (scope.since || scope.until) invalid(t("cli.usage-app-cli.undated_cannot_be_combined_with_a")); scope.undated = true; }
  if(unknownFlags.has('--all-time')) { if(scope.since || scope.until || scope.undated) invalid(t('cli.config.invalid',{value:'--all-time'})); scope.allTime=true; }
  if (scope.since && scope.until && scope.since >= scope.until)
    invalid(t("cli.usage-app-cli.until_must_be_later_than_since"));
  const timezone = values.get('timezone');
  if (timezone) {
    try {
      // Named zones need no formatter. Keep Intl's existing fallback for aliases
      // and offset syntax, and let the core validate its own supported zones.
      if (timezone !== 'UTC' && !(knownTimezones ??= new Set(Intl.supportedValuesOf('timeZone'))).has(timezone))
        new Intl.DateTimeFormat('zh-CN', { timeZone: timezone });
    }
    catch {
      invalid(t("cli.usage-app-cli.timezone_requires_a_valid_time_zone"));
    }
    scope.timezone = timezone;
  }
  for (const [flag, key] of [['agent', 'agentKind'], ['source', 'sourceInstanceId'], ['model', 'model'], ['effort', 'reasoningEffort'], ['project', 'project']] as const) {
    const value = values.get(flag);
    if (value)
      scope[key] = value;
  }
  if (unknownFlags.has('--model-unknown')) { if (scope.model) invalid(t("cli.usage-app-cli.model_unknown_cannot_be_combined_with")); scope.modelUnknown = true; }
  if (unknownFlags.has('--effort-unknown')) { if (scope.reasoningEffort) invalid(t("cli.usage-app-cli.effort_unknown_cannot_be_combined_with")); scope.effortUnknown = true; }
  if (unknownFlags.has('--project-unknown')) { if (scope.project) invalid(t('webui.projectConflict')); scope.projectUnknown = true; }
  const thread = values.get('thread');
  if (thread && action !== 'compare') {
    if (action === 'statistics')
      request.threadId = thread;
    else
      scope.threadId = thread;
  }
  if (unknownFlags.has('--family') && action !== 'compare') invalid(t('comparison.invalid'));
  if (action === 'compare') {
    if (thread || values.get('other-thread')) {
      if (!thread || !values.get('other-thread') || values.has('baseline-since') || values.has('baseline-until') || values.has('dimension')) invalid(t('comparison.invalid'));
      request.comparison = {kind:'sessions',leftThreadId:thread,rightThreadId:values.get('other-thread')!,includeDescendants:unknownFlags.has('--family')};
    } else {
      const baselineSince=values.get('baseline-since'),baselineUntil=values.get('baseline-until'),dimension=values.get('dimension')??'project';
      if (!help && (!baselineSince || !baselineUntil || !scope.since || !scope.until) || !['project','model','thread'].includes(dimension) || unknownFlags.has('--family')) invalid(t('comparison.invalid'));
      if (baselineSince && baselineUntil) request.comparison={kind:'periods',baselineSince,baselineUntil,dimension:dimension as 'project'|'model'|'thread'};
    }
  }
  if(action==='statistics' && (values.has('baseline-since') || values.has('baseline-until'))) {
    const baselineSince=values.get('baseline-since'),baselineUntil=values.get('baseline-until');
    if(!baselineSince||!baselineUntil||!scope.since||!scope.until)invalid(t('comparison.invalid'));
    request.comparison={kind:'periods',baselineSince,baselineUntil,dimension:'thread'};
  }
  const group = values.get('group');
  if (group) {
    if (!['day', 'week', 'month'].includes(group))
      invalid(t("cli.usage-app-cli.group_accepts_day_week_or_month"));
    request.group = group as UsageRequest['group'];
  }
  const presentation = values.get('presentation');
  if (presentation) { if (!['distribution', 'details', 'projects', 'models'].includes(presentation)) invalid(t('cli.usage-app-cli.presentation_invalid')); request.presentation = presentation as UsageRequest['presentation']; }
  const sort = values.get('sort');
  if (sort) {
    const choices = ['tokens','cost','time'];
    if (!choices.includes(sort))
      invalid(t("cli.usage-app-cli.sort_accepts_value", { p0: choices.join('、') }));
    request.sort = sort as UsageRequest['sort'];
  }
  for (const name of ['limit', 'offset'] as const) {
    const value = values.get(name);
    if (value) {
      const n = Number(value);
      if (!/^\d+$/.test(value) || !Number.isSafeInteger(n) || n < (name === 'limit' ? 1 : 0) || (name === 'limit' && n > 500))
        invalid(t("cli.usage-app-cli.value_is_out_of_range", { p0: name }));
      request[name] = n;
    }
  }
  if (values.get('snapshot'))
    request.snapshotId = values.get('snapshot');
  if (roots.length)
    request.roots = roots;
  if (Object.keys(scope).length)
    request.scope = scope;
  if (liveFlags.has('--fresh') && liveFlags.has('--cached')) invalid(t("cli.usage-app-cli.fresh_and_cached_cannot_be_combined"));
  if (liveFlags.has('--watch') && (action !== 'usage' || liveFlags.has('--cached') || request.snapshotId)) invalid(t("cli.usage-app-cli.watch_only_supports_live_usage_queries"));
  if (request.snapshotId && (liveFlags.has('--fresh') || roots.length)) invalid(t('cli.usage-app-cli.snapshot_cannot_be_combined_with_fresh'));
  return {request,json,help,version,...(waitText!==undefined?{waitMs:Number(waitText)}:{}),...(rebuild?{rebuild:true}:{}),...(liveFlags.has('--include-history')?{includeHistory:true}:{}),mode:waitText!==undefined||rebuild?'fresh':liveFlags.has('--cached')?'cached':liveFlags.has('--fresh')?'fresh':action==='compare'?'cached':'auto',watch:liveFlags.has('--watch')};
}
export { resultExitCode } from './exit-codes.js';
export async function runUsageCli(argv = process.argv.slice(2)): Promise<number> {
  const commandIndex = firstCommandIndex(argv);
  if (argv[commandIndex] === 'api' || argv[commandIndex] === 'call') {
    return (await import('./agent-cli.js')).runAgentCli(argv[commandIndex] as 'api'|'call', [...argv.slice(0, commandIndex), ...argv.slice(commandIndex + 1)]);
  }
  const json = argv.includes('--json');
  try {
    argv = configureLanguage(argv);
    const route = commandRoutes.get(argv[0]);
    if (route) return await route[0](argv.slice(1));
    const invocation = parseUsageArgs(argv);
    if (invocation.version) {
      process.stdout.write(invocation.json ? JSON.stringify({ outputVersion: 5, name: 'Wombat', version: packageMetadata.version }) + '\n' : 'Wombat ' + packageMetadata.version + '\n');
      return 0;
    }
    if (invocation.help) {
      process.stdout.write(invocation.json ? JSON.stringify({ outputVersion: 5, name: 'Wombat', commands: commandNames, help: usageHelp() }) + '\n' : usageHelp());
      return 0;
    }
    const client = createNodeClient();
    const controller = new AbortController();
    const stop = () => controller.abort();
    process.once('SIGINT', stop); process.once('SIGTERM', stop);
    try {
      let lastRevision = '';
      for (;;) {
        const queryOptions = {signal:controller.signal};
        const result = invocation.request.snapshotId && !invocation.request.snapshotId.startsWith('live:')
          ? await client.query(invocation.request, queryOptions)
          : (await client.live!({ query: invocation.request, mode: invocation.mode, waitMs:invocation.waitMs, rebuild:invocation.rebuild, includeHistory:invocation.includeHistory }, queryOptions)).result;
        const revision = JSON.stringify([result.snapshotRef.snapshotId, result.scope, result.freshness?.status, result.quality]);
        if (!invocation.watch || revision !== lastRevision) {
          if (!invocation.json && result.freshness && !['current', 'fixed'].includes(result.freshness.status)) process.stderr.write(`Wombat · ${result.freshness.status === 'failed' && result.freshness.error ? result.freshness.error : result.freshness.projectLoads?.length ? t('webui.projectsProgress',{ready:result.freshness.projectLoads.filter(p=>p.state==='ready').length,total:result.freshness.projectLoads.length}) : result.freshness.initialScan ? t('webui.initialTasks') : result.freshness.status === 'syncing' ? t("cli.usage-app-cli.syncing_showing_committed_data") : result.freshness.error ?? t("cli.usage-app-cli.showing_cached_data")}\n`);
          process.stdout.write(invocation.json ? JSON.stringify(result) + '\n' : (await import('./format.js')).renderUsageResult(result, process.stdout.columns ?? 120, invocation.request.group ?? undefined) + '\n');
          lastRevision = revision;
        }
        if (!invocation.watch) return resultExitCode(result);
        try { await delay(1_000, undefined, { signal: controller.signal }); }
        catch { throw new CoreError('CANCELLED', t("common.cancelled")); }
      }
    } finally { process.off('SIGINT', stop); process.off('SIGTERM', stop); }

  }
  catch (error) {
    const code = error instanceof CoreError ? error.code : 'INTERNAL_ERROR';
    const message = (error instanceof Error ? error.message : String(error));
    if (json)
      process.stdout.write(JSON.stringify({ outputVersion: commandRoutes.get(argv[0])?.[1] ?? 3, error: { code, message } }) + '\n');
    else
      process.stderr.write(`Wombat · ${message}\n`);
    return code === 'CANCELLED' ? 130 : 1;
  }
}
