import {test} from 'node:test';
import assert from 'node:assert/strict';
import { realpathSync } from 'node:fs';
import {mkdtemp,mkdir,rm,writeFile,readFile} from 'node:fs/promises';
import {tmpdir} from 'node:os';
import path from 'node:path';
import {spawn,spawnSync} from 'node:child_process';
import {once} from 'node:events';
import {DatabaseSync} from 'node:sqlite';
import {createNodeClient} from '@wombat/client/node';
import {createHttpClient} from '@wombat/client/http';
import {startWebHost} from '@wombat/web';

test('index initialization faults return actual errors, retain unsupported records and recover in the same service',{timeout:20_000},async()=>{
 const dir=realpathSync.native(await mkdtemp(path.join(tmpdir(),'wombat-startup-fault-'))),root=path.join(dir,'source'),project=path.join(dir,'project'),data=path.join(dir,'data'),database=path.join(data,'live-v2/index.sqlite');
 await mkdir(path.join(root,'sessions'),{recursive:true});await mkdir(path.dirname(database),{recursive:true});
 await mkdir(project);const instructions=path.join(project,'AGENTS.md');await writeFile(instructions,'Synthetic local instructions. '.repeat(1000));await writeFile(path.join(root,'AGENTS.md'),'Synthetic global instructions.');
 const unsupported=new DatabaseSync(database);unsupported.exec("CREATE TABLE sample(value TEXT); INSERT INTO sample VALUES('retained'); PRAGMA user_version=99;");unsupported.close();
 const previous={WOMBAT_DATA_HOME:process.env.WOMBAT_DATA_HOME,CODEX_HOME:process.env.CODEX_HOME,WOMBAT_AUTO_PRICES:process.env.WOMBAT_AUTO_PRICES};Object.assign(process.env,{WOMBAT_DATA_HOME:data,CODEX_HOME:root,WOMBAT_AUTO_PRICES:'0'});
 const binary=path.resolve('dist',process.platform==='win32'?'wombat-core.exe':'wombat-core'),service=spawn(binary,['--serve-usage'],{stdio:'ignore',env:process.env});await once(service,'spawn');
 const client=createNodeClient({binaryPath:binary,automaticPrices:false});
 const host=await startWebHost({client,roots:[root],projectRoots:[project],assets:path.resolve('dist/web'),automaticPrices:false});
 try{
  await assert.rejects(client.live!({query:{action:'usage',roots:[root]},mode:'auto'}),{code:'INDEX_UNSUPPORTED_VERSION'});
  const token=new URLSearchParams(new URL(host.url).hash.slice(1)).get('token')!;
  const browser=createHttpClient({origin:host.origin,token,fetch:(url,init)=>fetch(url,{...init,headers:{...init?.headers,Origin:host.origin}})});
  const inventory=await browser.config!({});const item=inventory.items.find(i=>i.path===instructions)!;assert.ok(item);assert.equal(inventory.coverage.historyStatus,'unavailable');assert.equal(item.measurementStatus,'complete');assert.equal(item.bytes,Buffer.byteLength('Synthetic local instructions. '.repeat(1000)));
  const pending=await browser.optimize!({project});const suggestion=pending.suggestions.find(s=>s.item.path===instructions)!;assert.ok(suggestion);assert.ok(suggestion.findings.some(f=>f.rule==='fileSize'));
  const checks=await browser.optimize!({project,action:'checks',itemId:item.id});assert.equal(checks.checks.find(c=>c.rule==='fileSize')?.outcome,'hit');assert.equal(checks.checks.find(c=>c.rule==='runtimeDuplicateInjection')?.outcome,'unsupported');
  await writeFile(instructions,'Short synthetic instructions.');await browser.optimize!({project,action:'recheck',suggestionId:suggestion.id});
  const history=await browser.optimize!({project,group:'history',suggestionId:suggestion.id});assert.equal(history.suggestions[0].status,'verified');assert.equal(history.followUps[0].status,'unavailable');assert.equal(history.followUps[0].observedRecords,null);assert.equal(history.suggestions[0].decision,null);
  const cli=spawnSync(process.execPath,[path.resolve('dist/wombat.js'),'optimize','history','--root',root,'--project-root',project,'--project',project,'--suggestion',suggestion.id,'--json'],{env:process.env,encoding:'utf8',timeout:10_000});assert.ifError(cli.error);assert.equal(cli.status,2,cli.stderr+cli.stdout);const record=JSON.parse(cli.stdout);assert.equal(record.suggestions[0].status,'verified');assert.equal(record.followUps[0].status,'unavailable');assert.equal(record.followUps[0].observedRecords,null);
  const retained=new DatabaseSync(database);assert.equal(retained.prepare('SELECT count(*) AS n FROM sample').get()!.n,1);assert.equal(retained.prepare('PRAGMA user_version').get()!.user_version,99);
  retained.close();
  // Explicit repair uses the real CLI and publishes only the derived generation.
  const rebuilt=spawnSync(process.execPath,[path.resolve('dist/wombat.js'),'usage','--root',root,'--rebuild','--wait-ms','20000','--json'],{env:{...process.env,WOMBAT_CORE_BIN:binary},encoding:'utf8',timeout:25000});
  assert.ifError(rebuilt.error);assert.ok([0,2].includes(rebuilt.status!),rebuilt.stdout+rebuilt.stderr);
  const restored=JSON.parse(rebuilt.stdout);assert.equal(restored.freshness.status,'current');assert.equal(restored.summary.measurementCount,0);
  const pointer=JSON.parse(await readFile(path.join(data,'live-v2/active-index.json'),'utf8'));
  const current=new DatabaseSync(path.join(data,'live-v2',pointer.directory,'index.sqlite'));assert.equal(current.prepare('PRAGMA user_version').get()!.user_version,9);current.close();
  const original=new DatabaseSync(database,{readOnly:true});assert.equal(original.prepare('PRAGMA user_version').get()!.user_version,99);assert.equal(original.prepare('SELECT value FROM sample').get()!.value,'retained');original.close();
  const reviewed=await browser.optimize!({project,group:'history',suggestionId:suggestion.id});assert.equal(reviewed.suggestions[0].status,'verified');assert.equal(reviewed.decisionRevision,history.decisionRevision);
 }finally{await host.close();service.kill('SIGTERM');await once(service,'exit').catch(()=>{});for(const[k,v]of Object.entries(previous)){if(v===undefined)delete process.env[k];else process.env[k]=v;}await rm(dir,{recursive:true,force:true});}
});
