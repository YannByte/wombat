import {writeHelp} from './help.js';
import path from 'node:path';
import {CoreError,validateConfigRequest,type ProjectResult, type ConfigRequest} from '@wombat/client';
import {createNodeClient} from '@wombat/client/node';
import {t} from '@wombat/client/locale';
import {dateWindow} from './calendar.js';
import {ask,canPrompt,commandLine,invalidInput, pageNumber, parseOptions, projectDirectory, projectFilter, withCancellation} from './human-input.js';
import {readBaseline} from './read-baseline.js';
import {formatConfigText} from './config-cli.js';
import {formatOptimizeText,runVerificationCli} from './optimize-cli.js';
import {formatArtifactsText} from './artifacts-cli.js';
import {terminalText} from './display-text.js';
import {configExitCode} from './exit-codes.js';
import {resolveRecord} from './history-cli.js';

export function formatProjectText(result:ProjectResult):string {
  const lines=[t('cli.project.title'),terminalText(result.scope.projectRoot),`${result.scope.since} .. ${result.scope.until} · ${result.scope.timezone}`,t('cli.project.readOnly')];
  const labels={configuration:'cli.project.configuration',checks:'cli.project.checks',history:'cli.project.history',records:'cli.project.records',artifacts:'cli.project.artifacts'} as const;
  const states={complete:'cli.project.state.complete',partial:'cli.project.state.partial',unavailable:'cli.project.state.unavailable'} as const;
  for (const key of ['configuration','checks','history','records','artifacts'] as const) {
    const section=result[key];
    lines.push('\n'+t(labels[key])+' · '+t(states[section.state]));
    if(section.error)lines.push(terminalText(section.error.code+' · '+section.error.message));
  }
  if(result.checks.result)lines.push(formatOptimizeText(result.checks.result));
  if(result.configuration.result)lines.push(formatConfigText(result.configuration.result,{action:'list',review:true}));
  if(result.history.result){
    const r=result.history.result.result;
    lines.push(t('cli.project.total',{tokens:r.summary.tokens.total??'—'}));
  }
  if(result.records.result)for(const i of result.records.result.items)lines.push(`${terminalText(i.title??i.evidence.threadId??'—')} · ${i.record?.lastActivityAt??'—'} · ${i.evidence.threadId}`);
  if(result.artifacts.result)lines.push(formatArtifactsText(result.artifacts.result));
  lines.push(t('cli.project.next'));
  return lines.join('\n')+'\n';
}
export async function runProjectCli(argv:readonly string[]):Promise<number>{
  return withCancellation(async signal=>{
    if(argv[0]==='verify')return runVerificationCli([...argv]);
    const p=parseOptions(argv,['--project','--root','--since','--until','--timezone','--source','--limit','--offset','--sort','--task','--turn','--agents-bytes','--description-characters','--item','--baseline'],['--explain','--unassigned'],['--root']);
    if(p.help){writeHelp('check',argv);return 0;}
    if(p.positionals.length>1)invalidInput('check');
    const target=p.positionals[0];
    if(target&&!['skills','mcp','rules','hooks'].includes(target))invalidInput(target);
    const explain=p.flags.has('--explain')||p.flags.has('--unassigned')||p.values.has('--item')||p.values.has('--baseline');
    if(explain&&(!target||p.get('--task')))invalidInput('--explain');
    if(p.flags.has('--unassigned')&&p.get('--item'))invalidInput('--unassigned');
    let baseline:ConfigRequest['baseline'];
    let baselineRecords:ConfigRequest['baselineRecords'];
    if(p.get('--baseline')) {
      const report=readBaseline(p.get('--baseline')!);
      if(!report||typeof report!=='object'||!('explanation' in report))invalidInput('--baseline');
      const explanation=report.explanation;
      if(!explanation||typeof explanation!=='object'||!('baseline' in explanation))invalidInput('--baseline');
      const candidate:unknown={action:'explain',baseline:explanation.baseline,baselineRecords:'records' in explanation?explanation.records:undefined};
      if(!validateConfigRequest(candidate)||!candidate.baseline)invalidInput('--baseline');
      baseline=candidate.baseline;
      baselineRecords=candidate.baselineRecords;
    }
    const window=baseline?.scope.allTime&&!p.get('--since')&&!p.get('--until')?{allTime:true,since:undefined,until:undefined,timezone:p.get('--timezone')??baseline.scope.timezone??'UTC'}:dateWindow({since:p.get('--since')??baseline?.scope.since??undefined,until:p.get('--until')??baseline?.scope.until??undefined,timezone:p.get('--timezone')??baseline?.scope.timezone??undefined},'recent');
    const roots=p.all('--root').length?p.all('--root').map(r=>path.resolve(r)):baseline?.roots??[],client=createNodeClient({automaticPrices:false});
    if(p.get('--task')) {
      if(target||['--agents-bytes','--description-characters','--sort','--offset','--limit','--since','--until','--timezone'].some(k=>p.values.has(k)))invalidInput('--task');
      const scope={allTime:true,...(p.get('--project')?{project:projectFilter(p.get('--project')!)}:{}),sourceInstanceId:p.get('--source')};
      const selected=await resolveRecord(client,p.get('--task')!,scope,roots,argv,signal);
      const turns=(await client.live!({query:{action:'turns',threadId:selected.thread.id,scope,limit:20,compact:true},mode:'cached'},{signal})).result;
      const available=turns.items.filter(i=>i.kind==='turn');
      let turn=p.get('--turn')??(available.length===1&&turns.page.total===1?available[0].id:undefined);
      if(!turn){
        const choices=available.map((i,n)=>`${n+1}. ${i.ordinal??'—'} · ${i.startedAt??'—'} · ${i.status} · ${i.id}`).join('\n');
        if(!canPrompt(argv)||!available.length)invalidInput(t('cli.project.turnRequired')+'\n'+choices);
        process.stderr.write(choices+'\n');
        const choice=await ask(t('cli.project.turnChoose'),signal);
        if(!choice)throw new CoreError('CANCELLED',t('common.cancelled'));
        turn=available[pageNumber(choice,0,1,available.length)-1].id;
      }
      const result=await client.optimize!({action:'activity',roots,activity:{threadId:selected.thread.id,turnId:turn}},{signal});
      process.stdout.write(p.json?JSON.stringify(result)+'\n':formatOptimizeText(result));return result.resultStatus==='partial'?2:0;
    }
    if(p.values.has('--turn'))invalidInput('--turn');
    const project=await projectDirectory(p.get('--project')??baseline?.scope.project??undefined,argv,signal);
    if(target){
      if(['--agents-bytes','--description-characters'].some(k=>p.values.has(k)))invalidInput(target);
      const kind:ConfigRequest['kind']=target==='skills'?'skill':target==='rules'?'rule':target==='hooks'?'hook':'mcp';
      if(kind!=='skill'&&p.get('--sort')==='context_tokens')invalidInput('--sort');
      const sort=p.get('--sort')??'review';
      if(!['review','context_tokens','least_activity','recent','name'].includes(sort))invalidInput('--sort');
      const request:ConfigRequest={action:explain?'explain':'list',kind,review:kind==='skill'||kind==='mcp',roots,projectRoots:[project],scope:{...baseline?.scope,...window,allTime:window.allTime??false,project,sourceInstanceId:p.get('--source')??baseline?.scope.sourceInstanceId},itemId:p.flags.has('--unassigned')?undefined:p.get('--item')??baseline?.itemId,unassigned:p.get('--item')?false:p.flags.has('--unassigned')||baseline?.unassigned,baseline,baselineRecords,limit:pageNumber(p.get('--limit'),20,1,200),offset:pageNumber(p.get('--offset'),0),sort:sort as ConfigRequest['sort']};
      const r=await client.config!(request,{signal});
      process.stdout.write(p.json?JSON.stringify(r)+'\n':formatConfigText(r,request,{explain:p.flags.has('--explain')}));
      if(!p.json&&r.explanation?.page.nextOffset!=null) {
        const next=argv.filter((arg,i)=>arg!=='--offset'&&!arg.startsWith('--offset=')&&argv[i-1]!=='--offset');
        process.stdout.write(t('config.explainNext',{command:terminalText(commandLine(['check',...next,'--offset',String(r.explanation.page.nextOffset)]))})+'\n');
      }
      return configExitCode(r);
    }
    if(['--limit','--offset','--sort'].some(k=>p.values.has(k)))invalidInput('check');
    const ruleOverrides=p.values.has('--agents-bytes')||p.values.has('--description-characters')?{agentsBytes:pageNumber(p.get('--agents-bytes'),16384,1),descriptionCharacters:pageNumber(p.get('--description-characters'),500,0,1024)}:undefined;
    const r=await client.project!({projectRoot:project,roots,since:window.since!,until:window.until!,timezone:window.timezone,sourceInstanceId:p.get('--source'),ruleOverrides},{signal});
    process.stdout.write(p.json?JSON.stringify(r)+'\n':formatProjectText(r));return r.partial?2:0;
  });
}
