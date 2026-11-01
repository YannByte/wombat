import path from 'node:path';
import {CoreError,type ArtifactsRequest,type ArtifactsResult} from '@wombat/client';
import {createNodeClient} from '@wombat/client/node';
import {t} from '@wombat/client/locale';
import {terminalText} from './display-text.js';

export function parseArtifactsArgs(argv:readonly string[]):{request:ArtifactsRequest;json:boolean;help:boolean}{
  let action:'inspect'|'recheck'|'apply'='inspect',json=false,help=false,project=process.cwd(),readView='',baselineId='',productTemporary=false,codex=false;
  const paths:string[]=[],keepPaths:string[]=[],selection:string[]=[],seen=new Set<string>();
  const args=[...argv];
  if(args[0]==='recheck'||args[0]==='apply'){action=args.shift() as 'recheck'|'apply';}
  const fail=():never=>{throw new CoreError('INVALID_ARGUMENT',t('artifacts.invalid'));};
  for(let i=0;i<args.length;i++){
    const [name,inline]=args[i].split(/=(.*)/s);
    if(name==='--help'||name==='-h'){if(inline!==undefined)fail();help=true;continue;}
    if(name==='--json'){if(json||inline!==undefined)fail();json=true;continue;}
    if(name==='--codex'){if(action!=='inspect'||codex||inline!==undefined)fail();codex=true;continue;}
    if(name==='--product-temporary'){if(action!=='inspect'||productTemporary||inline!==undefined)fail();productTemporary=true;continue;}
    const allowed=action==='inspect'?['--project-root','--path','--keep']:['--read-view','--baseline','--item'];
    if(!allowed.includes(name)||seen.has(name)&&!['--path','--keep','--item'].includes(name))fail();
    seen.add(name);
    const value=inline??args[++i];
    if(!value||value.startsWith('--')||value.includes('\0'))fail();
    switch(name){
      case '--project-root':project=path.resolve(value);break;
      case '--path':paths.push(value);break;
      case '--keep':keepPaths.push(value);break;
      case '--read-view':readView=value;break;
      case '--baseline':baselineId=value;break;
      case '--item':selection.push(value);break;
    }
  }
  if(codex&&productTemporary)fail();
  if(action!=='inspect'&&!help&&(!readView||!baselineId||action==='apply'&&!selection.length))fail();
  const request:ArtifactsRequest=action==='inspect'?{action,projectRoot:project,paths,keepPaths,productTemporary,codex}:action==='apply'?{action,readView,baselineId,selection}:{action,readView,baselineId,selection};
  return {request,json,help};
}
export function formatArtifactsText(result:ArtifactsResult):string{
  const lines=[t('artifacts.summary',{candidate:result.summary.candidates,review:result.summary.review,protected:result.summary.protected,unknown:result.summary.unknown,absent:result.summary.absent}),t('artifacts.bytes',{bytes:result.summary.observedLogicalBytes})];
  for(const item of result.items){
    lines.push(`${t(`artifacts.assessment.${item.assessment}`)}\t${terminalText(item.path)}`);
    if(item.observation)lines.push('  '+t('artifacts.bytes',{bytes:item.observation.logicalBytes}));
    const evidence=item.codexEvidence;
    if(evidence){
      if(evidence.currentRelease)lines.push('  '+t('artifacts.currentRelease',{value:terminalText(evidence.currentRelease)}));
      if(evidence.pendingRelease)lines.push('  '+t('artifacts.pendingRelease',{value:terminalText(evidence.pendingRelease)}));
      if(evidence.installedBuild!=null)lines.push('  '+t('artifacts.installedBuild',{value:evidence.installedBuild}));
      if(evidence.stagedBuild!=null)lines.push('  '+t('artifacts.stagedBuild',{value:evidence.stagedBuild}));
      if(evidence.minimumAgeDays!=null)lines.push('  '+t('artifacts.minimumAge',{value:evidence.minimumAgeDays}));
    }
    for(const reason of item.reasons)lines.push('  '+t(`artifacts.reason.${reason}`));
    if(item.recheck)lines.push('  '+t(`artifacts.recheck.${item.recheck}`));
    if(item.removal)lines.push('  '+t(`artifacts.removal.${item.removal}`));
  }
  if(result.partial)lines.push(t('artifacts.partial'));
  lines.push(t(result.scope?.codex?'artifacts.codexNote':'artifacts.note'));
  return lines.join('\n')+'\n';
}
export async function runArtifactsCli(argv:readonly string[]):Promise<number>{
  const {request,json,help}=parseArtifactsArgs(argv);
  if(help){process.stdout.write(t('artifacts.help'));return 0;}
  const controller=new AbortController(),stop=()=>controller.abort();
  process.once('SIGINT',stop);process.once('SIGTERM',stop);
  try{const result=await createNodeClient().artifacts!(request,{signal:controller.signal});process.stdout.write(json?JSON.stringify(result)+'\n':formatArtifactsText(result));return result.partial?2:0;}
  finally{process.off('SIGINT',stop);process.off('SIGTERM',stop);}
}
