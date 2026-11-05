import assert from 'node:assert/strict';
import { test } from 'node:test';
import { readFile, rename, rm, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { performance } from 'node:perf_hooks';
import type { ConversationRequest, ConversationResult } from '@wombat/client';
import {spawnSync} from 'node:child_process';
import { withLocalProduct, row } from '../helpers/local-product.js';

test('diagnostic tool text is a bounded source read and an opaque result never proves success or failure', {timeout:60000}, async()=>{
 await withLocalProduct(async f=>{
  const error='PRIVATE_SYNTHETIC_FAILURE '+ '中文错误 '.repeat(3000);
  await f.write('task',[
   row('session_meta',{id:'task',cwd:f.project},'2026-10-14T00:00:00Z'),
   row('turn_context',{turn_id:'turn',cwd:f.project},'2026-10-14T00:00:01Z'),
   row('response_item',{type:'message',role:'user',content:[{type:'input_text',text:'Investigate the failure'}]},'2026-10-14T00:00:02Z'),
   row('event_msg',{type:'item_completed',turn_id:'turn',item:{type:'CommandExecution',id:'failed',command:['false'],cwd:f.project,source:'unified_exec_startup',status:'failed',exit_code:1,aggregated_output:error}},'2026-10-14T00:00:03Z'),
   row('response_item',{type:'function_call',status:'completed',call_id:'opaque',name:'synthetic_tool',arguments:'{}'},'2026-10-14T00:00:04Z'),
   row('response_item',{type:'function_call_output',call_id:'opaque',output:[{type:'input_text',text:'PRIVATE_SYNTHETIC_OPAQUE_RESULT'},{type:'input_image',image_url:'PRIVATE_SYNTHETIC_NOT_TEXT'},{type:'output_text',text:'second result part'}]},'2026-10-14T00:00:05Z'),
  ]);
  const usage=await f.client.live!({mode:'fresh',query:{action:'usage',roots:[f.root],scope:{allTime:true}}});
  const req:ConversationRequest={action:'search',roots:[f.root],selection:{projects:[f.project]},query:'PRIVATE_SYNTHETIC_FAILURE'};
  assert.equal((await f.client.conversation!(req)).items.length,0);
  const selected={...req.selection,includeToolOutput:true};
  const found=await f.client.conversation!({...req,selection:selected});
  assert.equal(found.items.length,1);assert.ok(found.items[0].operationId);assert.ok(found.items[0].hasTextLocator);
  const target={evidence:found.items[0].evidence,cursor:found.items[0].readCursor,maxBytes:256};
  const read=await f.client.conversation!({action:'read',roots:[f.root],selection:selected,targets:[target]});
  assert.match(read.fragments[0].text,/PRIVATE_SYNTHETIC_FAILURE/);
  assert.ok(Buffer.byteLength(read.fragments[0].text)<=256);assert.ok(read.fragments[0].nextCursor);
  await assert.rejects(f.client.conversation!({action:'read',roots:[f.root],selection:req.selection,targets:[target]}),{code:'CONTENT_UNAVAILABLE'});
  const thread=found.items[0].evidence.threadId!;
  const opaque=await f.client.conversation!({...req,selection:selected,query:'PRIVATE_SYNTHETIC_OPAQUE_RESULT'});
  assert.equal(opaque.items.length,1);assert.ok(opaque.items[0].operationId);
  const opaqueRead=await f.client.conversation!({action:'read',roots:[f.root],selection:selected,targets:[{evidence:opaque.items[0].evidence,maxBytes:256}]});
  assert.equal(opaqueRead.fragments[0].text,'PRIVATE_SYNTHETIC_OPAQUE_RESULT\nsecond result part');
  assert.equal(opaqueRead.fragments[0].message.contentPartial,true);
  assert.doesNotMatch(JSON.stringify(opaqueRead),/PRIVATE_SYNTHETIC_NOT_TEXT/);
  assert.equal((await f.client.conversation!({...req,query:'PRIVATE_SYNTHETIC_OPAQUE_RESULT'})).items.length,0);
  const timing=await f.client.timing!({action:'summary',roots:[f.root],threadId:thread,turnId:found.items[0].evidence.turnId!,mode:'cached'});
  assert.equal(timing.action,'summary');if(timing.action!=='summary'||timing.profile!=='local')throw new Error('Expected local summary');
  assert.equal(timing.work.outcomes.failed.value,1);assert.equal(timing.work.outcomes.succeeded.value,0);assert.equal(timing.work.outcomes.indeterminate.value,1);
  const cli=spawnSync(process.execPath,[f.entry,'history','read',thread,'--root',f.root,'--tools','--find','PRIVATE_SYNTHETIC_FAILURE','--max-bytes','256','--json'],{env:process.env,encoding:'utf8',timeout:15000});
  assert.ok([0,2].includes(cli.status!),cli.stderr+cli.stdout);
  const text=JSON.parse(cli.stdout);assert.match(text.result.fragments[0].text,/PRIVATE_SYNTHETIC_FAILURE/);
  const after=await f.client.live!({mode:'cached',query:{action:'usage',roots:[f.root],scope:{allTime:true}}});
  assert.deepEqual(after.result.summary,usage.result.summary);
 });
});

test('message reads skip control facts and hide only declared interface context without changing usage', { timeout: 120_000 }, async () => {
  await withLocalProduct(async f => {
    const rows = [row('session_meta', { id: 'task', cwd: f.project }, '2026-10-14T00:00:00Z'), row('turn_context', { turn_id: 'turn', cwd: f.project }, '2026-10-14T00:00:01Z')];
    // Cross-host correctness needs control facts, not thousands of incomplete
    // operations. The core's 6,000-event fixture covers skipped frame boundaries.
    for (let i = 0; i < 256; i++) rows.push(row('event_msg', { type: 'item_completed', turn_id: 'turn', item: { type: 'CommandExecution', id: `control-${i}` } }, '2026-10-14T00:00:02Z'));
    const marker = '<external_codex_apps_open_page>{"page_id":null}</external_codex_apps_open_page>';
    const message = (id: string, text: string, kinds?: string[]) => row('response_item', { type: 'message', role: 'user', id, content: [{ type: 'input_text', text }], internal_chat_message_metadata_passthrough: { turn_id: 'turn', ...(kinds ? { content_item_kinds: kinds } : {}) } }, '2026-10-14T00:00:03Z');
    rows.push(message('ui', marker, ['additional_content.codex_apps_open_page']), message('quoted', `Explain this literal: ${marker}`), message('mixed', 'Keep this user request', ['additional_content.codex_apps_open_page', 'user.text']));
    rows.push(row('event_msg', { type: 'agent_message', phase: 'final_answer', message: 'Visible answer', turn_id: 'turn' }, '2026-10-14T00:00:04Z'));
    rows.push(row('event_msg', { type: 'token_usage_record', thread_id: 'task', turn_id: 'turn', response_id: 'response', usage: { input_tokens: 100, cached_input_tokens: 30, cache_write_input_tokens: 0, output_tokens: 10, reasoning_output_tokens: 0, total_tokens: 110 } }, '2026-10-14T00:00:05Z'));
    await f.write('task', rows);
    const view = await f.client.live!({ mode: 'fresh', query: { action: 'usage', roots: [f.root], scope: { allTime: true } } });
    assert.equal(view.result.summary.tokens.total, 110);
    assert.equal(view.result.summary.measurementCount, 1);
    const base: ConversationRequest = { action: 'locate', selection: { projects: [f.project] } };
    const complete = async (read: (request: ConversationRequest) => Promise<ConversationResult>, request: ConversationRequest) => {
      let cursor: string | null | undefined; const items: ConversationResult['items'] = []; let scannedEvents = 0;
      for (let page = 0; page < 100; page++) {
        const response = await read({ ...request, ...(cursor ? { cursor } : {}) });
        items.push(...response.items); scannedEvents += response.scannedEvents;
        if (response.scanComplete) return { ...response, items, scannedEvents };
        assert(response.nextCursor); cursor = response.nextCursor;
      }
      throw new Error('Message scan failed to advance');
    };
    const check = async (client: typeof f.client) => {
      const located = await complete(request => client.conversation!(request), base);
      assert(located.scanComplete); assert.equal(located.items.length, 3); assert.equal(located.scannedEvents, 4);
      const found = await complete(request => client.conversation!(request), { ...base, action: 'search', query: 'external_codex_apps_open_page' });
      assert.equal(found.items.length, 1, 'A user quotation must remain searchable');
      const explicit = await complete(request => client.conversation!(request), { ...base, selection: { projects: [f.project], origins: ['injected_context'] } });
      assert.equal(explicit.items.length, 1);
      const read = await client.conversation!({ ...base, action: 'read', selection: { projects: [f.project], origins: ['injected_context'] }, targets: [{ evidence: explicit.items[0].evidence }] });
      assert.equal(read.fragments[0].text, marker);
    };
    await check(f.client);
    const nativeApi = await f.native();
    await nativeApi.live!({ mode: 'cached', query: { action: 'usage', scope: { allTime: true } } });
    await check(nativeApi);
    // The control-only fixture has missing operation outcomes; preserve its
    // partial-source status through the CLI while returning usable messages.
    const cli = await complete(async request => JSON.parse(f.cli(['system','call'], JSON.stringify({ method: 'conversation', params: request }), 2)), base);
    assert.equal(cli.partial, true);
    assert.equal(cli.items.length, 3);

    const restored = await f.client.live!({ mode: 'cached', query: { action: 'usage', roots: [f.root], scope: { allTime: true } } });
    assert.equal(restored.result.summary.tokens.total, 110);
    const again = await complete(request => f.client.conversation!(request), { ...base  });
    assert.equal(again.items.length, 3);
  });
});

test('real conversation chain covers no-usage activity, CLI, continuation and source changes', { timeout: 120_000 }, async () => {
  await withLocalProduct(async f => {
    const text = 'PRIVATE_SYNTHETIC_BODY 中文🙂\n' + 'x'.repeat(2 * 1024 * 1024);
    const rows = [row('session_meta', { id: 'task', cwd: f.project }, '2026-10-14T00:00:00Z'), row('turn_context', { turn_id: 'turn', cwd: f.project }, '2026-10-14T00:00:01Z'), row('event_msg', { type: 'agent_message', phase: 'final_answer', message: text, turn_id: 'turn' }, '2026-10-14T00:00:02Z')];
    await f.write('task', rows);
    const prepare = () => f.client.live!({ query: { action: 'usage', roots: [f.root], scope: { allTime: true } }, mode: 'fresh' });
    const view = await prepare(); assert.ok(['current','stale'].includes(view.freshness.status));
    const id = view.result.snapshotRef.snapshotId;
    const base: ConversationRequest = { action: 'locate', selection: { projects: [f.project], since: '2026-10-14', until: '2026-10-15', timezone: 'Asia/Shanghai' } };
    const located = await f.client.conversation!(base); assert.equal(located.items.length, 1); assert.equal(located.sourceBytesRead, 0); assert(located.scanComplete);
    assert(!JSON.stringify(located).includes('PRIVATE_SYNTHETIC_BODY'));
    const target = located.items[0].evidence;
    const activity = await f.client.conversation!({ ...base, action: 'activity' }); assert(activity.items.some(i => i.kind === 'message'));
    const methods = JSON.parse(f.cli(['system','api'])); assert(methods.methods.includes('conversation'));
    const cliLocated: ConversationResult = JSON.parse(f.cli(['system','call'], JSON.stringify({ method: 'conversation', params: base }))); assert.equal(cliLocated.items[0].evidence.eventId, target.eventId);
    const read: ConversationRequest = { ...base, action: 'read', targets: [{ evidence: target, maxBytes: 256 * 1024 }] };
    let result = await f.client.conversation!(read), rebuilt = result.fragments[0].text, bytes = result.sourceBytesRead;
    while (result.fragments[0].nextCursor) {
      read.targets![0].cursor = result.fragments[0].nextCursor;
      result = await f.client.conversation!(read); rebuilt += result.fragments[0].text; bytes += result.sourceBytesRead;
    }
    assert.equal(rebuilt, text); assert(bytes < Buffer.byteLength(text) + 32768, 'Large-message continuation must not replay whole records');
    await assert.rejects(f.client.conversation!({ ...base, action: 'read', targets: [{ evidence: target }], selection: { projects: [path.join(f.dir, 'unrelated')] } }), { code: 'NOT_FOUND' });
    const nativeApi = await f.native();
    assert.ok((await nativeApi.conversation!({...base})).items.length);
    assert.equal((await nativeApi.conversation!({ ...base, roots: [f.root] })).snapshotId,id);
    await nativeApi.live!({ query: { action: 'usage', scope: { allTime: true } }, mode: 'cached' });
    const browserLocated = await nativeApi.conversation!(base); assert.equal(browserLocated.items[0].evidence.eventId, target.eventId);
    const browserRead = await nativeApi.conversation!({ ...base, action: 'read', targets: [{ evidence: target, maxBytes: 64 }] }); assert(browserRead.fragments[0].text.startsWith('PRIVATE_SYNTHETIC_BODY 中文🙂'));
    const cross = await f.client.conversation!({ ...base, action: 'search', query: '中文🙂' }); assert.equal(cross.items.length, 1);
    await f.append('task', [row('event_msg', { type: 'user_message', message: 'next', turn_id: 'turn' }, '2026-10-14T01:00:00Z')]);
    const later = await prepare();
    const updated=await f.client.conversation!({...base,action:'locate'});assert.equal(updated.items.filter(i=>i.kind==='message').length,2);
    const source = path.join(f.root, 'sessions/task.jsonl');
    const replacement = source + '.replacement'; await writeFile(replacement, await readFile(source)); await rename(replacement, source);
    await assert.rejects(f.client.conversation!({ ...base, action: 'read', targets: [{ evidence: target }] }), { code: 'NOT_FOUND' });
    await rm(source);
    await assert.rejects(f.client.conversation!({ ...base, action: 'read', targets: [{ evidence: target }] }), { code: 'NOT_FOUND' });
  });
});

test('real RPC navigation remains bounded across many messages and service restart', { timeout: 120_000 }, async () => {
  await withLocalProduct(async f => {
    const rows = [row('session_meta', { id: 'task', cwd: f.project }, '2026-10-14T00:00:00Z'), row('turn_context', { turn_id: 'turn', cwd: f.project }, '2026-10-14T00:00:01Z')];
    for (let i = 0; i < 3000; i++) rows.push(row('event_msg', { type: 'agent_message', message: `message ${i} ` + 'x'.repeat(1024), phase: 'final_answer', turn_id: 'turn' }, '2026-10-14T00:00:02Z'));
    await f.write('many', rows);
    let view = await f.client.live!({ query: { action: 'usage', roots: [f.root], scope: { allTime: true } }, mode: 'fresh' });
    const times: number[] = [];
    for (let i = 0; i < 30; i++) {
      const start = performance.now();
      const result = await f.client.conversation!({ action: 'locate', selection: { projects: [f.project] }, limit: 3 });
      assert.equal(result.items.length, 3, JSON.stringify({limits:result.limits,complete:result.scanComplete,partial:result.partial,scanned:result.scannedEvents,facets:view.result.facets,quality:view.result.quality})); assert.equal(result.sourceBytesRead, 0); times.push(performance.now() - start);
    }
    if (process.env.WOMBAT_CONVERSATION_BENCHMARK_OUTPUT) await writeFile(process.env.WOMBAT_CONVERSATION_BENCHMARK_OUTPUT, JSON.stringify({ source: '3000 synthetic messages, about 3 MiB; fresh product-data directory', platform: `${process.platform}/${process.arch}`, method: 'real Node client to release Rust service; first and repeated RPCs; uncontrolled OS cache', firstMs: times[0], p95Ms: [...times.slice(1)].sort((a,b)=>a-b)[Math.ceil((times.length-1)*.95)-1], maximumMs: Math.max(...times), samplesMs: times }, null, 2));

    view = await f.client.live!({ query: { action: 'usage', roots: [f.root], scope: { allTime: true } }, mode: 'fresh' });
    const request: ConversationRequest = { action: 'locate', selection: { projects: [f.project] }, limit: 2 };
    const first = await f.client.conversation!(request); assert(first.nextCursor);
    const second = await f.client.conversation!({ ...request, cursor: first.nextCursor }); assert.equal(second.items.length, 2); assert.notEqual(second.items[0].evidence.eventId, first.items[0].evidence.eventId);
    await assert.rejects(f.client.conversation!({ ...request, cursor: first.nextCursor, selection: { projects: [f.project], phases: ['commentary'] } }), { code: 'CURSOR_MISMATCH' });
    const restored = await f.client.conversation!({ ...request, action: 'read', limit: undefined, targets: [{ evidence: second.items[0].evidence, maxBytes: 64 }] }); assert(restored.fragments[0].text.startsWith('message '));
  });
});
