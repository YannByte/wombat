import {writeHelp} from './help.js';
import path from 'node:path';
import { commandRoutes } from './command-routing.js';
import {dateWindow, isTimeWord, shiftDate} from './calendar.js';
import {parseOptions,pageNumber,invalidInput,projectFilter} from './human-input.js';
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
  rebuild?: boolean;
  includeHistory?: boolean;
}
const valued=['--root','--agent','--source','--timezone','--since','--until','--model','--effort','--project','--task','--against','--group','--by','--sort','--limit','--offset','--baseline-since','--baseline-until'];
function invalid(message:string):never {throw new CoreError('INVALID_ARGUMENT',message);}
export function parseUsageArgs(input:string[],now=new Date()):Invocation {
  const argv=[...input];
  if(argv[0]==='usage')argv.shift();
  let action:UsageRequest['action']='usage';
  if(argv[0]==='stats'||argv[0]==='compare'||argv[0]==='top'){const name=argv.shift();action=name==='stats'?'statistics':name==='top'?'threads':'compare';}
  const word=isTimeWord(argv[0])?argv.shift() as import('./calendar.js').TimeWord:undefined;
  const p=parseOptions(argv,valued,['--fresh','--cached','--watch','--rebuild','--include-history','--version','--all-time','--undated','--model-unknown','--effort-unknown','--project-unknown','--family','--compact'],['--root']);
  if(p.positionals.length)invalidInput(p.positionals[0]);
  const version=p.flags.has('--version');
  if(version&&argv.some(a=>!['--version','--json'].includes(a)))invalidInput('--version');
  const request:UsageRequest={action,...(p.json?{compact:true}:{})};
  const flag=(name:string)=>p.flags.has(name), value=(name:string)=>p.get(name);
  if(flag('--compact')){if(!p.json)invalidInput('--compact requires --json');request.compact=true;}
  if(p.help||version)return {request,json:p.json,help:p.help,version,mode:'auto',watch:false};
  const session=action==='compare'&&(p.values.has('--task')||p.values.has('--against'));
  const explicitBaseline=p.values.has('--baseline-since')||p.values.has('--baseline-until');
  const scope:NonNullable<UsageRequest['scope']>={...dateWindow({since:value('--since'),until:value('--until'),timezone:value('--timezone'),allTime:flag('--all-time')||session&&!value('--since')&&!value('--until')||undefined,undated:flag('--undated')||undefined},action==='statistics'?'recent':action==='compare'?'compare':'today',word,now)};
  for(const [name,key]of [['--agent','agentKind'],['--source','sourceInstanceId'],['--model','model'],['--effort','reasoningEffort'],['--project','project']] as const)if(value(name))scope[key]=value(name);
  if(scope.project)scope.project=projectFilter(scope.project);
  for(const [name,key,conflict]of [['--model-unknown','modelUnknown','--model'],['--effort-unknown','effortUnknown','--effort'],['--project-unknown','projectUnknown','--project']] as const)if(flag(name)){if(value(conflict))invalidInput(name);scope[key]=true;}
  if(value('--task')&&action!=='compare'){if(action==='statistics')request.threadId=value('--task');else scope.threadId=value('--task');}
  if(action==='compare') {
    if(session){
      if(!value('--task')||!value('--against')||explicitBaseline||value('--by'))invalidInput('compare');
      request.comparison={kind:'sessions',leftThreadId:value('--task')!,rightThreadId:value('--against')!,includeDescendants:flag('--family')};
    }else{
      if(flag('--family')||flag('--all-time')||flag('--undated')||!explicitBaseline&&(value('--since')||value('--until')||word))invalidInput('compare');
      const by=value('--by')??'project';if(!['project','model','task'].includes(by))invalidInput('--by');
      const baselineSince=value('--baseline-since'),baselineUntil=value('--baseline-until');
      if(explicitBaseline&&(!baselineSince||!baselineUntil||!value('--since')||!value('--until')))invalidInput('compare dates');
      request.comparison={kind:'periods',baselineSince:baselineSince??shiftDate(scope.since!,-7),baselineUntil:baselineUntil??scope.since!,dimension:by==='task'?'thread':by as 'project'|'model'};
    }
  }else{
    if(value('--against')||flag('--family')||explicitBaseline)invalidInput('comparison options');
    if(value('--by')){if(action==='threads'||!['project','model'].includes(value('--by')!))invalidInput('--by');request.presentation=value('--by')==='model'?'models':'projects';}
  }
  if(action==='usage')request.presentation??='distribution';
  const group=value('--group');if(group){if(action!=='usage'||!['day','week','month'].includes(group))invalidInput('--group');request.group=group as UsageRequest['group'];}
  const sort=value('--sort');if(sort){if(!['usage','threads'].includes(action)||!(action==='threads'?['tokens','cost']:['tokens','cost','time']).includes(sort))invalidInput('--sort');request.sort=sort as UsageRequest['sort'];}
  if(action==='threads')request.sort??='tokens';
  request.scope=scope;
  if(session&&(p.values.has('--limit')||p.values.has('--offset')))invalidInput('session comparison pagination');
  if(!session)request.limit=pageNumber(value('--limit'),action==='threads'?5:20,1,200);
  if(value('--offset'))request.offset=pageNumber(value('--offset'),0);
  if(p.all('--root').length)request.roots=p.all('--root').map(root=>path.resolve(root));
  const rebuild=flag('--rebuild'),watch=flag('--watch');
  if(flag('--fresh')&&flag('--cached')||rebuild&&(flag('--cached')||watch)||watch&&(action!=='usage'||flag('--cached')))invalidInput('query mode');
  return {request,json:p.json,help:false,version:false,mode:rebuild||flag('--fresh')?'fresh':flag('--cached')||action==='compare'?'cached':'auto',watch,...(rebuild?{rebuild:true}:{}),...(flag('--include-history')?{includeHistory:true}:{})};
}
export { resultExitCode } from './exit-codes.js';
export async function runUsageCli(argv = process.argv.slice(2)): Promise<number> {
  const json = argv.includes('--json');
  try {
    argv = configureLanguage(argv);
    const route = commandRoutes.get(argv[0]);
    if (route) return await route[0](argv.slice(1));
    if (!argv.length || argv.every(a=>['--help','-h','--json','--no-input'].includes(a))) {parseOptions(argv,[]);writeHelp('root',argv);return 0;}
    if (argv[0] === 'usage' && argv[1] === 'alerts') return (await import('./monitor-watch.js')).runMonitorWatch(argv.slice(2).filter(a=>a!=='--no-input'));
    if (argv[0] !== 'usage' && !argv.includes('--version')) invalid(t('cli.config.invalid',{value:argv[0]}));
    const invocation = parseUsageArgs(argv);
    if (invocation.version) {
      process.stdout.write(invocation.json ? JSON.stringify({ outputVersion: 5, name: 'Wombat', version: packageMetadata.version }) + '\n' : 'Wombat ' + packageMetadata.version + '\n');
      return 0;
    }
    if(invocation.help){writeHelp('usage',argv);return 0;}

    const client = createNodeClient();
    const controller = new AbortController();
    const stop = () => controller.abort();
    process.once('SIGINT', stop); process.once('SIGTERM', stop);
    try {
    const requestedTask=invocation.request.threadId??invocation.request.scope?.threadId;
    if(requestedTask||invocation.request.comparison?.kind==='sessions'){
      const {resolveRecord}=await import('./history-cli.js');
      const select=async(record:string)=>(await resolveRecord(client,record,invocation.request.scope??{},invocation.request.roots??undefined,argv,controller.signal)).thread.id;
      if(requestedTask){const id=await select(requestedTask);if(invocation.request.threadId)invocation.request.threadId=id;else invocation.request.scope!.threadId=id;}
      const comparison=invocation.request.comparison;
      if(comparison?.kind==='sessions'){comparison.leftThreadId=await select(comparison.leftThreadId);comparison.rightThreadId=await select(comparison.rightThreadId);}
    }
      let lastRevision = '';
      for (;;) {
        const queryOptions = {signal:controller.signal};
        const result = (await client.live!({ query: invocation.request, mode: invocation.mode, rebuild:invocation.rebuild, includeHistory:invocation.includeHistory }, queryOptions)).result;
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
      process.stdout.write(JSON.stringify({ outputVersion: commandRoutes.get(argv[0])?.[1] ?? 3, error: { code, message, ...(error instanceof CoreError && error.details !== undefined ? {details:error.details} : {}) } }) + '\n');
    else
      process.stderr.write(`Wombat · ${message}\n`);
    return code === 'CANCELLED' ? 130 : 1;
  }
}
