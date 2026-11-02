import {writeHelp} from './help.js';
import {commandLine,invalidInput,pageNumber,parseOptions,withCancellation} from './human-input.js';
import {optimizeExitCode} from './exit-codes.js';
import path from 'node:path';
import {readBaseline} from './read-baseline.js';
import {type OptimizeRequest,type OptimizeResult} from '@wombat/client';
import {createNodeClient} from '@wombat/client/node';
import {t,reviewFindingLabel,reviewFindingNote,reviewFindingCount,reviewPresentation,assessmentReason,activityRuleTitle,activityFindingTitle,activityCheckText,activityAdviceText} from '@wombat/client/locale';
import {terminalText} from './display-text.js';
export function parseVerificationArgs(argv: string[]) {
  if(argv[0]!=='verify')invalidInput(argv[0]??'verify');
  const p=parseOptions(argv.slice(1),['--root','--project','--source','--suggestion','--item','--baseline','--agents-bytes','--description-characters'],[],['--root']);
  if(p.positionals.length)invalidInput(p.positionals[0]);
  const request:OptimizeRequest={action:'recheck'};
  if(!p.help){
    if(!p.get('--baseline'))invalidInput('--baseline');
    const value=readBaseline(p.get('--baseline')!);
    if(value===null||typeof value!=='object'||Array.isArray(value))invalidInput('--baseline');
    const report=value as OptimizeResult | {checks:{result:OptimizeResult}};
    const result='checks' in report && !Array.isArray(report.checks) ? (report as {checks:{result:OptimizeResult}}).checks.result : report as OptimizeResult;
    const rows=result?.suggestions;
    if(!Array.isArray(rows))invalidInput('--baseline');
    const old=p.get('--suggestion')?rows.find(s=>s.id===p.get('--suggestion')):rows.length===1?rows[0]:undefined;
    if(!old)invalidInput('--suggestion');
    request.baseline=old;request.baselineId=old.baselineId;request.suggestionId=old.id;
    request.ruleOverrides=old.ruleParameters?.overrides;
    const scope=old.checks[0]?.basis.scope;
    if(scope){request.roots=scope.roots;request.projectRoots=scope.projectRoots;request.project=scope.project;request.sourceInstanceId=scope.sourceInstanceId;}
  }
  if(p.all('--root').length)request.roots=p.all('--root').map(r=>path.resolve(r));
  if(p.get('--project')){const project=path.resolve(p.get('--project')!);request.projectRoots=[project];request.project=project;}
  if(p.get('--source'))request.sourceInstanceId=p.get('--source');
  if(p.get('--item'))request.itemId=p.get('--item');
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
      t('activity.note'),`${activity.basis.snapshotId} · ${activity.scope.threadId} · ${activity.scope.turnId}`,
      activity.analysisMethod].map(terminalText).join('\n')+'\n';
  }

 const lines=[t('optimize.summaryCount',{pending:result.pending})];
 for(const s of result.suggestions){
  const p=reviewPresentation(s);lines.push(s.id+'\t'+p.title+'\t'+s.item.path);
  for(const f of s.findings)lines.push('  '+(reviewFindingCount(f)??reviewFindingLabel(f.rule)), '  '+reviewFindingNote(f.rule,s.item.project??undefined));

 }
 if(result.checks.length)lines.push(t('optimize.checks'));
 for(const c of result.checks){lines.push(reviewFindingLabel(c.rule)+'\t'+t(`optimize.check.${c.outcome}`),'  '+t(`optimize.assessment.comparison.${c.comparison.status}`));if(c.reason)lines.push('  '+assessmentReason(c.reason));if(c.comparison.reason)lines.push('  '+assessmentReason(c.comparison.reason));}
 if(result.resultStatus==='partial')lines.push(t('optimize.coverageNote'));
 return lines.map(terminalText).join('\n')+'\n';
}
