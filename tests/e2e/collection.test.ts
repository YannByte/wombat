import {test} from 'node:test';
import assert from 'node:assert/strict';
import {realpathSync} from 'node:fs';
import {mkdtemp,mkdir,writeFile,readFile,rm} from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import {spawn,spawnSync} from 'node:child_process';
import {once} from 'node:events';
import {createNodeClient,receiveCodexHook} from '@wombat/client/node';
import {createHttpClient} from '@wombat/client/http';
import {startWebHost} from '@wombat/web';
import {nativeCodexFixture} from '../helpers/native-codex.js';

test('collection CLI, Rust, Web and setup retain privacy, scope and the log ledger', {timeout:60000}, async()=>{
 const dir=realpathSync.native(await mkdtemp(path.join(os.tmpdir(),'wombat-collection-'))),source=path.join(dir,'source'),project=path.join(dir,'project'),outside=path.join(dir,'outside'),foreign=path.join(dir,'foreign');
 for(const directory of [path.join(source,'sessions'),project,outside,path.join(foreign,'sessions')])await mkdir(directory,{recursive:true});
 const records=[
  {timestamp:'2026-10-01T00:00:00Z',type:'session_meta',payload:{id:'synthetic-session',cwd:project}},
  {timestamp:'2026-10-01T00:00:01Z',type:'event_msg',payload:{type:'token_usage_record',thread_id:'synthetic-session',turn_id:'turn-1',response_id:'response-1',usage:{input_tokens:100,cached_input_tokens:0,cache_creation_input_tokens:0,output_tokens:10,reasoning_output_tokens:0,total_tokens:110}}},
 ];
 await writeFile(path.join(source,'sessions/log.jsonl'),records.map(r=>JSON.stringify(r)).join('\n')+'\n');
 const native=await nativeCodexFixture(dir);await writeFile(native.mode,JSON.stringify({version:'0.160.1',skills:[{name:'wombat-collection:wombat',path:path.join(project,'skills/wombat/SKILL.md'),enabled:true}]}));
 const old={WOMBAT_DATA_HOME:process.env.WOMBAT_DATA_HOME,CODEX_HOME:process.env.CODEX_HOME,WOMBAT_AUTO_PRICES:process.env.WOMBAT_AUTO_PRICES,WOMBAT_CODEX_BIN:process.env.WOMBAT_CODEX_BIN,WOMBAT_CORE_BIN:process.env.WOMBAT_CORE_BIN};
 const binary=path.resolve('dist',process.platform==='win32'?'wombat-core.exe':'wombat-core'),entry=path.resolve('dist/wombat.js');
 Object.assign(process.env,{WOMBAT_DATA_HOME:path.join(dir,'data'),CODEX_HOME:source,WOMBAT_AUTO_PRICES:'0',WOMBAT_CODEX_BIN:native.binary,WOMBAT_CORE_BIN:binary});
 const service=spawn(binary,['--serve-usage'],{env:process.env,stdio:'ignore'});await once(service,'spawn');
 const client=createNodeClient({binaryPath:binary,codexBinaryPath:native.binary,automaticPrices:false});let host:Awaited<ReturnType<typeof startWebHost>>|undefined;
 const run=(args:string[],input='')=>{const result=spawnSync(process.execPath,[entry,...args],{env:process.env,input,encoding:'utf8',timeout:15000,maxBuffer:2*1024*1024});assert.ifError(result.error);assert.ok(!/PRIVATE_INPUT|PRIVATE_OUTPUT|PRIVATE_PROMPT/.test(result.stdout+result.stderr));return result;};
 try {
  await client.setup!({project});
  await assert.rejects(client.live!({query:{action:'usage',roots:[source]},mode:'cached'}),{code:'NO_SNAPSHOT'});
  const before=(await client.live!({query:{action:'usage',roots:[source],scope:{allTime:true},limit:1},mode:'fresh'})).result;assert.equal(before.summary.tokens.total,110);
  assert.equal((await client.collection!({})).state,'logs_only');
  const mode=run(['collection','mode','hooks','--json']);assert.equal(mode.status,0);assert.equal(JSON.parse(mode.stdout).state,'waiting');
  const body={hook_event_name:'PostToolUse',session_id:'synthetic-session',turn_id:'turn-1',tool_use_id:'call-1',cwd:project,tool_input:{secret:'PRIVATE_INPUT'},tool_response:'PRIVATE_OUTPUT',prompt:'PRIVATE_PROMPT'};
  assert.equal(run(['hook','codex'],JSON.stringify(body)).stdout,'');
  await Promise.all(Array.from({length:8},()=>receiveCodexHook(body,{}, {binaryPath:binary})));
  assert.equal((await client.collection!({})).received,1);
  await client.collection!({action:'pause'});await receiveCodexHook({...body,tool_use_id:'call-2'},{},{binaryPath:binary});assert.equal((await client.collection!({})).buffered,1);
  const resumed=await client.collection!({action:'resume'});assert.equal(resumed.buffered,0);assert.equal(resumed.received,2);
  for(const lang of ['zh','en']) {const text=run(['collection','status','--lang',lang]);assert.equal(text.status,0);assert.match(text.stdout,lang==='zh'?/已收到事件/:/Events received/);}
  host=await startWebHost({client,assets:path.resolve('dist/web'),roots:[source],projectRoots:[project],automaticPrices:false});
  const token=new URLSearchParams(new URL(host.url).hash.slice(1)).get('token')!;
  const browser=createHttpClient({origin:host.origin,token,fetch:(input,init)=>fetch(input,{...init,headers:{...init?.headers,Origin:host!.origin}})});
  const events=await browser.collection!({action:'events',project,limit:1});assert.equal(events.events.length,1);assert.ok(events.nextAfter);assert.equal(events.events[0].observation.sessionId,'synthetic-session');assert.equal(events.events[0].association.state,'linked');assert.ok(events.events[0].association.threadId);assert.ok(events.events[0].association.turnId);assert.ok(events.events[0].association.sourceEpoch);
  const selected=await browser.collection!({sourceInstanceId:events.events[0].observation.sourceInstanceId,project});assert.equal(selected.received,2);
  await assert.rejects(browser.collection!({sourceInstanceId:'foreign-source',project}),{code:'SOURCE_NOT_AUTHORIZED'});
  const next=await browser.collection!({action:'events',project,after:events.nextAfter,limit:1});assert.equal(next.events.length,1);assert.equal(next.nextAfter,null);
  await assert.rejects(browser.collection!({roots:[outside]}),{code:'INVALID_ARGUMENT'});await assert.rejects(browser.collection!({project:outside}),{code:'PROJECT_NOT_AUTHORIZED'});
  await assert.rejects(browser.setup!({roots:[outside],project}),{code:'INVALID_ARGUMENT'});
  const setup=await browser.setup!({project});assert.equal(setup.nativeVersion,'0.160.1');assert.ok(path.isAbsolute(setup.marketplacePath!));assert.equal(setup.discovery.instances[0].name,'wombat-collection:wombat');assert.equal(setup.discovery.status,'available');assert.ok(setup.runtimeCapabilities.includes('collection-json-v1'));
  const calls=(await readFile(native.calls,'utf8')).trim().split('\n').map(l=>JSON.parse(l).method);assert.ok(!calls.some(m=>m.startsWith('thread/')));
  const empty=await client.collection!({roots:[foreign]});assert.equal(empty.received,0);assert.equal(empty.lastReceivedAt,null);
  const after=(await client.live!({query:{action:'usage',roots:[source],scope:{allTime:true},limit:1},mode:'fresh'})).result;assert.equal(after.summary.tokens.total,110);assert.equal(after.summary.measurementCount,before.summary.measurementCount);
  assert.equal(run(['hook','codex'],'{"PRIVATE_INPUT":').status,0);assert.equal(run(['hook','codex'],'x'.repeat(64*1024+1)).status,0);
  await host.close();host=undefined;
  const bytes=await readFile(path.join(dir,'data/collection-v1/observations.sqlite'));assert.ok(!/PRIVATE_INPUT|PRIVATE_OUTPUT|PRIVATE_PROMPT/.test(bytes.toString()));
 } finally {await host?.close();if(service.exitCode===null&&service.signalCode===null){service.kill('SIGTERM');await once(service,'exit');}await native.waitForExit();for(const [key,value]of Object.entries(old)){if(value===undefined)delete process.env[key];else process.env[key]=value;}await rm(dir,{recursive:true,force:true});}
});
