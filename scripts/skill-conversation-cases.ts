import assert from 'node:assert/strict';
import type {ConversationLanguage} from './verify-skill-conversation-helpers.ts';

/** Paired natural requests exercise intent and follow-up boundaries, not Prompt wording. */
export const focusedQuestions={
  usage:[
    ['全部日期一共用了多少 Token？只要总量和统计范围。','How many Tokens have I used across all dates? Just give the total and scope.'],
    ['帮我查累计用量，先不用看配置。','Check my cumulative usage; leave configuration for later.'],
    ['这个项目总共消耗了多少？','What is this project’s total usage?'],
    ['先给我一个用量总数。','Start with one total usage figure.'],
  ],
  pricing:[
    ['全部日期大概花了多少钱？哪些模型还不能计价？离线查询。','What is the estimated cost across all dates, and which models remain unpriced? Stay offline.'],
    ['用现有记录估算费用，缺少价格的部分单独说明。','Estimate cost from existing records and explain any portion missing prices.'],
    ['我只想看已知费用和未计价用量，不更新价表。','Show only known cost and unpriced usage; do not update prices.'],
    ['这些 Token 折算多少钱？不要当成订阅账单。','What is the API-equivalent cost of these Tokens? Keep it distinct from my subscription bill.'],
  ],
  timing:[
    ['已选回合耗时多久？说明计时依据，不检查配置。','How long did the selected turn take? Explain the timing basis without inspecting configuration.'],
    ['这个回合从开始到结束用了多长时间？','How much time passed from the start to the end of this turn?'],
    ['查一下这一轮的用时，缺失的项目别填零。','Check this turn’s duration; keep missing metrics unknown.'],
    ['先只看这个回合的耗时摘要。','Show only the timing summary for this turn.'],
  ],
  statistics:[
    ['全部日期的任务规模中位数和 P90 是多少？样本有多少？','What are median and P90 task usage across all dates, and how many tasks are in the sample?'],
    ['按当前项目看看任务用量分布，不分析配置。','Show task usage distribution for this project without inspecting configuration.'],
    ['这项任务在本项目的任务规模中处于什么位置？','Where does this task sit in the project’s task usage distribution?'],
    ['任务量的平均值和中位数差多少？先给统计结果。','How do mean and median task usage compare? Start with the statistics.'],
  ],
  monitor:[
    ['现在配置了哪些 Token 预算？先只查看。','Which Token budgets are configured? Read only for now.'],
    ['帮我看看已有预算计划，不创建新的。','Show existing budget plans without creating one.'],
    ['有没有已保存的用量预算？','Are there any saved usage budgets?'],
    ['列一下现有 Token 预算，额度另外再说。','List existing Token budgets; account allowance can wait.'],
  ],
  account:[
    ['账户额度还剩多少？实际窗口有多长？','How much account allowance remains, and how long is its actual window?'],
    ['只查订阅额度，不查项目用量。','Check account allowance only, without querying project usage.'],
    ['当前窗口用了多少比例？多久重置？','What percentage of the current window is used, and when does it reset?'],
    ['帮我看剩余额度，缺失部分说明原因。','Show remaining allowance and explain unavailable sections.'],
  ],
} as const;
export type FocusedScenario=keyof typeof focusedQuestions;
export const conversationScenarios=['workflow',...Object.keys(focusedQuestions)] as const;
export function isFocusedScenario(value:string):value is FocusedScenario{return Object.hasOwn(focusedQuestions,value);}
export function focusedQuestion(scenario:FocusedScenario,language:ConversationLanguage,variant:number):string {
  assert.ok(Number.isInteger(variant)&&variant>=0&&variant<4,'Expected variant 0..3');
  return focusedQuestions[scenario][variant][language==='zh'?0:1];
}
type ObjectValue=Record<string,unknown>;
const object=(v:unknown):v is ObjectValue=>v!==null&&typeof v==='object'&&!Array.isArray(v);
/** Inspect structured runtime results; prose quality is reviewed separately, never a keyword gate. */
export function focusedResults(events:string):ObjectValue[] {
  const results:ObjectValue[]=[];
  for(const line of events.split('\n').filter(Boolean)){
    const event:unknown=JSON.parse(line);
    if(!object(event)||event.type!=='item.completed'||!object(event.item)||event.item.type!=='command_execution'||typeof event.item.aggregated_output!=='string')continue;
    for(const row of event.item.aggregated_output.split('\n')){
      if(!row.trim().startsWith('{'))continue;
      let parsed:unknown;try{parsed=JSON.parse(row);}catch{continue;}
      if(object(parsed))results.push(object(parsed.result)?parsed.result:parsed);
    }
  }
  return results;
}
function resultKind(value:ObjectValue):string|null {
  if(object(value.error)||Array.isArray(value.methods))return null;
  if(object(value.snapshotRef))return 'usage';
  if(Array.isArray(value.windows))return 'account';
  if(Array.isArray(value.plans))return 'monitor';
  if(value.outputVersion===6&&object(value.time))return 'timing';
  if(Array.isArray(value.suggestions))return 'optimize';
  if(typeof value.readView==='string'&&Array.isArray(value.items))return 'config';
  if(object(value.catalog))return 'prices';
  if(object(value.discovery))return 'setup';
  return null;
}
export function validateFocusedResults(scenario:FocusedScenario,results:ObjectValue[],scope:{snapshotId:string;project:string;totalTokens?:number;since?:string;until?:string}):void {
  const required=scenario==='statistics'?'usage':scenario==='pricing'?'usage':scenario;
  const allowed=new Set([required,...(scenario==='pricing'?['prices']:scenario==='timing'?['usage']:[])]);
  const kinds=results.map(resultKind).filter(k=>k!==null);
  assert.ok(kinds.includes(required),'No result for the requested capability');
  assert.ok(kinds.every(k=>allowed.has(k)),'Unrelated capability queried: '+kinds.join(', '));
  for(const result of results){
    assert.ok(!['upsert','remove','acknowledge','update','auto_update','pause','resume','send','keep','not_applicable','recheck','authorize','revoke'].includes(String(result.action)),'Read-only question performed a mutation');
    if(resultKind(result)==='usage'){
      assert.ok(object(result.snapshotRef)&&result.snapshotRef.snapshotId===scope.snapshotId&&object(result.scope)&&result.scope.project===scope.project,'Usage changed selected scope or version');
      if(scope.since!==undefined||scope.until!==undefined)assert.ok(result.scope.since===scope.since&&result.scope.until===scope.until,'Usage changed the requested date interval');
    }
  }
  if(scenario==='usage'||scenario==='pricing'||scenario==='statistics'){
    const expected=scope.totalTokens??(scenario==='pricing'?1101000:1100000);
    assert.ok(results.some(r=>object(r.summary)&&object(r.summary.tokens)&&r.summary.tokens.total===expected),'Missing independent usage truth');
  }
  if(scenario==='statistics')assert.ok(results.some(r=>object(r.statistics)&&object(r.statistics.population)&&r.statistics.population.completeTasks===1&&r.statistics.population.medianTokens===1100000&&r.statistics.population.p90Tokens===1100000),'No independent complete task statistics');
  if(scenario==='pricing')assert.ok(results.some(r=>object(r.summary)&&object(r.summary.price)&&r.summary.price.cost===null&&r.summary.price.knownCost==='7.25'),'Missing partial cost truth');
  if(scenario==='timing')assert.ok(results.some(r=>object(r.time)&&object(r.time.nativeWallClockMs)&&r.time.nativeWallClockMs.value===15000),'Missing independent 15-second timing truth');
  if(scenario==='account')assert.ok(results.some(r=>Array.isArray(r.windows)&&r.windows.some(w=>object(w)&&w.usedPercent===20&&w.durationMinutes===17)),'Missing synthetic allowance truth');
  if(scenario==='monitor')assert.ok(results.some(r=>Array.isArray(r.plans)&&r.plans.length===0),'Empty budget list was not read');
}
