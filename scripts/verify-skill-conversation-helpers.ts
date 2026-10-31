import assert from 'node:assert/strict';
import path from 'node:path';
import {conversationScenarios,isFocusedScenario} from './skill-conversation-cases.ts';
export type ConversationLanguage='en'|'zh';
export function parseConversationArgs(args:string[]) {
  if(args[0]==='--')args=args.slice(1);
  const values=new Map<string,string>();
  for(let i=0;i<args.length;i++){
    const key=args[i],value=args[++i];
    if(!['--output-dir','--agent-bin','--language','--scenario','--variant'].includes(key)||values.has(key)||!value||value.startsWith('--'))throw new Error('Usage: verify:skill-conversation --output-dir ABSOLUTE_EXTERNAL_PATH --agent-bin ABSOLUTE_EXECUTABLE [--language en|zh|both] [--scenario usage|pricing|timing|statistics|monitor|account|daily-review|task-retro|config-review|all] [--variant 0..3]');
    values.set(key,value);
  }
  const output=values.get('--output-dir'),agentBin=values.get('--agent-bin'),language=values.get('--language')??'both';
  if(!output||!path.isAbsolute(output)||!agentBin||!path.isAbsolute(agentBin))throw new Error('Explicit absolute --output-dir and --agent-bin are required');
  if(!['en','zh','both'].includes(language))throw new Error('--language must be en, zh, or both');
  const scenario=values.get('--scenario')??'usage',variant=values.get('--variant')??'0';
  if(scenario!=='all'&&!isFocusedScenario(scenario))throw new Error('Unknown conversation scenario');
  if(!/^[0-3]$/.test(variant))throw new Error('--variant must be 0..3');
  return {output,agentBin,languages:language==='both'?['en','zh'] as ConversationLanguage[]:[language as ConversationLanguage],scenarios:scenario==='all'?conversationScenarios:[scenario],variant:Number(variant)};
}
function record(value:unknown):value is Record<string,unknown>{return value!==null&&typeof value==='object'&&!Array.isArray(value);}
export function nativeSessionId(events:string):string {
  const started:unknown[]=events.split('\n').filter(Boolean).map(line=>JSON.parse(line)).filter(value=>record(value)&&value.type==='thread.started');
  assert.equal(started.length,1,'Expected exactly one newly created native session');
  const event=started[0];assert.ok(record(event));
  assert.ok(typeof event.thread_id==='string'&&/^[0-9a-f]{8}(?:-[0-9a-f]{4}){3}-[0-9a-f]{12}$/.test(event.thread_id),'Missing exact native session identity');
  return event.thread_id;
}
