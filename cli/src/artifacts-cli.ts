import {writeHelp} from './help.js';
import path from 'node:path';
import {CoreError,type ArtifactsRequest,type ArtifactsResult} from '@wombat/client';
import {createNodeClient} from '@wombat/client/node';
import {t} from '@wombat/client/locale';
import {ask,canPrompt,invalidInput,pageNumber,projectDirectory,withCancellation} from './human-input.js';
import {terminalText} from './display-text.js';

export function parseArtifactsArgs(argv:readonly string[]):{request:ArtifactsRequest;json:boolean;help:boolean;guided:boolean}{
  let action:'inspect'|'recheck'|'apply'='inspect',json=false,help=false,project=process.cwd(),readView='',baselineId='',productTemporary=false,codex=false,apply=false;
  const paths:string[]=[],keepPaths:string[]=[],selection:string[]=[],seen=new Set<string>();
  const args=[...argv];
  if(args[0]==='verify'){action='recheck';args.shift();}
  const fail=():never=>{throw new CoreError('INVALID_ARGUMENT',t('artifacts.invalid'));};
  for(let i=0;i<args.length;i++){
    const [name,inline]=args[i].split(/=(.*)/s);
    if(seen.has(name)&&!['--path','--keep','--item'].includes(name))fail();
    seen.add(name);
    if(name==='--help'||name==='-h'){if(inline!==undefined)fail();help=true;continue;}
    if(name==='--json'){if(inline!==undefined)fail();json=true;continue;}
    if(name==='--no-input'){if(inline!==undefined)fail();continue;}
    if(name==='--apply'){if(action!=='inspect'||inline!==undefined)fail();apply=true;continue;}
    if(name==='--codex'){if(action!=='inspect'||inline!==undefined)fail();codex=true;continue;}
    if(name==='--wombat-temp'){if(action!=='inspect'||inline!==undefined)fail();productTemporary=true;continue;}
    if(!['--project','--path','--keep','--read-view','--baseline','--item'].includes(name))fail();
    const value=inline??args[++i];
    if(!value||value.startsWith('--')||value.includes('\0'))fail();
    switch(name){
      case '--project':project=path.resolve(value);break;
      case '--path':paths.push(value);break;
      case '--keep':keepPaths.push(value);break;
      case '--read-view':readView=value;break;
      case '--baseline':baselineId=value;break;
      case '--item':selection.push(value);break;
    }
  }
  if(codex&&(productTemporary||seen.has('--project')))fail();
  const guided=apply&&!readView&&!baselineId;
  if(apply&&!guided)action='apply';
  if(action!=='inspect'&&['--project','--path','--keep','--codex','--wombat-temp'].some(f=>seen.has(f)))fail();
  if(action==='inspect'&&(readView||baselineId||selection.length))fail();
  if(guided&&!codex)fail();
  if(action!=='inspect'&&!help&&(!readView||!baselineId||action==='apply'&&!selection.length))fail();
  const request:ArtifactsRequest=action==='inspect'?{action,projectRoot:project,paths,keepPaths,productTemporary,codex}:{action,readView,baselineId,selection};
  return {request,json,help,guided};
}
export function formatArtifactsText(result:ArtifactsResult):string{
  const lines=[...(result.action==='inspect'?[t('cli.clean.preview')]:[]),t('artifacts.summary',{candidate:result.summary.candidates,review:result.summary.review,protected:result.summary.protected,unknown:result.summary.unknown,absent:result.summary.absent}),t('artifacts.bytes',{bytes:result.summary.observedLogicalBytes})];
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
  return withCancellation(async signal=>{
    const {request,json,help,guided}=parseArtifactsArgs(argv);
    if(help){writeHelp('clean',argv);return 0;}
    if(guided&&!canPrompt(argv))invalidInput(t('cli.clean.selectionRequired'));
    if(request.action==='inspect'&&!request.codex){
      const explicit=argv.some(a=>a==='--project'||a.startsWith('--project='));
      request.projectRoot=await projectDirectory(explicit?request.projectRoot:undefined,argv,signal);
    }
    const client=createNodeClient({automaticPrices:false});
    const result=await client.artifacts!(request,{signal});
    if(guided){
      process.stdout.write(formatArtifactsText(result));
      const choices=result.items.filter(i=>i.assessment==='candidate');
      if(!choices.length)return result.partial?2:0;
      process.stderr.write(choices.map((i,n)=>`${n+1}. ${terminalText(i.path)}`).join('\n')+'\n');
      const answer=await ask(t('cli.clean.choose'),signal);
      if(!answer)throw new CoreError('CANCELLED',t('common.cancelled'));
      const selected=[...new Set(answer.split(/[,\s]+/).filter(Boolean).map(n=>choices[pageNumber(n,0,1,choices.length)-1].id))];
      if(!selected.length)throw new CoreError('CANCELLED',t('common.cancelled'));
      const yes=await ask(t('cli.clean.confirm',{count:selected.length}),signal);
      if(!['yes','确认'].includes(yes.toLowerCase()))throw new CoreError('CANCELLED',t('common.cancelled'));
      const removed=await client.artifacts!({action:'apply',readView:result.readView,baselineId:result.baselineId,selection:selected},{signal});
      process.stdout.write(formatArtifactsText(removed));
      const checked=await client.artifacts!({action:'recheck',readView:result.readView,baselineId:result.baselineId,selection:selected},{signal});
      process.stdout.write(formatArtifactsText(checked));
      return removed.partial||checked.partial?2:0;
    }
    process.stdout.write(json?JSON.stringify(result)+'\n':formatArtifactsText(result));
    if(!json&&request.action==='inspect')process.stdout.write('wombat clean verify --read-view '+JSON.stringify(result.readView)+' --baseline '+JSON.stringify(result.baselineId)+'\n');
    return result.partial?2:0;
  });
}
