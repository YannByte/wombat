import { configureLanguage } from './locale.js';
import { t, progressText } from '@wombat/client/locale';
import { setTimeout as delay } from 'node:timers/promises';
import packageMetadata from '../package.json' with { type: 'json' };
import { CoreError, type UsageRequest, type UsageResult } from '@wombat/client';
import { createNodeClient } from '@wombat/client/node';
export function usageHelp(): string { return (t("statistics.help") + "\n" + t("monitor.help") + "\n" + t("cli.usage-app-cli.help") + "\n" + t("comparison.help") + "\n" + t("inspection.help") + "\n").replace('  wombat optimize', `${t('cli.update.summary')}\n${t('skill.summary')}\n${t('setup.summaryHelp')}\n${t('collection.summaryHelp')}\n${t('cli.doctor.summary')}\n${t('cli.timing.summary')}\n  wombat optimize`); }
export interface Invocation {
  request: UsageRequest;
  json: boolean;
  help: boolean;
  version: boolean;
  mode: 'auto' | 'fresh' | 'cached';
  watch: boolean;
  verify: boolean;
}
const actions = new Set(['refresh', 'usage', 'threads', 'turns', 'steps', 'compare', 'investigate', 'trajectory', 'resources', 'review', 'context', 'statistics']);
const valued = new Set(['root', 'snapshot', 'agent', 'source', 'timezone', 'since', 'until', 'model', 'effort', 'project', 'thread', 'turn', 'group', 'presentation', 'sort', 'search', 'limit', 'offset', 'locate-thread', 'locate-turn', 'locate-operation', 'baseline-since', 'baseline-until', 'dimension', 'other-thread']);
let knownTimezones: Set<string> | undefined;
function invalid(message: string): never { throw new CoreError('INVALID_ARGUMENT', message); }
export function parseUsageArgs(argv: string[]): Invocation {
  let action: UsageRequest['action'] = 'usage';
  let explicit = false;
  let json = false;
  let help = false;
  let version = false;
  const liveFlags = new Set<string>();
  const unknownFlags = new Set<string>();
  const values = new Map<string, string>();
  const roots: string[] = [];
  for (let i = 0;i < argv.length;i++) {
    const arg = argv[i];
    if (['--fresh', '--cached', '--watch', '--verify'].includes(arg)) { if (liveFlags.has(arg)) invalid(t("common.value_cannot_be_repeated", { p0: arg })); liveFlags.add(arg); continue; }
    if (arg === '--version' || arg === '-v') {
      version = true;
      continue;
    }
    if (arg === 'help' || arg === '--help' || arg === '-h') {
      help = true;
      continue;
    }
    if (['--all-time', '--model-unknown', '--effort-unknown', '--undated', '--project-unknown', '--matched-only', '--family'].includes(arg)) { if (unknownFlags.has(arg)) invalid(t("common.value_cannot_be_repeated", { p0: arg })); unknownFlags.add(arg); continue; }
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
    if (!arg.startsWith('--')) {
      if (explicit || !actions.has(arg))
        invalid(t("cli.usage-app-cli.unknown_command_value", { p0: arg }));
      action = arg as UsageRequest['action'];
      explicit = true;
      continue;
    }
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
  if (action === 'refresh' && unknownFlags.size) invalid(t("cli.usage-app-cli.refresh_does_not_support_query_filters"));
  const request: UsageRequest = { action };
  if (liveFlags.has('--compact')) {
    if (!json || action === 'refresh') invalid(t('cli.usage-app-cli.compactRequiresJson'));
    request.compact = true;
  }
  const scope: NonNullable<UsageRequest['scope']> = {};
  const allowed = action === 'refresh' ? new Set(['root']) : new Set(['root', 'snapshot', 'agent', 'source', 'timezone', 'since', 'until', 'model', 'effort', 'project', 'thread', 'limit', 'offset', ...(action === 'compare' ? ['baseline-since', 'baseline-until', 'dimension', 'other-thread'] : []), ...(action === 'usage' ? ['group', 'presentation', 'sort'] : []), ...(action === 'statistics' ? ['presentation','baseline-since','baseline-until'] : []), ...(action === 'threads' ? ['sort', 'search', 'locate-thread'] : []), ...(action === 'turns' || action === 'steps' ? ['sort'] : []), ...(action === 'steps' ? ['turn','locate-operation'] : []), ...(action === 'investigate' ? ['turn'] : []), ...(action === 'turns' ? ['locate-turn'] : [])]);
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
  if (values.get('locate-thread')) request.locateThreadId = values.get('locate-thread');
  if(values.get('locate-operation'))request.locateOperationId=values.get('locate-operation');
  if (values.get('locate-turn')) request.locateTurnId = values.get('locate-turn');
  if (unknownFlags.has('--matched-only')) {
    if (action !== 'turns') invalid(t('cli.usage-app-cli.value_does_not_support_value', {p0:action,p1:'matched-only'}));
    request.matchedOnly = true;
  }
  const thread = values.get('thread');
  if (thread && action !== 'compare') {
    if (action === 'turns' || action === 'steps' || action === 'statistics')
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
  const turn = values.get('turn');
  if (turn) {
    if (action === 'investigate') {
      if (!thread) invalid(t('cli.usage-app-cli.value_requires_thread_id',{p0:action}));
      scope.turnId = turn;
    } else request.turnId = turn;
  }
  if (!help && (action === 'turns' || action === 'steps') && !request.threadId)
    invalid(t("cli.usage-app-cli.value_requires_thread_id", { p0: action }));
  if(!help&&action==='trajectory'&&!thread)invalid(t('cli.usage-app-cli.value_requires_thread_id',{p0:action}));
  if(!help&&action==='review'&&(scope.allTime||scope.undated||values.has('limit')||values.has('offset')||!!scope.since!==!!scope.until))invalid(t('cli.config.invalid',{value:action}));
  if (!help && action === 'steps' && !request.turnId)
    invalid(t("cli.usage-app-cli.steps_requires_turn_id"));
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
    const choices = action === 'threads' ? ['tokens', 'cost', 'recent'] : action === 'turns' ? ['tokens', 'cost', 'time', 'recent'] : ['tokens', 'cost', 'time'];
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
  if (values.get('search'))
    request.search = values.get('search');
  if (roots.length)
    request.roots = roots;
  if (Object.keys(scope).length)
    request.scope = scope;
  if (liveFlags.has('--fresh') && liveFlags.has('--cached')) invalid(t("cli.usage-app-cli.fresh_and_cached_cannot_be_combined"));
  if (liveFlags.has('--watch') && (action !== 'usage' || liveFlags.has('--cached') || request.snapshotId)) invalid(t("cli.usage-app-cli.watch_only_supports_live_usage_queries"));
  if (liveFlags.has('--verify') && action !== 'refresh') invalid(t("cli.usage-app-cli.verify_only_supports_refresh"));
  if (request.snapshotId && (liveFlags.has('--fresh') || roots.length)) invalid(t("cli.usage-app-cli.snapshot_cannot_be_combined_with_fresh"));
  if (action === 'refresh' && liveFlags.has('--cached')) invalid(t("cli.usage-app-cli.refresh_cannot_use_cached"));
  return { request, json, help, version, mode: liveFlags.has('--cached') ? 'cached' : liveFlags.has('--fresh') ? 'fresh' : ['compare','investigate','trajectory','resources','review','context'].includes(action) ? 'cached' : 'auto', watch: liveFlags.has('--watch'), verify: liveFlags.has('--verify') };
}
export function resultExitCode(result: UsageResult): number { return result.quality.status === 'partial' || result.inspection?.partial || (result.freshness && !['current', 'fixed'].includes(result.freshness.status)) ? 2 : 0; }
export async function runUsageCli(argv = process.argv.slice(2)): Promise<number> {
  // Timing owns its default JSON and safe error envelope, including language errors.
  let commandIndex = 0;
  while (argv[commandIndex] === '--lang' || argv[commandIndex]?.startsWith('--lang=')) {
    commandIndex += argv[commandIndex] === '--lang' ? 2 : 1;
  }
  if (argv[commandIndex] === 'timing') {
    return (await import('./timing-cli.js')).runTimingCli([...argv.slice(0, commandIndex), ...argv.slice(commandIndex + 1)]);
  }
  if (argv[commandIndex] === 'api' || argv[commandIndex] === 'call') {
    return (await import('./agent-cli.js')).runAgentCli(argv[commandIndex] as 'api'|'call', [...argv.slice(0, commandIndex), ...argv.slice(commandIndex + 1)]);
  }
  const json = argv.includes('--json');
  try {
    argv = configureLanguage(argv);
    if (argv[0] === 'monitor') return await (await import('./monitor-cli.js')).runMonitorCli(argv.slice(1));
    if (argv[0] === 'setup') return await (await import('./setup-cli.js')).runSetupCli(argv.slice(1));
    if (argv[0] === 'collection') return await (await import('./collection-cli.js')).runCollectionCli(argv.slice(1));
    if (argv[0] === 'hook') return await (await import('./collection-cli.js')).runHookCli(argv.slice(1));
    if (argv[0] === 'account') return await (await import('./account-cli.js')).runAccountCli(argv.slice(1));
    if (argv[0] === 'directories') return await (await import('./directories-cli.js')).runDirectoriesCli(argv.slice(1));
    if (argv[0] === 'prices') return await (await import('./prices-cli.js')).runPricingCli(argv.slice(1));
    if (argv[0] === 'optimize') return await (await import('./config-cli.js')).runConfigCli(argv.slice(1));
    if (argv[0] === 'skill') return (await import('./skill-cli.js')).runSkillCli(argv.slice(1));
    if (argv[0] === 'web') return await (await import('./web-cli.js')).runWebCli(argv.slice(1));
    if (argv[0] === 'update') return await (await import('./update-cli.js')).runUpdateCli(argv.slice(1));
    if (argv[0] === 'doctor') return (await import('./doctor-cli.js')).runDoctorCli(argv.slice(1));
    const invocation = parseUsageArgs(argv);
    if (invocation.version) {
      process.stdout.write(invocation.json ? JSON.stringify({ outputVersion: 5, name: 'Wombat', version: packageMetadata.version }) + '\n' : 'Wombat ' + packageMetadata.version + '\n');
      return 0;
    }
    if (invocation.help) {
      process.stdout.write(invocation.json ? JSON.stringify({ outputVersion: 5, name: 'Wombat', commands: ['refresh', 'usage', 'threads', 'turns', 'steps', 'prices', 'web', 'optimize', 'directories', 'account', 'update', 'doctor', 'timing', 'compare', 'investigate', 'trajectory', 'resources', 'review', 'context', 'skill', 'collection', 'hook', 'setup', 'api', 'call', 'statistics', 'monitor'], help: usageHelp() }) + '\n' : usageHelp());
      return 0;
    }
    const client = createNodeClient();
    const controller = new AbortController();
    const stop = () => controller.abort();
    process.once('SIGINT', stop); process.once('SIGTERM', stop);
    try {
      let lastRevision = '';
      for (;;) {
        const queryOptions = { signal: controller.signal, onProgress: (stage: string) => {
          if (invocation.request.action === 'refresh') process.stderr.write(`Wombat · ${progressText(stage)}\n`);
        } };
        const result = invocation.request.snapshotId && !invocation.request.snapshotId.startsWith('live:')
          ? await client.query(invocation.request, queryOptions)
          : (await client.live!({ query: invocation.request, mode: invocation.mode, verify: invocation.verify }, queryOptions)).result;
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
    const code = error instanceof CoreError ? error.code : error instanceof Error && ['ExitPromptError', 'AbortPromptError'].includes(error.name) ? 'CANCELLED' : 'INTERNAL_ERROR';
    const message = (error instanceof Error ? error.message : String(error));
    if (json)
      process.stdout.write(JSON.stringify({ outputVersion: ['prices', 'optimize', 'account', 'update', 'doctor', 'skill', 'web', 'collection', 'hook', 'setup', 'api', 'call'].includes(argv[0]) ? 1 : 3, error: { code, message } }) + '\n');
    else
      process.stderr.write(`Wombat · ${message}\n`);
    return code === 'CANCELLED' ? 130 : 1;
  }
}
