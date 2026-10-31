import {setupExitCode} from './exit-codes.js';
import path from 'node:path';
import {CoreError,type SetupRequest} from '@wombat/client';
import {createNodeClient} from '@wombat/client/node';
import {t} from '@wombat/client/locale';
export async function runSetupCli(argv:string[]):Promise<number>{
  const r:SetupRequest={project:process.cwd()};let json=false;const seen=new Set<string>();
  const invalid=(value:string):never=>{throw new CoreError('INVALID_ARGUMENT',t('cli.config.invalid',{value}));};
  for(let i=0;i<argv.length;i++){const [flag,inline]=argv[i].split(/=(.*)/s);if(seen.has(flag)&&flag!=='--root')invalid(flag);seen.add(flag);
    if(flag==='--hooks'){if(inline!==undefined)invalid(flag);r.includeHooks=true;continue;}
    if(flag==='--help'||flag==='-h'){process.stdout.write(t('setup.help')+'\n');return 0;}if(flag==='--json'){if(inline!==undefined)invalid(flag);json=true;continue;}
    if(!['--root','--project'].includes(flag))invalid(flag);const value=inline??argv[++i];if(!value||value.startsWith('--'))invalid(flag);if(flag==='--root')(r.roots??=[]).push(path.resolve(value));else r.project=path.resolve(value);
  }
  const controller=new AbortController(),stop=()=>controller.abort();process.once('SIGINT',stop);process.once('SIGTERM',stop);
  try{const result=await createNodeClient({automaticPrices:false}).setup!(r,{signal:controller.signal});
    if(json)process.stdout.write(JSON.stringify(result)+'\n');else{
      process.stdout.write(t('setup.runtimeVersion',{version:result.runtimeVersion??'—'})+'\n'+t('setup.summary',{version:result.nativeVersion??'—',skill:t(`skill.${result.discovery.status==='selection_changed'?'unavailable':result.discovery.status}`)})+'\n');
      for(const instance of result.discovery.instances.filter(i=>i.enabled))process.stdout.write('$'+instance.name+'\n');
      for(const check of result.runtimeChecks??[])process.stdout.write(t('setup.runtimeCheck',{version:check.pluginVersion??'—',status:t(`setup.runtime.${check.status}`)})+'\n');
      for(const code of result.errorCodes)process.stdout.write(t('setup.errorLine',{code})+'\n');
      process.stdout.write(t('setup.logsReadyNote')+'\n'+t('setup.registrationNote')+'\n');
    }
    return setupExitCode(result);
  }finally{process.off('SIGINT',stop);process.off('SIGTERM',stop);}
}
