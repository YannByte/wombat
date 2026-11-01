import {writeHelp} from './help.js';
import path from 'node:path';
import {CoreError,type UsageClient} from '@wombat/client';
import {createNodeClient} from '@wombat/client/node';
import {t} from '@wombat/client/locale';
import {setTimeout as delay} from 'node:timers/promises';
export async function checkMonitor(
  client: UsageClient,
  roots: string[],
  ids: string[] | undefined,
  signal: AbortSignal,
) {
  if (!client.monitor || !client.live)
    throw new CoreError('MONITOR_UNAVAILABLE', t('monitor.unavailable'));
  const settings = await client.monitor({ action: 'list' }, { signal });
  const selected = ids ?? settings.plans.filter((p) => p.enabled).map((p) => p.id);
  if (selected.some((id) => !settings.plans.some((p) => p.id === id)))
    throw new CoreError('NOT_FOUND', t('monitor.invalid'));
  if (!selected.length) return { ...settings, notifications: [] };
  const live = await client.live(
    {
      query: { action: 'usage', roots, scope: { allTime: true }, limit: 1, compact: true },
      mode: 'fresh',
    },
    { signal },
  );
  if (!['current', 'fixed'].includes(live.freshness.status) || live.freshness.initialScan)
    throw new CoreError('SYNC_PENDING', t('monitor.pending'));
  return client.monitor(
    { action: 'check', snapshotId: live.result.snapshotRef.snapshotId, ids: selected },
    { signal },
  );
}
type MonitorResult = Awaited<ReturnType<NonNullable<UsageClient['monitor']>>>;
const retryableWatchErrors = new Set([
  'SYNC_PENDING',
  'TIMEOUT',
  'VIEW_EXPIRED',
  'TRANSPORT_ERROR',
  'RESOURCE_LIMIT',
  'INDEX_UNAVAILABLE',
  'SOURCE_UNREADABLE',
  'MONITOR_CHANGED',
]);

export async function runMonitorChecks(
  client: UsageClient,
  options: {roots:string[];ids?:string[];watch:boolean;interval:number},
  signal: AbortSignal,
  print: (result: MonitorResult) => void,
  onRetry: (error: CoreError) => void,
): Promise<number> {
  try {
    while (!signal.aborted) {
      try {
        const result = await checkMonitor(client, options.roots, options.ids, signal);
        if (!result.plans.some(p=>p.enabled && (!options.ids || options.ids.includes(p.id)))) {print(result);return 2;}
        if (!options.watch || result.notifications.length) print(result);
        if (!options.watch) return result.notifications.some((n) => n.partial) ? 2 : 0;
      } catch (error) {
        if (signal.aborted) break;
        if (
          !options.watch ||
          !(error instanceof CoreError) ||
          !retryableWatchErrors.has(error.code)
        )
          throw error;
        onRetry(error);
      }
      await delay(options.interval * 1000, undefined, { signal });
    }
    return 130;
  } catch (error) {
    if (signal.aborted) return 130;
    throw error;
  }
}


export async function runMonitorWatch(argv:string[]):Promise<number>{
  if(argv.includes('--help')||argv.includes('-h')){writeHelp('alerts',argv);return 0;}
  const roots:string[]=[],ids:string[]=[];let interval=60,once=false;const seen=new Set<string>();
  for(let i=0;i<argv.length;i++){
    const [flag,inline]=argv[i].split(/=(.*)/s);if(seen.has(flag)&&!['--root','--id'].includes(flag))throw new CoreError('INVALID_ARGUMENT',t('monitor.invalid'));seen.add(flag);
    if(flag==='--once'&&inline===undefined){once=true;continue;}if(flag==='--json'&&inline===undefined)continue;
    if(!['--root','--id','--interval'].includes(flag)||(inline===undefined&&(!argv[i+1]||argv[i+1].startsWith('-'))))throw new CoreError('INVALID_ARGUMENT',t('monitor.invalid'));
    const value=inline??argv[++i];if(!value)throw new CoreError('INVALID_ARGUMENT',t('monitor.invalid'));if(flag==='--root')roots.push(path.resolve(value));else if(flag==='--id')ids.push(value);else{interval=Number(value);if(!/^\d+$/.test(value)||interval<5||interval>3600)throw new CoreError('INVALID_ARGUMENT',t('monitor.invalid'));}
  }
  const controller=new AbortController(),stop=()=>controller.abort();process.once('SIGINT',stop);process.once('SIGTERM',stop);
  try{return await runMonitorChecks(createNodeClient(),{roots,ids:ids.length?ids:undefined,interval,watch:!once},controller.signal,r=>process.stdout.write(argv.includes('--json')?JSON.stringify(r)+'\n':!r.plans.some(p=>p.enabled&&(!ids.length||ids.includes(p.id)))?t('cli.alerts.noPlans')+'\n':t('cli.alerts.result',{count:r.notifications.length})+'\n'+r.notifications.map(n=>`${n.planId} · ${n.kind} · ${n.checkedAt}`).join('\n')+'\n'),e=>process.stderr.write(JSON.stringify({error:{code:e.code,message:t('monitor.checkFailed')}})+'\n'));}
  finally{process.off('SIGINT',stop);process.off('SIGTERM',stop);}
}
