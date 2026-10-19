import assert from 'node:assert/strict';
import path from 'node:path';
import type {ConversationLanguage} from './verify-skill-conversation-helpers.ts';

/** Paired natural requests exercise intent and follow-up boundaries, not Prompt wording. */
export const focusedQuestions={
  'daily-review':[
    ['复盘所选日期的用量，找出一个有证据的改善点。先只读。','Review usage for the selected day and identify one evidence-backed improvement. Read only.'],
    ['这一天的消耗主要在哪些任务？有什么具体可改善的？','Which tasks account for this day’s usage, and what could I improve?'],
    ['给我一份当天用量小结，不需要查账户额度或配置。','Give me a daily usage review without checking account allowance or configuration.'],
    ['帮我看看这一天最值得检查的任务，不要为了凑建议而找问题。','Show which task is most worth checking that day. Do not manufacture recommendations.'],
  ],
  'task-retro':[
    ['复盘已选任务，结合原文给一个具体改善建议；证据不足就说明。','Review the selected task using original evidence and suggest one improvement, if supported.'],
    ['这项任务有没有反复折腾？结合过程检查，先不要改配置。','Check the selected task for repeated work using its history. Do not edit configuration.'],
    ['帮我看看已选任务的用量和过程，指出下一步该查哪里。','Review the selected task’s usage and history, and identify what to check next.'],
    ['我想复盘当前选中的任务，给出依据，不要换成排行榜第一名。','Review the currently selected task with evidence. Keep its identity rather than choosing the top-ranked task.'],
  ],
  'config-review':[
    ['检查这个项目的 Skills 配置，指出具体文件和问题。先只读。','Review this project’s Skill configuration with exact files and findings. Read only.'],
    ['这个项目的 Skill 描述是不是太长了？给出精简建议，暂时不修改。','Check this project’s Skill descriptions for excessive length and propose changes without editing.'],
    ['帮我审阅 Skills 的触发描述和范围，不要分析整段历史用量。','Review Skill descriptions and scope without analyzing all historical usage.'],
    ['看看这个项目有哪些可以改善的 Skill 配置，别把没有使用记录当成闲置。','Find actionable Skill configuration issues without treating missing use records as inactivity.'],
  ],
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
export const taskSkillNames={ 'daily-review':'wombat-daily-review','task-retro':'wombat-task-retro','config-review':'wombat-config-review' } as const;
export function taskSkill(scenario:string):string{return Object.hasOwn(taskSkillNames,scenario)?taskSkillNames[scenario as keyof typeof taskSkillNames]:'wombat';}
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
  if(value.outputVersion===1&&typeof value.snapshotId==='string'&&Array.isArray(value.fragments)&&Array.isArray(value.items))return 'conversation';
  if(object(value.catalog))return 'prices';
  if(object(value.discovery))return 'setup';
  return null;
}
export function validateFocusedResults(scenario:FocusedScenario,results:ObjectValue[],scope:{snapshotId:string;project:string;threadId?:string;totalTokens?:number;since?:string;until?:string}):void {
  const required=['statistics','pricing','daily-review','task-retro'].includes(scenario)?'usage':scenario==='config-review'?'optimize':scenario;
  const allowed=new Set([required,...(scenario==='pricing'?['prices']:scenario==='timing'?['usage']:scenario==='daily-review'?['conversation']:scenario==='task-retro'?['conversation','timing']:scenario==='config-review'?['config']:[])]);
  const kinds=results.map(resultKind).filter(k=>k!==null);
  assert.ok(kinds.includes(required),'No result for the requested capability');
  assert.ok(kinds.every(k=>allowed.has(k)),'Unrelated capability queried: '+kinds.join(', '));
  for(const result of results){
    assert.ok(!['upsert','remove','acknowledge','update','auto_update','pause','resume','send','keep','not_applicable','recheck','authorize','revoke'].includes(String(result.action)),'Read-only question performed a mutation');
    if(resultKind(result)==='usage'){
      assert.ok(object(result.snapshotRef)&&result.snapshotRef.snapshotId===scope.snapshotId&&object(result.scope)&&result.scope.project===scope.project,'Usage changed selected scope or version');
      if(scope.since!==undefined||scope.until!==undefined)assert.ok(result.scope.since===scope.since&&result.scope.until===scope.until,'Usage changed the requested date interval');
    }
    if(resultKind(result)==='conversation')assert.ok(result.snapshotId===scope.snapshotId&&object(result.selection)&&Array.isArray(result.selection.projects)&&result.selection.projects.length===1&&result.selection.projects[0]===scope.project,'Conversation changed scope or version');
  }
  if(['usage','pricing','statistics','daily-review','task-retro'].includes(scenario)){
    const expected=scope.totalTokens??(scenario==='pricing'?1101000:1100000);
    assert.ok(results.some(r=>object(r.summary)&&object(r.summary.tokens)&&r.summary.tokens.total===expected),'Missing independent usage truth');
  }
  if(scenario==='daily-review')assert.ok(results.some(r=>Array.isArray(r.items)&&r.items.some(i=>object(i)&&i.kind==='thread'&&i.id===scope.threadId)),'Daily review omitted task evidence');
  if(scenario==='task-retro'){
    assert.ok(results.some(r=>object(r.inspection)&&object(r.scope)&&r.scope.threadId===scope.threadId),'Retrospective omitted the selected task investigation');
    assert.ok(results.some(r=>resultKind(r)==='conversation'&&Array.isArray(r.fragments)&&r.fragments.some(f=>object(f)&&typeof f.text==='string'&&f.text.includes('SYNTHETIC_WORK_EVIDENCE'))),'Retrospective omitted original source evidence');
    for(const r of results)if(resultKind(r)==='conversation')assert.ok(object(r.selection)&&Array.isArray(r.selection.threads)&&r.selection.threads.length===1&&r.selection.threads[0]===scope.threadId,'Retrospective changed task identity');
  }
  if(scenario==='config-review')assert.ok(results.some(r=>Array.isArray(r.suggestions)&&r.suggestions.some(s=>object(s)&&object(s.item)&&s.item.path===path.join(scope.project,'.agents/skills/sample/SKILL.md')&&Array.isArray(s.findings)&&s.findings.some(f=>object(f)&&f.rule==='descriptionSize'))),'Configuration review omitted the independent description-size finding');
  if(scenario==='statistics')assert.ok(results.some(r=>object(r.statistics)&&object(r.statistics.population)&&r.statistics.population.completeTasks===1&&r.statistics.population.medianTokens===1100000&&r.statistics.population.p90Tokens===1100000),'No independent complete task statistics');
  if(scenario==='pricing')assert.ok(results.some(r=>object(r.summary)&&object(r.summary.price)&&r.summary.price.cost===null&&r.summary.price.knownCost==='7.25'),'Missing partial cost truth');
  if(scenario==='timing')assert.ok(results.some(r=>object(r.time)&&object(r.time.nativeWallClockMs)&&r.time.nativeWallClockMs.value===15000),'Missing independent 15-second timing truth');
  if(scenario==='account')assert.ok(results.some(r=>Array.isArray(r.windows)&&r.windows.some(w=>object(w)&&w.usedPercent===20&&w.durationMinutes===17)),'Missing synthetic allowance truth');
  if(scenario==='monitor')assert.ok(results.some(r=>Array.isArray(r.plans)&&r.plans.length===0),'Empty budget list was not read');
}
