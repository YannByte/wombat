import assert from 'node:assert/strict';
import path from 'node:path';
import {conversationScenarios,isFocusedScenario} from './skill-conversation-cases.ts';
export type ConversationLanguage='en'|'zh';
export function parseConversationArgs(args:string[]) {
  if(args[0]==='--')args=args.slice(1);
  const values=new Map<string,string>();
  for(let i=0;i<args.length;i++){
    const key=args[i],value=args[++i];
    if(!['--output-dir','--agent-bin','--language','--scenario','--variant'].includes(key)||values.has(key)||!value||value.startsWith('--'))throw new Error('Usage: verify:skill-conversation --output-dir ABSOLUTE_EXTERNAL_PATH --agent-bin ABSOLUTE_EXECUTABLE [--language en|zh|both] [--scenario workflow|usage|pricing|timing|statistics|monitor|account|daily-review|task-retro|config-review|all] [--variant 0..3]');
    values.set(key,value);
  }
  const output=values.get('--output-dir'),agentBin=values.get('--agent-bin'),language=values.get('--language')??'both';
  if(!output||!path.isAbsolute(output)||!agentBin||!path.isAbsolute(agentBin))throw new Error('Explicit absolute --output-dir and --agent-bin are required');
  if(!['en','zh','both'].includes(language))throw new Error('--language must be en, zh, or both');
  const scenario=values.get('--scenario')??'workflow',variant=values.get('--variant')??'0';
  if(scenario!=='all'&&scenario!=='workflow'&&!isFocusedScenario(scenario))throw new Error('Unknown conversation scenario');
  if(!/^[0-3]$/.test(variant))throw new Error('--variant must be 0..3');
  return {output,agentBin,languages:language==='both'?['en','zh'] as ConversationLanguage[]:[language as ConversationLanguage],scenarios:scenario==='all'?conversationScenarios:[scenario],variant:Number(variant)};
}
function record(value:unknown):value is Record<string,unknown>{return value!==null&&typeof value==='object'&&!Array.isArray(value);}
export interface ConversationExpected {threadId:string;suggestionId:string;contextFile:string}
export function validateConversationAnswer(value:unknown,phase:'read'|'continue',expected:ConversationExpected,language:ConversationLanguage):void {
  assert.ok(record(value),'Missing structured conversation answer');
  assert.equal(value.completeTokens,1_100_000,'Answer disagrees with independent Token truth');
  assert.equal(value.topTaskId,expected.threadId,'Conversation changed task identity');
  assert.equal(value.suggestionId,expected.suggestionId,'Conversation changed original suggestion identity');
  assert.ok(typeof value.explanation==='string'&&value.explanation.trim()&&(language==='zh'?/\p{Script=Han}/u.test(value.explanation):/\b(?:tokens?|usage|total|description|recheck)\b/i.test(value.explanation)),'Explanation language was not verified');
  if(phase==='read'){
    assert.equal(value.remainingPercent,80,'Synthetic allowance truth differs');
    assert.equal(value.windowMinutes,17,'Synthetic window truth differs');
    assert.equal(value.contextFile,expected.contextFile,'Wrong Web context artifact');
  }else{
    assert.equal(value.recheckStatus,'verified','Original suggestion was not independently rechecked');
    assert.equal(value.decision,null,'Recheck must preserve the separate user decision');
  }
}
export function nativeSessionId(events:string):string {
  const started:unknown[]=events.split('\n').filter(Boolean).map(line=>JSON.parse(line)).filter(value=>record(value)&&value.type==='thread.started');
  assert.equal(started.length,1,'Expected exactly one newly created native session');
  const event=started[0];assert.ok(record(event));
  assert.ok(typeof event.thread_id==='string'&&/^[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}$/.test(event.thread_id),'Missing exact native session identity');
  return event.thread_id;
}
