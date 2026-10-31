import {test} from 'node:test';
import assert from 'node:assert/strict';
import path from 'node:path';
import {tmpdir} from 'node:os';
import {parseConversationArgs,nativeSessionId} from './verify-skill-conversation-helpers.ts';
test('conversation arguments require explicit executables and reject ambiguous or unsupported modes',()=>{
  const args=['--output-dir',path.join(tmpdir(),'conversation-output'),'--agent-bin',process.execPath];
  assert.deepEqual(parseConversationArgs(['--',...args]).languages,['en','zh']);
  assert.deepEqual(parseConversationArgs([...args,'--language','zh']).languages,['zh']);
  assert.equal(parseConversationArgs(args).scenarios[0],'usage');
  assert.equal(parseConversationArgs([...args,'--scenario','all']).scenarios.length,9);
  assert.equal(parseConversationArgs([...args,'--scenario','pricing','--variant','3']).variant,3);
  assert.deepEqual(parseConversationArgs([...args,'--scenario','timing']).scenarios,['timing']);
  for(const extra of [['--language','fr'],['--last'],['--agent-bin',process.execPath],['--language'],['--unknown','value']])assert.throws(()=>parseConversationArgs([...args,...extra]));
  assert.throws(()=>parseConversationArgs(['--output-dir','relative','--agent-bin',process.execPath]));
  assert.throws(()=>parseConversationArgs(['--output-dir',path.join(tmpdir(),'conversation-output'),'--agent-bin','codex']));
  for(const extra of [['--scenario','unknown'],['--variant','-1'],['--variant','4'],['--variant','1.0'],['--variant','no']])assert.throws(()=>parseConversationArgs([...args,...extra]));
});
test('resuming requires exactly the newly returned native session UUID',()=>{
  const id='00000000-0000-4000-8000-000000000001',event=JSON.stringify({type:'thread.started',thread_id:id});
  assert.equal(nativeSessionId(event+'\n'+JSON.stringify({type:'turn.completed',usage:{}})),id);
  for(const input of ['',JSON.stringify({type:'thread.started',thread_id:'last'}),event+'\n'+event])assert.throws(()=>nativeSessionId(input));
});
