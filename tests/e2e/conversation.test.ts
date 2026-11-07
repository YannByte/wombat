import assert from 'node:assert/strict';
import { test } from 'node:test';
import { createHash } from 'node:crypto';
import { access, readFile, rename, rm, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { performance } from 'node:perf_hooks';
import type { ConversationRequest, ConversationResult, HistoryRead } from '@wombat/client';
import {spawnSync} from 'node:child_process';
import { withLocalProduct, row } from '../helpers/local-product.js';

test('default search reads original messages on a fresh data home and binds continuations', async()=>{
  await withLocalProduct(async f=>{
    const at='2026-10-14T00:00:02Z';
    const user=(text:string, extra={})=>row('response_item',{
      type:'message',role:'user',content:[{type:'input_text',text}],...extra,
    },at);
    await f.write('a',[
      row('session_meta',{id:'a',cwd:f.project},at),
      user('先试试。最终保留 SQLite 和引用。'),
      user('保留 injected',{internal_chat_message_metadata_passthrough:{content_item_kinds:['agents_md.instructions']}}),
      {...user('保留 inherited'),metadata:{inherited_user_message:true}},
      user('保留 assistant',{role:'assistant'}),
      row('response_item',{type:'function_call_output',output:'保留 hidden output'},at),
    ]);
    await f.write('b',[
      row('session_meta',{id:'b',cwd:f.project},at),
      {timestamp:at,ordinal:2,type:'event_msg',payload:{type:'item_completed',thread_id:'b',turn_id:'u',
        item:{type:'UserMessage',content:[{type:'input_text',text:'保留按需读取。'}]}}},
      row('event_msg',{type:'user_message',message:'保留 older'},'2026-09-01T00:00:02Z'),
    ]);
    await f.write('other',[row('session_meta',{id:'other',cwd:f.dir},at),user('保留 from another project')]);
    const args=['history','search','保留','SQLite','--root',f.root,'--project',f.project,'--speaker','user',
      '--since','2026-10-14','--until','2026-10-15','--timezone','UTC','--limit','1','--max-bytes','30','--json'];
    const first:ConversationResult=JSON.parse(f.cli(args));
    assert.equal(first.action,'source_search');
    assert.equal(first.items.length,0);
    assert.ok(first.sourceHits?.length===1 && first.sourceScope && first.nextSearch);
    const [hit]=first.sourceHits;
    const file=first.sourceFiles?.[hit.fileIndex];
    assert.ok(file && file.upstreamId==='a');
    const bytes=Buffer.from(hit.text);
    assert.equal(bytes.subarray(hit.matchRange.start,hit.matchRange.end).toString(),'保留');
    const raw=(await readFile(file.file)).subarray(hit.byteStart,hit.byteEnd);
    assert.equal(createHash('sha256').update(raw).digest('hex'),hit.sha256);
    assert.equal(JSON.parse(raw.toString()).payload.content[0].text,'先试试。最终保留 SQLite 和引用。');
    const next=await f.client.conversation!(first.nextSearch);
    assert.deepEqual(next.sourceHits?.map(i=>next.sourceFiles?.[i.fileIndex].upstreamId),['b']);
    assert.equal(next.sourceScope,first.sourceScope);
    const done=next.nextSearch ? await f.client.conversation!(next.nextSearch) : next;
    assert.ok(done.scanComplete && !done.partial);
    await assert.rejects(access(f.data),{code:'ENOENT'});
    await f.append('b',[user('保留 new decision')]);
    await assert.rejects(f.client.conversation!(first.nextSearch),{code:'SOURCE_CHANGED'});
    await assert.rejects(access(f.data),{code:'ENOENT'});
  });
});

test('complete CLI search hands off many hits and reads visible context across turns', {timeout:120_000}, async()=>{
  await withLocalProduct(async f=>{
    const base='2026-10-14T00:00:';
    const at=(second:number)=>base+String(second).padStart(2,'0')+'Z';
    const user=(message:string,turn_id:string,second:number)=>row('event_msg',{type:'user_message',message,turn_id},at(second));
    const assistant=(message:string,turn_id:string,second:number)=>row('event_msg',{type:'agent_message',message,turn_id,phase:'final_answer'},at(second));
    const turn=(turn_id:string,second:number)=>row('turn_context',{turn_id,cwd:f.project},at(second));
    await f.write('decision',[
      row('session_meta',{id:'decision',cwd:f.project},at(0)),
      turn('proposal',1),user('先考虑移除。','proposal',2),assistant('建议保留本地 SQLite。','proposal',3),
      turn('confirmation',4),
      ...Array.from({length:256},()=>row('event_msg',{type:'agent_reasoning',text:'non-visible',turn_id:'confirmation'},at(5))),
      user('MARK 按这个做。','confirmation',6),
      ...Array.from({length:20},(_,i)=>user(`MARK follow-up ${i}`,'confirmation',7)),
    ]);
    await f.write('other',[
      row('session_meta',{id:'other',cwd:f.project},at(0)),turn('other-turn',1),
      user('MARK independent chat','other-turn',2),
    ]);
    const scope=['MARK','--root',f.root,'--project',f.project,
      '--since','2026-10-14','--until','2026-10-15','--timezone','UTC','--fixed','--complete','--speaker','user','--json'];
    const directArgs=['history','search','--context','1',...scope];
    const direct:ConversationResult=JSON.parse(f.cli(directArgs));
    assert.ok(direct.scanComplete && !direct.partial && direct.sourceHits?.length===22);
    const page1:ConversationResult=JSON.parse(f.cli([...directArgs,'--limit','1']));
    assert.equal(page1.nextSearch?.contextMessages,1);
    assert.ok(page1.nextSearch);
    const page2=await f.client.conversation!(page1.nextSearch);
    const later=page2.sourceHits?.[0];
    assert.equal(later?.text,'MARK follow-up 0');
    assert.deepEqual(later?.context?.before.map(i=>page2.sourceNeighbors?.[i].text),['MARK 按这个做。']);
    await assert.rejects(access(f.data),{code:'ENOENT'});

    const args=['history','search','--locate',...scope];
    const search=f.cli(args);
    const matches:ConversationResult=JSON.parse(search);
    assert.ok(matches.scanComplete && matches.sourceScope);
    assert.equal(matches.items.length,22);
    const output:HistoryRead=JSON.parse(f.cli(['history','read','--from','-','--context','1','--json'],search));
    const batch=output.result;
    assert.equal(batch.sourceScope,matches.sourceScope);
    assert.equal(batch.fragments.length,22);
    const approve=batch.fragments.find(i=>i.text==='MARK 按这个做。')!;
    assert.ok(approve);
    const previous=batch.neighbors?.find(n=>n.message.evidence.eventId===approve.visibleContext?.before[0]);
    assert.equal(previous?.text,'建议保留本地 SQLite。');
    assert.equal(previous?.message.origin,'assistant_visible');
    assert.notEqual(previous?.message.evidence.turnId,approve.evidence.turnId);
    assert.equal(batch.neighbors?.find(n=>n.message.evidence.eventId===approve.visibleContext?.after[0])?.text,'MARK follow-up 0');
    assert.equal(new Set(batch.neighbors?.map(n=>n.message.evidence.eventId)).size,batch.neighbors?.length);
    assert.equal(batch.readIssues,undefined);
    const byId=new Map(batch.neighbors?.map(n=>[n.message.evidence.eventId,n]));
    for (const hit of direct.sourceHits) {
      const file=direct.sourceFiles![hit.fileIndex];
      const contains=(range:{byteStart:number;byteEnd:number}, body:{locations:{file:string;byteStart:number;byteEnd:number}[]}) =>
        body.locations.length>0 && body.locations.every(l=>l.file===file.file && range.byteStart<=l.byteStart && l.byteEnd<=range.byteEnd);
      const matched=batch.fragments.find(i=>contains(hit,i));
      assert.ok(matched && hit.context && matched.visibleContext);
      assert.equal(hit.text,matched.text);
      for (const side of ['before','after'] as const) {
        const indices: number[]=hit.context[side];
        const ids: string[]=matched.visibleContext[side];
        assert.equal(indices.length,ids.length);
        for (const [j,index] of indices.entries()) {
          const neighbor=direct.sourceNeighbors![index], indexed=byId.get(ids[j])!;
          assert.equal(neighbor.text,indexed.text);
          assert.equal(neighbor.origin,indexed.message.origin);
          assert.ok(contains(neighbor,indexed));
        }
      }
    }

    const page=f.cli([...args,'--limit','1']);
    const pending:HistoryRead=JSON.parse(f.cli(['history','read','--from','-','--json'],page,2));
    assert.equal(pending.nextLocations?.action,'search');
    assert.equal(pending.nextLocations?.sourceScope,matches.sourceScope);
    const continued=await f.client.conversation!({...pending.nextLocations!,limit:200});
    assert.equal(continued.items.length,21);
    const denied=JSON.parse(f.cli(['history','read','--from','-','--project',f.project,'--json'],search,1));
    assert.equal(denied.error.code,'INVALID_ARGUMENT');
  });
});

test('expanded search scans once and batched reads retain every occurrence and continuation', { timeout: 120_000 }, async () => {
  await withLocalProduct(async f => {
    const shared = '已确认保留 SQLite。';
    const distinct = '已确认使用按需模式。';
    const message = (text: string, seconds: number) => row('response_item',
      {type:'message',role:'user',content:[{type:'input_text',text}]},
      `2026-10-14T00:00:0${seconds}Z`);
    await f.write('task', [
      row('session_meta', {id:'task',cwd:f.project}, '2026-10-14T00:00:00Z'),
      row('turn_context', {turn_id:'turn',cwd:f.project}, '2026-10-14T00:00:01Z'),
      message(shared, 2), message(distinct, 3), message(shared, 4),
    ]);
    const scope = ['--root',f.root,'--project',f.project,'--since','2026-10-14','--until','2026-10-15','--timezone','UTC','--json'];
    const terms = ['SQLite','已确认保留','按需模式','SQLite'];
    const search = (query: string[], extra: string[] = []): ConversationResult =>
      JSON.parse(f.cli(['history','search','--locate',...scope,...extra,'--',...query]));
    const found = search(terms);
    assert.deepEqual(found.items.map(i=>i.matchedTerm), ['已确认保留','按需模式','已确认保留']);
    assert.equal(new Set(found.items.map(i=>i.evidence.eventId)).size, 3);
    assert.equal(found.sourceBytesRead, search(['NO_MATCH']).sourceBytesRead);
    const page = search(terms,['--limit','1']);
    assert.ok(page.nextCursor);
    assert.equal(search(terms,['--limit','1','--cursor',page.nextCursor]).items[0].evidence.eventId, found.items[1].evidence.eventId);

    const read: ConversationRequest = {action:'read',roots:[f.root],selection:found.selection,
      targets:[...found.items,found.items[0]].map(i=>({evidence:i.evidence,maxBytes:15}))};
    const batch = await f.client.conversation!(read);
    assert.deepEqual(batch.fragments.map(i=>i.text), ['已确认保留','已确认使用']);
    const [first, second] = batch.fragments;
    const repeated = first.alsoAt?.[0];
    assert.ok(repeated);
    assert.deepEqual(first.targetIndices, [0,3]);
    assert.deepEqual(second.targetIndices, [1]);
    assert.deepEqual(repeated.targetIndices, [2]);
    assert.notEqual(first.locations[0].byteStart, repeated.locations[0].byteStart);
    assert.notEqual(first.nextCursor,repeated.nextCursor);
    assert.ok(first.nextCursor && repeated.nextCursor);
    const continued = await f.client.conversation!({...read,targets:[first,repeated].map(i=>({evidence:i.evidence,cursor:i.nextCursor}))});
    assert.equal(continued.fragments[0].text,' SQLite。');
    assert.equal(continued.fragments[0].alsoAt?.length,1);
    const once = await f.client.conversation!({...read,targets:read.targets!.slice(0,3)});
    assert.equal(once.sourceBytesRead,batch.sourceBytesRead);
    const divergent = await f.client.conversation!({...read,targets:read.targets!.slice(0,2).map(i=>({...i,maxBytes:9}))});
    assert.equal(divergent.fragments.length,1);
    const branches = [divergent.fragments[0], ...divergent.fragments[0].alsoAt ?? []];
    const next = await f.client.conversation!({...read,targets:branches.map(i=>({evidence:i.evidence,cursor:i.nextCursor}))});
    assert.deepEqual(next.fragments.map(i=>i.text), ['保留 SQLite。','使用按需模式。']);

    const cli = JSON.parse(f.cli(['history','read',found.items[0].evidence.threadId!,...scope,
      '--find','SQLite','--find','按需模式','--limit','3'])).result as ConversationResult;
    assert.deepEqual(cli.fragments.map(i=>i.text), [shared,distinct]);
    const range = cli.fragments[0].matchRange!;
    assert.equal(Buffer.from(cli.fragments[0].text).subarray(range.start,range.end).toString(),'SQLite');
    assert.equal(cli.fragments[0].alsoAt?.length,1);
    await assert.rejects(f.client.conversation!({action:'search',selection:found.selection,terms,query:'conflict'}), {code:'INVALID_ARGUMENT'});
    await assert.rejects(f.client.conversation!({action:'search',selection:found.selection,terms:['   ']}), {code:'INVALID_ARGUMENT'});
  });
});

test('a changed target and the output budget preserve other batch reads', { timeout: 120_000 }, async () => {
  await withLocalProduct(async f => {
    const at = '2026-10-14T00:00:02Z';
    const write = (id: string, texts: string[]) => f.write(id, [
      row('session_meta',{id,cwd:f.project},at), row('turn_context',{turn_id:id+'-turn',cwd:f.project},at),
      ...texts.map(message=>row('event_msg',{type:'user_message',message,turn_id:id+'-turn'},at)),
    ]);
    await write('healthy',Array.from({length:17},(_,i)=>`Message ${i}\n`+'x'.repeat(70000)));
    await write('replace',['Original location']);
    const base: ConversationRequest = {action:'locate',roots:[f.root],selection:{projects:[f.project]}};
    const items: ConversationResult['items'] = [];
    let cursor: string | null | undefined;
    for(let page=0;page<40;page++){
      const located = await f.client.conversation!({...base,cursor});
      items.push(...located.items);
      if(located.scanComplete)break;
      cursor=located.nextCursor;
      assert.ok(cursor);
    }
    assert.equal(items.length,18);
    const {record} = await f.client.conversation!({...base,action:'resolve',query:'replace'});
    const stale = items.find(i=>i.evidence.threadId===record?.id)!;
    assert.ok(stale);
    const healthy = items.filter(i=>i!==stale);
    const old = path.join(f.root,'sessions','replace.jsonl');
    await rename(old,old+'.old');
    await write('replace',['Replacement at the same path']);
    const targets = [...healthy.slice(0,1),stale,...healthy.slice(1)].map(i=>({evidence:i.evidence,maxBytes:65536}));
    const request: ConversationRequest = {...base,action:'read',targets};
    const result: ConversationResult = JSON.parse(f.cli(['system','call'],JSON.stringify({method:'conversation',params:request}),2));
    assert.ok(result.fragments.length>=1);
    assert.deepEqual(result.readIssues?.find(i=>i.targetIndices.includes(1)),{
      targetIndices:[1],eventId:stale.evidence.eventId,code:'NOT_FOUND',deferred:false,
    });
    const deferred = result.readIssues?.filter(i=>i.deferred).flatMap(i=>i.targetIndices)??[];
    assert.ok(deferred.length>0);
    const retry = await f.client.conversation!({...request,targets:deferred.map(i=>targets[i])});
    assert.equal(retry.fragments.length,deferred.length);
    assert.equal(retry.readIssues,undefined);
    const fromCli = JSON.parse(f.cli(['history','read','healthy','--root',f.root,'--limit','17',
      '--max-bytes','65536','--json'],'',2)) as HistoryRead;
    assert.ok(fromCli.result.fragments.length);
    assert.ok(fromCli.retryRead?.targets?.length);
    const resumed = await f.client.conversation!(fromCli.retryRead!);
    assert.equal(resumed.readIssues,undefined);
  });
});

test('fixed evidence pages and reads reject selected changes while other projects can advance', { timeout: 120_000 }, async () => {
  await withLocalProduct(async f => {
    const at = '2026-10-14T00:00:02Z';
    const msg = (turn: string, message: string) => row('event_msg',{type:'user_message',turn_id:turn,message},at);
    const write = (id: string, project: string, message: string) => f.write(id, [
      row('session_meta',{id,cwd:project},at), row('turn_context',{turn_id:id+'-turn',cwd:project},at),
      msg(id+'-turn',message),
    ]);
    await write('a',f.project,'本轮没有失败，验收通过。');
    await write('b',f.project,'此前的失败已经恢复。');
    await write('other',path.join(f.dir,'another-project'),'另一个项目没有失败。');
    const flags=['--root',f.root,'--project',f.project,'--since','2026-10-14','--until','2026-10-15',
      '--timezone','UTC','--fixed','--json'];
    const scan = (extra: string[] = []): ConversationResult =>
      JSON.parse(f.cli(['history','search','--locate','失败',...flags,...extra]));
    const first=scan(['--limit','1']);
    assert.ok(first.nextCursor && first.sourceScope);
    const locate=first.items[0];
    const request:ConversationRequest={action:'read',roots:[f.root],selection:first.selection,sourceScope:first.sourceScope,
      targets:[{evidence:locate.evidence,cursor:locate.readCursor}]};
    const original=await f.client.conversation!(request);
    assert.match(original.fragments[0].text, /^(本轮没有失败，验收通过。|此前的失败已经恢复。)$/u);
    await f.append('other',[msg('other-turn','另一条失败。')]);
    const second=scan(['--limit','1','--cursor',first.nextCursor]);
    assert.notEqual(second.snapshotId,first.snapshotId);
    assert.equal(second.sourceScope,first.sourceScope);
    assert.notEqual(second.items[0].evidence.eventId,locate.evidence.eventId);
    const firstId=first.items[0].evidence.threadId;
    const {record}=await f.client.conversation!({action:'resolve',roots:[f.root],query:'a'});
    const changed=record?.id===firstId?'a':'b';
    await f.append(changed,[msg(changed+'-turn','新增一条失败。')]);
    await assert.rejects(f.client.conversation!(request),{code:'SOURCE_CHANGED'});
    const error=JSON.parse(f.cli(['history','search','--locate','失败',...flags,'--limit','1','--cursor',first.nextCursor],'',1));
    assert.equal(error.error.code,'SOURCE_CHANGED');
    const fresh=scan();
    assert.equal(fresh.items.length,3);
    assert.notEqual(fresh.sourceScope,first.sourceScope);
  });
});

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
