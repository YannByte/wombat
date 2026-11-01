import {CoreError} from '@wombat/client';
import {t} from '@wombat/client/locale';
/** Local host operations only; product queries use generated calls. */
export async function runSystemCli(argv:string[]):Promise<number>{
  const [action,...args]=argv;
  if(!action||action==='--help'||action==='-h'){process.stdout.write(t('cli.system.help')+'\n');return 0;}
  if(action==='doctor'){
    if(args.filter(a=>a==='--plugin').length>1)throw new CoreError('INVALID_ARGUMENT',t('cli.config.invalid',{value:'--plugin'}));
    if(args.includes('--plugin')||args.includes('--hooks')){
      return (await import('./setup-cli.js')).runSetupCli(args.filter(a=>a!=='--plugin'));
    }
    return (await import('./doctor-cli.js')).runDoctorCli(args);
  }
  if(action==='update')return (await import('./update-cli.js')).runUpdateCli(args);
  if(action==='hook')return (await import('./hook-input.js')).runHookCli(args);
  if(action==='watch')return (await import('./monitor-watch.js')).runMonitorWatch(args);
  throw new CoreError('INVALID_ARGUMENT',t('cli.system.help'));
}
