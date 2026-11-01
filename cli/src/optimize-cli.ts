import {writeHelp} from './help.js';
import {commandLine,invalidInput,pageNumber,parseOptions,withCancellation} from './human-input.js';
import {optimizeExitCode} from './exit-codes.js';
import path from 'node:path';
import {type OptimizeRequest,type OptimizeResult} from '@wombat/client';
import {createNodeClient} from '@wombat/client/node';
import {t,reviewFindingLabel,reviewFindingNote,reviewFindingCount,reviewPresentation,assessmentReason,activityRuleTitle,activityFindingTitle,activityCheckText,activityAdviceText} from '@wombat/client/locale';
import {terminalText} from './display-text.js';
export function parseVerificationArgs(argv: string[]) {
  if(argv[0]!=='verify')invalidInput(argv[0]??'verify');
  const p=parseOptions(argv.slice(1),['--root','--project','--source','--suggestion','--item','--read-view','--baseline','--agents-bytes','--description-characters'],[],['--root']);
  if(p.positionals.length)invalidInput(p.positionals[0]);
  const request:OptimizeRequest={action:'recheck',readView:p.get('--read-view'),baselineId:p.get('--baseline')};
  if(!p.help&&(!request.readView||!request.baselineId))invalidInput('--read-view/--baseline');
  if(p.all('--root').length)request.roots=p.all('--root').map(r=>path.resolve(r));
  const project=path.resolve(p.get('--project')??process.cwd());
  request.projectRoots=[project];request.project=project;
  request.sourceInstanceId=p.get('--source');request.suggestionId=p.get('--suggestion');request.itemId=p.get('--item');
  if(p.get('--agents-bytes')||p.get('--description-characters'))request.ruleOverrides={...(p.get('--agents-bytes')?{agentsBytes:pageNumber(p.get('--agents-bytes'),16384,1)}:{}),...(p.get('--description-characters')?{descriptionCharacters:pageNumber(p.get('--description-characters'),500,0,1024)}:{})};
  return {request,json:p.json,help:p.help};
}
export async function runVerificationCli(argv:string[]):Promise<number>{
  const {request,json,help}=parseVerificationArgs(argv);
  if(help){writeHelp('check',argv);return 0;}
  return withCancellation(async signal=>{
    const result=await createNodeClient().optimize!(request,{signal});
    process.stdout.write(json?JSON.stringify(result)+'\n':formatOptimizeText(result));
    return optimizeExitCode(result);
  });
}
export function formatOptimizeText(result:OptimizeResult):string {
  if(result.action==='activity'&&result.activity){
    const activity=result.activity;
    return [t('activity.title'),...activity.advice.flatMap(rule=>{const check=activity.checks.find(check=>check.rule===rule)!;return [activityFindingTitle(check),activityAdviceText(rule),...(check.partial?[t('activity.partial')]:[])];}),
      t('activity.checks'),...activity.checks.map(check=>`${activityRuleTitle(check.rule)}: ${activityCheckText(check)}`),
      t('activity.note'),`${activity.readView.snapshotId} · ${activity.scope.threadId} · ${activity.scope.turnId}`,
      activity.analysisMethod].map(terminalText).join('\n')+'\n';
  }

 const lines=[t('optimize.summaryCount',{pending:result.pending})];
 for(const s of result.suggestions){
  const p=reviewPresentation(s);lines.push(s.id+'\t'+p.title+'\t'+s.item.path);
  for(const f of s.findings)lines.push('  '+(reviewFindingCount(f)??reviewFindingLabel(f.rule)), '  '+reviewFindingNote(f.rule,s.item.project??undefined));
  if(s.baselineId)lines.push('  '+t('optimize.baseline',{id:s.baselineId}));
  const scope=s.checks[0]?.basis.scope, project=s.scopeProject??scope?.project;
  if(s.baselineId&&result.readView&&project&&scope?.projectRoots.length===1&&scope.projectRoots[0]===project){
    const args=['check','verify','--read-view',result.readView,'--baseline',s.baselineId,'--project',project,...scope.roots.flatMap(root=>['--root',root])];
    if(scope.sourceInstanceId)args.push('--source',scope.sourceInstanceId);
    const overrides=result.ruleParameters.overrides;
    if(overrides.agentsBytes!=null)args.push('--agents-bytes',String(overrides.agentsBytes));
    if(overrides.descriptionCharacters!=null)args.push('--description-characters',String(overrides.descriptionCharacters));
    lines.push('  '+commandLine(args));
  }
 }
 if(result.checks.length)lines.push(t('optimize.checks'));
 for(const c of result.checks){lines.push(reviewFindingLabel(c.rule)+'\t'+t(`optimize.check.${c.outcome}`),'  '+t(`optimize.assessment.comparison.${c.comparison.status}`));if(c.reason)lines.push('  '+assessmentReason(c.reason));if(c.comparison.reason)lines.push('  '+assessmentReason(c.comparison.reason));}
 if(result.readView)lines.push(t('config.readVersion',{version:result.readView}));
 if(result.resultStatus==='partial')lines.push(t('optimize.coverageNote'));
 return lines.map(terminalText).join('\n')+'\n';
}
