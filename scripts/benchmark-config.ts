import {spawnProductCli} from './cli-invocation.js';
/** Synthetic configuration collection + CLI benchmark, including core peak RSS. */
import assert from 'node:assert/strict';
import {spawn,spawnSync} from 'node:child_process';
import {createHash} from 'node:crypto';
import {mkdirSync,mkdtempSync,writeFileSync,rmSync,realpathSync} from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import {performance} from 'node:perf_hooks';
import {setTimeout as delay} from 'node:timers/promises';
import {fileSha256,option} from './benchmark-common.js';
assert(process.platform==='darwin','Peak RSS collection currently requires macOS time -l');
const output=path.resolve(option('--output')),binary=path.resolve('dist/wombat-core');
const temp=realpathSync(mkdtempSync(path.join(os.tmpdir(),'wombat-config-bench-'))),source=path.join(temp,'source'),project=path.join(temp,'project');
const rule='hello world',large='a'.repeat(1024*1024),small='---\nname: synthetic\ndescription: Synthetic reference\n---\nhello world\n';
mkdirSync(source,{recursive:true});mkdirSync(project,{recursive:true});writeFileSync(path.join(project,'AGENTS.md'),rule);
for(let n=0;n<32;n++){const dir=path.join(project,'.agents/skills',`s${n}`);mkdirSync(dir,{recursive:true});writeFileSync(path.join(dir,'SKILL.md'),small);}
const largeFile=path.join(source,'AGENTS.md');writeFileSync(largeFile,large);
const env={...process.env,CODEX_HOME:source,WOMBAT_DATA_HOME:path.join(temp,'data'),WOMBAT_AUTO_PRICES:'0'};
const service=spawn('/usr/bin/time',['-l',binary,'--serve-usage'],{env,stdio:['ignore','ignore','pipe']});
let timer='';service.stderr!.on('data',c=>{timer=(timer+c.toString()).slice(-5000);});
const closed=new Promise<void>(resolve=>service.once('close',()=>resolve()));
let pid=service.pid!;
for(let n=0;n<20;n++){const child=Number(spawnSync('pgrep',['-P',String(service.pid)],{encoding:'utf8'}).stdout.trim().split(/\s+/)[0]);if(child){pid=child;break;}await delay(20);}
const sample=()=>{const start=performance.now();const p=spawnProductCli('dist/wombat.js',{method:'config',params:{action:'list',roots:[source],projectRoots:[project],scope:{allTime:true}}},{env,encoding:'utf8',timeout:30_000,maxBuffer:16*1024*1024});assert.equal(p.status,2,p.stderr+p.stdout);return {ms:Math.round((performance.now()-start)*100)/100,data:JSON.parse(p.stdout)};};
try{
 const cold=sample();const a=cold.data.items.find((i:any)=>i.path===largeFile);assert.equal(a.contentTokens,131072,'Official tiktoken 0.13.0 synthetic ordinary vector');assert.equal(cold.data.items.find((i:any)=>i.path===path.join(project,'AGENTS.md')).contentTokens,2);
 for(const item of cold.data.items.filter((i:any)=>i.kind==='skill')){assert.equal(item.bodyTokenEstimate?.tokens,3,'Independent official hello world newline body vector');assert.equal(item.bodyTokenEstimate?.payload,'skillBody');assert.equal(item.bodyTokenEstimate?.contentHash,createHash('sha256').update('hello world\n').digest('hex'));}
 const warm=[];for(let n=0;n<5;n++){const q=sample();warm.push(q.ms);const stable=(items:any[])=>items.map(({observedAt,...item})=>item);assert.deepEqual(stable(q.data.items),stable(cold.data.items),'same corpus yields identical metadata and measurements apart from collection time');}
 writeFileSync(largeFile,large+'\n');const changed=sample();const next=changed.data.items.find((i:any)=>i.path===largeFile);assert.equal(next.contentTokens,null);assert.equal(next.estimateStatus,'resourceLimited');assert.notEqual(next.contentHash,a.contentHash);assert.equal(next.bytes,1048577);
 process.kill(pid,'SIGTERM');await closed;const peak=/\n\s*(\d+)\s+maximum resident set size/.exec(timer);assert(peak,timer);
 const result={platform:`${os.type()} ${os.arch()}`,node:process.version,coreSha256:fileSha256(binary),corpusSha256:createHash('sha256').update(rule).update(large).update(small.repeat(32)).digest('hex'),files:34,method:'tiktoken-rs-0.12.0/o200k_base/ordinary-v1',coldCollectionAndCliMs:cold.ms,warmCollectionAndCliMs:warm,changedCollectionAndCliMs:changed.ms,peakCoreRssBytes:Number(peak[1]),correctness:'1 MiB ASCII run matches independent official 131072-token truth; hello world=2; Skill body hello world newline=3 with exact body hash; all warmed item measurements identical; one-byte limit crossing invalidates the estimate and updates hash/metadata.',conditions:'Release core, empty usage source, new derived inventory; warm calls recollect files and reuse full/body token counts keyed by content/method. Filesystem cache uncontrolled. Peak RSS covers the Rust service including embedded encoding, not browser/Node or large usage ledgers; no claim of scan speedup or production scale.'};
 mkdirSync(path.dirname(output),{recursive:true});writeFileSync(output,JSON.stringify(result,null,2)+'\n');console.log(JSON.stringify(result));
}finally{if(service.exitCode===null){try{process.kill(pid,'SIGTERM');}catch{}await closed;}rmSync(temp,{recursive:true,force:true});}
