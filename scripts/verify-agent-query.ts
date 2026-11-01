import { queryInvocation, spawnProductCli, type CliInput } from './cli-invocation.ts';
/** Installed-runtime qualification over synthetic truth; optional real Codex execution. */
import {spawn,spawnSync,type ChildProcess} from 'node:child_process';
import {cpSync,mkdirSync,mkdtempSync,readFileSync,readdirSync,realpathSync,rmSync,writeFileSync} from 'node:fs';
import path from 'node:path';
import {tmpdir} from 'node:os';
import {fileURLToPath} from 'node:url';
import {once} from 'node:events';
import {setTimeout as delay} from 'node:timers/promises';
import {parseArgs} from 'node:util';
import {checkBuild,sourceIdentity} from './build-identity.ts';
import {assertExternalOutputDir,runBoundedCommand,terminateTree,stdoutFromLog} from './verify-e2e-helpers.ts';
const root=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'..');
const record=(v:unknown):v is Record<string,unknown>=>v!=null&&typeof v==='object'&&!Array.isArray(v);
const integer=(v:unknown):v is number=>typeof v==='number'&&Number.isSafeInteger(v)&&v>=0;
/** Native missing token fields stay null. No reasoning estimates from output length. */
export function agentMetrics(stdout:string){
 const commands:string[]=[],bytes:number[]=[];let unsupportedToolCalls=0;const turns:Record<string,unknown>[]=[];
 for(const line of stdout.trim().split('\n')){
  if(!line)continue;const event:unknown=JSON.parse(line);if(!record(event)||typeof event.type!=='string')throw new Error('Invalid Agent event');
  if(event.type==='turn.completed'){if(!record(event.usage))throw new Error('Missing native Agent usage');turns.push(event.usage);}
  if(event.type==='item.completed'&&record(event.item)&&['mcp_tool_call','web_search','collab_tool_call'].includes(String(event.item.type)))unsupportedToolCalls++;
  if(event.type==='item.completed'&&record(event.item)&&event.item.type==='command_execution'){
   if(typeof event.item.command!=='string'||typeof event.item.aggregated_output!=='string')throw new Error('Invalid command event');commands.push(event.item.command);bytes.push(Buffer.byteLength(event.item.aggregated_output));
  }
 }
 const sum=(key:string)=>{if(!turns.length||turns.some(t=>!integer(t[key])))return null;const value=turns.reduce((n,t)=>n+(t[key] as number),0);if(!integer(value))throw new Error('Unsafe Agent token total');return value;};
 return {commands,unsupportedToolCalls,toolCalls:commands.length,toolResultBytes:bytes.reduce((a,b)=>a+b,0),inputTokens:sum('input_tokens'),cachedInputTokens:sum('cached_input_tokens'),outputTokens:sum('output_tokens'),reasoningTokens:sum('reasoning_tokens')};
}
export {stdoutFromLog} from './verify-e2e-helpers.ts';
function query(cli:string,env:NodeJS.ProcessEnv,args: CliInput){
 const start=performance.now();const run=spawnProductCli(cli, Array.isArray(args) ? [...args, '--json'] : args, {cwd:path.dirname(cli),env,encoding:'utf8',timeout:15_000,maxBuffer:1024*1024});
 if(run.error||![0,2].includes(run.status??-1))throw new Error('Installed query failed: '+(run.error?.message??run.stderr.slice(0,1000)));
 const value:unknown=JSON.parse(run.stdout);if(!record(value)||record(value.error))throw new Error('Invalid installed query result');return {value,bytes:Buffer.byteLength(run.stdout),elapsedMs:Math.round(performance.now()-start)};
}
async function qualify(outputDir:string,agentBin?:string,language:'zh'|'en'='en'){
 checkBuild(root);mkdirSync(outputDir,{recursive:true,mode:0o700});const sandbox=realpathSync.native(mkdtempSync(path.join(tmpdir(),'wombat-agent-query-')));
 const report:Record<string,unknown>={format:1,status:'running',sourceSha256:sourceIdentity(root),platform:process.platform+'-'+process.arch,actualAgent:!!agentBin,language,limits:{timeoutMs:180_000,maxOutputBytes:4*1024*1024},boundaries:['Synthetic source only; no real user logs or credential inspection.','Runtime and Skill assets are staged from the verified build; this is not a system installation check.','The fixed query matrix establishes correctness for this platform, not productivity, savings, scale or an SLO.','Native unavailable Agent token fields stay null.'],startedAt:new Date().toISOString()};
 const save=()=>writeFileSync(path.join(outputDir,'agent-query-report.json'),JSON.stringify(report,null,2)+'\n',{mode:0o600});save();
 let service:ChildProcess|undefined;
 try{
  const source=path.join(sandbox,'source');mkdirSync(path.join(source,'sessions'),{recursive:true});
  const row=(type:string,payload:object,index=0)=>({type,payload,timestamp:new Date(Date.parse('2026-09-02T00:00:00Z')+index*1000).toISOString()});
  const rows=[row('session_meta',{id:'qualification',cwd:'/synthetic/qualification'}),row('event_msg',{type:'thread_title_updated',thread_id:'qualification',title:'Synthetic high usage'}),row('turn_context',{turn_id:'turn',model:'gpt-5.4',effort:'high'}),row('event_msg',{type:'task_started',turn_id:'turn',model_context_window:128000}),row('response_item',{type:'message',role:'developer',content:[{type:'input_text',text:'Synthetic injected context'}]})];
  for(const [index,n] of [200_000,300_000,600_000].entries()){if(index===2)rows.push(row('event_msg',{type:'compacted',turn_id:'turn'},index+1));rows.push(row('event_msg',{type:'token_usage_record',thread_id:'qualification',turn_id:'turn',response_id:'response-'+index,usage:{input_tokens:n,cached_input_tokens:0,cache_write_input_tokens:0,output_tokens:0,reasoning_output_tokens:0,total_tokens:n}},index+1));}
  writeFileSync(path.join(source,'sessions/qualification.jsonl'),rows.map(r=>JSON.stringify(r)).join('\n')+'\n',{mode:0o600});
  const runtime=path.join(sandbox,'runtime'),installed=path.join(sandbox,'.agents/skills/wombat');
  mkdirSync(runtime);mkdirSync(path.dirname(installed),{recursive:true});
  const dist=path.join(root,'dist');
  for(const name of readdirSync(dist).filter(name=>/^wombat(?:-.*)?\.js$/.test(name)))cpSync(path.join(dist,name),path.join(runtime,name));
  for(const name of [process.platform==='win32'?'wombat-core.exe':'wombat-core','skill'])cpSync(path.join(dist,name),path.join(runtime,name),{recursive:true});
  cpSync(path.join(runtime,'skill/plugin/skills/wombat'),installed,{recursive:true});
  const cli=path.join(runtime,'wombat.js');
  const env={...process.env,WOMBAT_DATA_HOME:path.join(sandbox,'data'),WOMBAT_AUTO_PRICES:'0',WOMBAT_CORE_BIN:''};
  service=spawn(path.join(runtime,process.platform==='win32'?'wombat-core.exe':'wombat-core'),['--serve-usage'],{cwd:sandbox,env,detached:process.platform!=='win32',stdio:'ignore'});await once(service,'spawn');await delay(100);
  const refreshed=query(cli,env,{"method":"usage","params":{"query":{"action":"refresh","roots":[source]},"mode":"auto"}});const ref=refreshed.value.snapshotRef;if(!record(ref)||typeof ref.snapshotId!=='string')throw new Error('Missing fixed snapshot');
  const listed=query(cli,env,queryInvocation({"action":"threads","snapshotId":ref.snapshotId as string,"scope":{"allTime":true}}, {mode:"auto"}));if(!Array.isArray(listed.value.items))throw new Error('Missing installed task list');const task=listed.value.items.find(t=>record(t)&&t.upstreamId==='qualification');if(!record(task)||typeof task.id!=='string')throw new Error('Missing full task identity');
  const common=['--snapshot',ref.snapshotId,'--all-time'];
  const samples=[0,1,2].map(()=>query(cli,env,queryInvocation({"action":"investigate","limit":5,"snapshotId":ref.snapshotId as string,"scope":{"allTime":true}}, {mode:"cached"})));
  for(const sample of samples){const i=sample.value.inspection,summary=sample.value.summary;if(!record(i)||i.candidateCount!==1||!record(summary)||!record(summary.tokens)||summary.tokens.total!==1_100_000)throw new Error('Independent synthetic truth failed');}
  const contexts=query(cli,env,queryInvocation({"action":"context","snapshotId":ref.snapshotId as string,"scope":{"allTime":true,"threadId":task.id}}, {mode:"cached"}));const inventory=record(contexts.value.inspection)?contexts.value.inspection.context:null;if(!record(inventory)||inventory.injectedRecords!==1||inventory.modelWindowRecords!==1||!Array.isArray(inventory.records)||inventory.records.some(r=>!record(r)||r.bytes!==null||r.contentVersion!==null))throw new Error('Context physical-record truth failed');
  const trajectory=query(cli,env,queryInvocation({"action":"trajectory","snapshotId":ref.snapshotId as string,"scope":{"allTime":true,"threadId":task.id}}, {mode:"cached"}));const points=record(trajectory.value.inspection)?trajectory.value.inspection.trajectory:null;if(!Array.isArray(points)||points.length!==3||!record(points[2])||points[2].inputDelta!==null||!record(points[2].compactionComparison)||points[2].compactionComparison.inputDifference!==300000)throw new Error('Compaction boundary truth failed');
  const review=query(cli,env,queryInvocation({"action":"review","snapshotId":ref.snapshotId as string,"scope":{"since":"2026-09-02","until":"2026-09-03"}}, {mode:"cached"}));const concentration=record(review.value.inspection)&&record(review.value.inspection.review)?review.value.inspection.review.concentration:null;if(!record(concentration)||concentration.topTaskShare!==1||concentration.totalTokens!==1100000)throw new Error('Full-scope concentration truth failed');
  const api=query(cli,env,['api']);
  if(!Array.isArray(api.value.methods)||!api.value.methods.includes('usage')||api.value.inputSchema!==null)throw new Error('Installed Agent method discovery failed');
  const method=query(cli,env,['api','--method','snapshot']);
  if(!record(method.value.inputSchema)||method.value.selectedMethod!=='snapshot')throw new Error('Installed per-method schema failed');
  const structured=spawnProductCli(cli, ['call'], {cwd:sandbox,env,input:JSON.stringify({method:'snapshot',params:{action:'investigate',snapshotId:ref.snapshotId,scope:{allTime:true},limit:3,compact:true}}),encoding:'utf8',timeout:15000,maxBuffer:2*1024*1024});
  if(structured.error||![0,2].includes(structured.status??-1))throw new Error('Installed structured call failed');
  const structuredResult:unknown=JSON.parse(structured.stdout);
  if(!record(structuredResult)||!record(structuredResult.summary)||!record(structuredResult.summary.tokens)||structuredResult.summary.tokens.total!==1100000||!record(structuredResult.quality)||!record(structuredResult.quality.detailSummary)||structuredResult.facets!==undefined)throw new Error('Compact structured total/quality failed');
  const compact=query(cli,env,['usage',...common,'--compact','--limit','1']);
  const full=query(cli,env,['usage',...common,'--limit','1']);
  if(JSON.stringify(compact.value.summary)!==JSON.stringify(full.value.summary)||JSON.stringify(compact.value.items)!==JSON.stringify(full.value.items))throw new Error('Installed compact conservation failed');
  const turns=query(cli,env,queryInvocation({"action":"turns","matchedOnly":true,"threadId":task.id,"snapshotId":ref.snapshotId as string,"scope":{"allTime":true}}, {mode:"auto"}));
  const turn=Array.isArray(turns.value.items)?turns.value.items.find(t=>record(t)&&t.ordinal!==null):null;
  if(!record(turn)||typeof turn.id!=='string')throw new Error('Missing selected turn');
  const selected=query(cli,env,queryInvocation({action:'investigate',snapshotId:ref.snapshotId as string,scope:{allTime:true,threadId:task.id,turnId:turn.id},compact:true},{mode:'cached'}));
  const selectedInspection=selected.value.inspection;
  if(!record(selectedInspection)||!Array.isArray(selectedInspection.limitations)||!selectedInspection.limitations.includes('selected_turn_only')||!record(selected.value.scope)||selected.value.scope.turnId!==turn.id)throw new Error('Installed exact-turn inspection failed');
  const invalid=spawnProductCli(cli, queryInvocation({"action":"context","snapshotId":"unsupported:synthetic","scope":{"allTime":true}}, {mode:"cached"}), {cwd:sandbox,env,encoding:'utf8',timeout:15000,maxBuffer:1024*1024});if(invalid.error||invalid.status!==1||!record(JSON.parse(invalid.stdout).error))throw new Error('Unknown snapshot failed closed incorrectly');
  report.agentInterface={methods:api.value.methods.length,structured:true,compactBytes:compact.bytes,fullBytes:full.bytes,selectedTurn:true};
  report.queryMatrix=[{case:'context-physical-records',records:2,bytes:contexts.bytes,elapsedMs:contexts.elapsedMs},{case:'compaction-observation',inputDifference:300000,continuousDelta:null,bytes:trajectory.bytes,elapsedMs:trajectory.elapsedMs},{case:'full-scope-concentration',topTaskShare:1,bytes:review.bytes,elapsedMs:review.elapsedMs},{case:'unknown-snapshot',status:'rejected'}];
  report.runtime={staged:true,fixtureRequests:3,refresh:{bytes:refreshed.bytes,elapsedMs:refreshed.elapsedMs},cachedQueries:samples.map(s=>({bytes:s.bytes,elapsedMs:s.elapsedMs})),completeTokens:1_100_000,candidateCount:1};
  if(agentBin){
   const answerFile=path.join(sandbox,'answer.json'),schemaFile=path.join(sandbox,'answer-schema.json'),log=path.join(outputDir,'agent-events.log');
   writeFileSync(schemaFile,JSON.stringify({type:'object',additionalProperties:false,required:['completeTokens','candidateCount','snapshotId','limitations','explanation'],properties:{completeTokens:{type:'integer'},candidateCount:{type:'integer'},snapshotId:{type:'string'},explanation:{type:'string'},limitations:{type:'array',items:{type:'string'}}}}));
   const prompt=`Use $wombat installed at ${installed}. Write the explanation in ${language==='zh'?'Chinese':'English'}; preserve protocol values and IDs. Analyze the fixed synthetic snapshot ${ref.snapshotId} across all dates with investigate. Return its complete token total, total candidate count, snapshot identity, and exact inspection limitations in the required schema. For EVERY installed runtime command explicitly set WOMBAT_DATA_HOME=${env.WOMBAT_DATA_HOME} and WOMBAT_AUTO_PRICES=0 in that command; do not rely on inherited shell environment. Only read this Skill and its references and call the installed runtime ${cli}. Read no source logs, databases, credentials, or other user data. Do not refresh, modify files, call other plugins, or delegate.`;
   const run=await runBoundedCommand({command:[agentBin,'exec','--ephemeral','--config','mcp_servers={}','--sandbox','workspace-write','--skip-git-repo-check','--cd',sandbox,'--json','--output-schema',schemaFile,'--output-last-message',answerFile,prompt],cwd:sandbox,env,logFile:log,timeoutMs:180_000,maxBytes:4*1024*1024});
   report.agentProcess=run;save();if(run.exitCode!==0||run.timedOut||run.outputLimit||run.spawnError||run.interrupted||run.closeTimedOut||run.logError)throw new Error('Actual Agent process did not complete successfully');
   const metrics=agentMetrics(stdoutFromLog(readFileSync(log,'utf8'))),answer:unknown=JSON.parse(readFileSync(answerFile,'utf8'));
   report.agentMetrics=metrics;report.answer=answer;
   if(metrics.unsupportedToolCalls>0||!metrics.commands.some(c=>c.includes(cli))||metrics.commands.some(c=>!c.includes(cli)&&!c.includes(installed)))throw new Error('Agent used an unqualified query surface');
   if(!record(answer)||answer.completeTokens!==1_100_000||answer.candidateCount!==1||answer.snapshotId!==ref.snapshotId||!Array.isArray(answer.limitations)||!answer.limitations.includes('context_occupancy_unavailable'))throw new Error('Actual Agent answer does not match independent truth');
   if(typeof answer.explanation!=='string'||!answer.explanation.trim()||(language==='zh'?!/\p{Script=Han}/u.test(answer.explanation):!/\b(?:tokens?|usage|total)\b/i.test(answer.explanation)))throw new Error('Actual Agent explanation language was not verified');
  }
  checkBuild(root);report.status=agentBin?'passed':'runtime_passed';report.completedAt=new Date().toISOString();save();
 }catch(error){report.status='failed';report.failure=error instanceof Error?error.message:String(error);report.completedAt=new Date().toISOString();save();throw error;}
 finally{if(service&&service.exitCode===null&&service.signalCode===null&&!await terminateTree(service)){report.status='failed';report.failure='Installed service cleanup failed';save();throw new Error('Installed service cleanup failed');}rmSync(sandbox,{recursive:true,force:true,maxRetries:5,retryDelay:100});}
}
if(process.argv[1]&&path.resolve(process.argv[1])===fileURLToPath(import.meta.url)){
 const {values}=parseArgs({args:process.argv.slice(2).filter(a=>a!=='--'),options:{'output-dir':{type:'string'},'agent-bin':{type:'string'},language:{type:'string',default:'en'}}});
 if(!values['output-dir'])throw new Error('verify:agent-query requires --output-dir ABSOLUTE_PATH outside the repository');
 if(values.language!=='zh'&&values.language!=='en')throw new Error('--language must be zh or en');
 await qualify(assertExternalOutputDir(root,values['output-dir']),values['agent-bin'],values.language);
}
