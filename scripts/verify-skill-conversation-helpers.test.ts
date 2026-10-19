import {test} from 'node:test';
import assert from 'node:assert/strict';
import path from 'node:path';
import {tmpdir} from 'node:os';
import {parseConversationArgs,validateConversationAnswer,nativeSessionId} from './verify-skill-conversation-helpers.ts';
const expected={threadId:'fixed-task',suggestionId:'original-suggestion',contextFile:path.join(tmpdir(),'web-context.json')};
const read={completeTokens:1100000,topTaskId:expected.threadId,suggestionId:expected.suggestionId,contextFile:expected.contextFile,remainingPercent:80,windowMinutes:17,explanation:'Tokens total and configuration evidence.'};
const follow={completeTokens:1100000,topTaskId:expected.threadId,suggestionId:expected.suggestionId,recheckStatus:'verified',decision:null,explanation:'Description edited and recheck verified.'};
test('conversation arguments require explicit executables and reject ambiguous or unsupported modes',()=>{
  const args=['--output-dir',path.join(tmpdir(),'conversation-output'),'--agent-bin',process.execPath];
  assert.deepEqual(parseConversationArgs(['--',...args]).languages,['en','zh']);
  assert.deepEqual(parseConversationArgs([...args,'--language','zh']).languages,['zh']);
  assert.equal(parseConversationArgs(args).scenarios[0],'workflow');
  assert.equal(parseConversationArgs([...args,'--scenario','all']).scenarios.length,10);
  assert.equal(parseConversationArgs([...args,'--scenario','pricing','--variant','3']).variant,3);
  assert.deepEqual(parseConversationArgs([...args,'--scenario','timing']).scenarios,['timing']);
  for(const extra of [['--language','fr'],['--last'],['--agent-bin',process.execPath],['--language'],['--unknown','value']])assert.throws(()=>parseConversationArgs([...args,...extra]));
  assert.throws(()=>parseConversationArgs(['--output-dir','relative','--agent-bin',process.execPath]));
  assert.throws(()=>parseConversationArgs(['--output-dir',path.join(tmpdir(),'conversation-output'),'--agent-bin','codex']));
  for(const extra of [['--scenario','unknown'],['--variant','-1'],['--variant','4'],['--variant','1.0'],['--variant','no']])assert.throws(()=>parseConversationArgs([...args,...extra]));
});
test('native structured answers must preserve synthetic truth, task, suggestion and user decision',()=>{
  validateConversationAnswer(read,'read',expected,'en');validateConversationAnswer(follow,'continue',expected,'en');
  validateConversationAnswer({...read,explanation:'总量、额度和配置问题依据已核对。'},'read',expected,'zh');
  for(const change of [{completeTokens:0},{topTaskId:'other'},{suggestionId:'new'},{contextFile:'other'},{remainingPercent:0},{windowMinutes:60},{explanation:''}])assert.throws(()=>validateConversationAnswer({...read,...change},'read',expected,'en'));
  for(const change of [{recheckStatus:'accepted'},{decision:'keep'},{decision:undefined},{explanation:'no relevant words'}])assert.throws(()=>validateConversationAnswer({...follow,...change},'continue',expected,'en'));
  assert.throws(()=>validateConversationAnswer(read,'read',expected,'zh'));
  assert.throws(()=>validateConversationAnswer(null,'read',expected,'en'));
});
test('resuming requires exactly the newly returned native session UUID',()=>{
  const id='00000000-0000-4000-8000-000000000001',event=JSON.stringify({type:'thread.started',thread_id:id});
  assert.equal(nativeSessionId(event+'\n'+JSON.stringify({type:'turn.completed',usage:{}})),id);
  for(const input of ['',JSON.stringify({type:'thread.started',thread_id:'last'}),event+'\n'+event])assert.throws(()=>nativeSessionId(input));
});
