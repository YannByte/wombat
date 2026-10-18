/** Native packaging/discovery qualification in a fresh profile; never trusts user Hooks. */
import assert from 'node:assert/strict';
import {spawn, type ChildProcess} from 'node:child_process';
import {mkdirSync,mkdtempSync,readFileSync,realpathSync,rmSync,writeFileSync} from 'node:fs';
import path from 'node:path';
import {tmpdir} from 'node:os';
import {parseArgs} from 'node:util';
import {fileURLToPath} from 'node:url';
import {once} from 'node:events';
import {createNodeClient} from '@wombat/client/node';
import {checkBuild,sourceIdentity} from './build-identity.ts';
import {assertExternalOutputDir,runBoundedCommand,terminateTree} from './verify-e2e-helpers.ts';
import {stdoutFromLog} from './verify-agent-query.ts';
import {installBundledPlugin} from './install-plugin-bootstrap.ts';
const root=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'..');
const {values}=parseArgs({args:process.argv.slice(2).filter(a=>a!=='--'),options:{'output-dir':{type:'string'},'codex-bin':{type:'string'}}});
if(!values['output-dir']||!values['codex-bin']||!path.isAbsolute(values['codex-bin'])) throw new Error('verify:skill-native requires --output-dir ABSOLUTE_EXTERNAL_PATH --codex-bin ABSOLUTE_EXECUTABLE');
checkBuild(root);
const output=assertExternalOutputDir(root,values['output-dir']);mkdirSync(output,{recursive:true,mode:0o700});
const temp=realpathSync.native(mkdtempSync(path.join(tmpdir(),'wombat-native-install-'))),home=path.join(temp,'codex'),project=path.join(temp,'project'),data=path.join(temp,'data');
mkdirSync(path.join(home,'sessions'),{recursive:true});mkdirSync(project);
const env={...process.env,CODEX_HOME:home,WOMBAT_DATA_HOME:data,WOMBAT_AUTO_PRICES:'0',WOMBAT_CORE_BIN:''};
const previous=['CODEX_HOME','WOMBAT_DATA_HOME','WOMBAT_AUTO_PRICES'].map(k=>[k,process.env[k]] as const);
const report:Record<string,unknown>={format:1,status:'running',sourceSha256:sourceIdentity(root),platform:process.platform+'-'+process.arch,startedAt:new Date().toISOString(),boundaries:['Fresh Codex profile; no user credentials, configuration, source logs or trust changes.','Native installation and metadata discovery do not prove Hook execution, model use, or public distribution.']};
const save=()=>writeFileSync(path.join(output,'native-skill-report.json'),JSON.stringify(report,null,2)+'\n',{mode:0o600});save();
let service:ChildProcess|undefined;
try {
 const run=async(name:string,args:string[])=>{const log=path.join(output,name+'.log');const result=await runBoundedCommand({command:[values['codex-bin']!,...args],cwd:project,env,logFile:log,timeoutMs:15000,maxBytes:512*1024});assert.equal(result.exitCode,0,`${name}: native command failed`);assert.equal(result.timedOut,false);assert.equal(result.outputLimit,false);return stdoutFromLog(readFileSync(log,'utf8')).trim();};
 report.nativeVersion=await run('version',['--version']);
 Object.assign(process.env,{CODEX_HOME:home,WOMBAT_DATA_HOME:data,WOMBAT_AUTO_PRICES:'0'});
 const binary=path.join(root,'dist',process.platform==='win32'?'wombat-core.exe':'wombat-core');
 service=spawn(binary,['--serve-usage'],{cwd:project,env,stdio:'ignore',detached:process.platform!=='win32'});await once(service,'spawn');
 const client=createNodeClient({binaryPath:binary,codexBinaryPath:values['codex-bin'],automaticPrices:false});
 const caller=process.cwd();
 try {process.chdir(project);assert.equal(await installBundledPlugin(path.join(root,'dist/skill'),{binary:values['codex-bin'],env}),'wombat');}finally{process.chdir(caller);}
 const base=await client.setup!({project,roots:[home]},{signal:AbortSignal.timeout(10000)});
 assert.ok(base.runtimeChecks);
 assert.equal(base.discovery.status,'available');assert.equal(base.discovery.instances[0]?.name,'wombat:wombat');assert.equal(base.runtimeChecks[0]?.status,'compatible');
 report.installer={stage:'verified',plugin:'wombat',runtimeStatus:base.runtimeChecks[0].status};
 await run('remove-base',['plugin','remove','wombat@wombat-local']);
 await run('install',['plugin','add','wombat-collection@wombat-local','--json']);
 const setup=await client.setup!({project,roots:[home]},{signal:AbortSignal.timeout(10000)});
 assert.equal(setup.discovery.status,'available');assert.equal(setup.discovery.instances.length,1);assert.equal(setup.discovery.instances[0].name,'wombat-collection:wombat');
 assert.ok(setup.hooks);assert.equal(setup.hooks.status,'observed');
 const registrations=setup.hooks.contexts.flatMap(c=>c.registrations).filter(r=>r.pluginId==='wombat-collection@wombat-local');
 assert.equal(registrations.length,10);assert.ok(registrations.every(r=>r.enabled));
 assert.ok(registrations.every(r=>r.trust==='untrusted'),'Fresh collection Hooks must await native human trust');
 assert.equal((await client.collection!({roots:[home]})).received,0);
 report.discovery={status:setup.discovery.status,invocation:'$'+setup.discovery.instances[0].name};report.hooks={status:setup.hooks.status,registrations:registrations.length,trust:'untrusted',received:0};
 await run('remove',['plugin','remove','wombat-collection@wombat-local']);
 const removed=await client.setup!({project,roots:[home]},{signal:AbortSignal.timeout(10000)});assert.equal(removed.discovery.status,'missing');
 assert.equal((await client.collection!({roots:[home]})).received,0);
 report.removal={discovery:'missing',productDataPreserved:true};checkBuild(root);report.status='passed';
} catch(error) {report.status='failed';report.failure=error instanceof Error?error.message:String(error);process.exitCode=1;}
finally {if(service&&service.exitCode===null&&service.signalCode===null&&!await terminateTree(service)){report.status='failed';report.failure='Native qualification service cleanup failed';process.exitCode=1;}
 for(const [k,v] of previous){if(v===undefined)delete process.env[k];else process.env[k]=v;}rmSync(temp,{recursive:true,force:true,maxRetries:5,retryDelay:100});report.completedAt=new Date().toISOString();save();}
console.log(JSON.stringify({status:report.status,report:path.join(output,'native-skill-report.json'),failure:report.failure}));
