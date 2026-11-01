import {writeHelp} from './help.js';
import {CoreError} from '@wombat/client';
import {t,locale} from '@wombat/client/locale';
/** Local host operations only; product queries use generated calls. */
export async function runSystemCli(argv:string[]):Promise<number>{
  argv=argv.filter(a=>a!=='--no-input');
  const [action,...args]=argv;
  if(!action||action==='--help'||action==='-h'){writeHelp('system',argv);return 0;}
  if(['doctor','update','hook'].includes(action)&&(args.includes('--help')||args.includes('-h'))){writeHelp('system',argv);return 0;}
  if(action==='doctor')return (await import('./doctor-cli.js')).runDoctorCli(args);
  if(action==='update')return (await import('./update-cli.js')).runUpdateCli(args);
  if(action==='hook')return (await import('./hook-input.js')).runHookCli(args);
  if(action==='api'||action==='call')return (await import('./agent-cli.js')).runAgentCli(action,[...args,'--lang',locale.getLocale()]);
  throw new CoreError('INVALID_ARGUMENT',t('cli.system.help'));
}
