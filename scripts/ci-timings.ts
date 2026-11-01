/** Read-only Actions evidence. Unknown historical phase boundaries remain explicit. */
import {spawn} from 'node:child_process';
import {existsSync,mkdirSync,readFileSync,writeFileSync} from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {parseArgs} from 'node:util';
import {repositorySlug} from './release-policy.ts';

interface Step {name:string;started_at:string|null;completed_at:string|null;conclusion:string|null}
interface Job {id:number;name:string;created_at:string|null;started_at:string|null;completed_at:string|null;conclusion:string|null;steps:Step[];html_url:string}
interface Run {id:number;name:string;created_at:string;updated_at:string;conclusion:string|null;head_sha:string;html_url:string}
type Phase='preparation'|'verification'|'unknown';
const seconds=(from:string|null,to:string|null)=>{const a=Date.parse(from??''),b=Date.parse(to??'');return Number.isFinite(a)&&Number.isFinite(b)&&b>=a?(b-a)/1000:null;};
export function stepPhase(name:string):Phase {
  if(/^Run \.\/\.github\/actions\/(?:node|rust)$/.test(name))return 'preparation';
  if(/^(?:Post |Set up job|Complete job|Run actions\/|Run pnpm\/|Run Swatinem\/|Run taiki-e\/)|Prepare|Install locked|Isolate|Inventory native|Export |Retain |Build release|Compile Rust|Assemble.*archives|Setup/i.test(name))return 'preparation';
  if(/^Run npm|^Release gate$|Check platform assumptions without built artifacts/.test(name))return 'unknown';
  if(/test|check|verif|audit|format|lint|Require every|Exercise/i.test(name))return 'verification';
  return 'unknown';
}
export function phaseMarkers(log:string):{at:number;phase:Phase}[] {
  const points:{at:number;phase:Phase}[]=[];
  for(const line of log.split('\n')){
    const m=line.match(/\b(\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?Z)\s+(.*)$/);if(!m)continue;
    const text=m[2];let phase:Phase|undefined;
    const stage=text.match(/^=== (.+) ===$/);
    if(stage)phase=stage[1]==='Build'?'preparation':stage[1]==='Generated contracts'?'preparation':'verification';
    const script=text.match(/^> wombat@[\w.+-]+ ([\w:-]+) /)?.[1];
    if(script){if(script==='build'||script==='test:core'||script==='contracts:check'||script==='github:pack')phase='preparation';else if(/^(?:test(?::|$)|repo:check|typecheck|licenses:check)/.test(script))phase='verification';}
    if(/^Release archive verification: /.test(text))phase='verification';
    if(/^\s*Installing cargo-audit v/.test(text))phase='preparation';
    if(/^\s*Finished `(test|dev|release)` profile/.test(text))phase='verification';
    if(/^running \d+ tests?/.test(text))phase='verification';
    if(phase)points.push({at:Date.parse(m[1]),phase});
  }
  return points.sort((a,b)=>a.at-b.at);
}
export function jobTiming(run:Run,job:Job,log='') {
  const elapsed=seconds(job.started_at,job.completed_at);
  const totals={preparation:0,verification:0,unknown:0};
  const markers=phaseMarkers(log);
  let recorded=0;
  for(const step of job.steps??[]){
    if(step.conclusion==='skipped')continue;
    const length=seconds(step.started_at,step.completed_at);if(length===null)continue;
    recorded+=length;const start=Date.parse(step.started_at!),end=Date.parse(step.completed_at!);
    const local=markers.filter(p=>p.at>=start&&p.at<end);
    let phase=stepPhase(step.name),at=start;
    for(const next of [...local,{at:end,phase}]){totals[phase]+=(next.at-at)/1000;phase=next.phase;at=next.at;}
  }
  if(elapsed!==null)totals.preparation+=Math.max(0,elapsed-recorded);
  return {runId:run.id,workflow:run.name,jobId:job.id,job:job.name,conclusion:job.conclusion,url:job.html_url,
    dependencyWaitSeconds:seconds(run.created_at,job.created_at),queueSeconds:seconds(job.created_at,job.started_at),
    executionSeconds:elapsed,preparationSeconds:elapsed===null?null:totals.preparation,
    verificationSeconds:elapsed===null?null:totals.verification,unclassifiedSeconds:elapsed===null?null:totals.unknown,
    boundaryEvidence:log?'step timestamps and command log markers':'step timestamps only'};
}
async function gh(args:string[],maxBytes=32*1024*1024):Promise<string>{
  return new Promise((resolve,reject)=>{
    const p=spawn('gh',args,{stdio:['ignore','pipe','pipe'],env:{...process.env,GH_PROMPT_DISABLED:'1'}});let text='',error='',bytes=0,problem:Error|undefined;
    const stop=(message:string)=>{problem=new Error(message);p.kill('SIGTERM');};
    const timer=setTimeout(()=>stop('GitHub request timed out'),45000),force=setTimeout(()=>p.kill('SIGKILL'),50000);
    p.stdout.on('data',c=>{bytes+=c.length;if(bytes>maxBytes)stop('GitHub response exceeds budget');else text+=c.toString();});p.stderr.on('data',c=>{error=(error+c.toString()).slice(-4096);});
    p.on('error',e=>{problem=e;});p.on('close',code=>{clearTimeout(timer);clearTimeout(force);if(problem||code!==0)reject(problem??new Error(error||`gh exited ${code}`));else resolve(text);});
  });
}
const median=(a:number[])=>{const values=[...a].sort((x,y)=>x-y);return values.length?values[Math.floor(values.length/2)]:null;};
async function main(){
  const {values}=parseArgs({args:process.argv.slice(2).filter(a=>a!=='--'),options:{'output-dir':{type:'string'},label:{type:'string',default:'before'},limit:{type:'string',default:'50'},run:{type:'string'},reuse:{type:'boolean'},logs:{type:'boolean'}}});
  if(!values['output-dir']||!/^\w[\w-]{0,40}$/.test(values.label!)||!/^\d+$/.test(values.limit!)||Number(values.limit)<1||Number(values.limit)>100||values.run&&!/^\d+$/.test(values.run))throw new Error('Require external --output-dir, valid label, limit 1..100 and optional run ID');
  const out=path.resolve(values['output-dir']),root=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'..');if(out===root||out.startsWith(root+path.sep))throw new Error('Write CI evidence outside the repository');mkdirSync(out,{recursive:true});
  const metadata=JSON.parse(readFileSync(path.join(root,'package.json'),'utf8'));
  const repository=repositorySlug(typeof metadata.repository==='string'?metadata.repository:metadata.repository.url);
  const load=async(file:string,args:string[],bound?:number)=>{const p=path.join(out,values.label+'-'+file);if(values.reuse&&existsSync(p))return readFileSync(p,'utf8');const text=await gh(args,bound);writeFileSync(p,text);return text;};
  const source=values.run?{workflow_runs:[JSON.parse(await load('run.json',['api',`repos/${repository}/actions/runs/${values.run}`]))]}:JSON.parse(await load('runs.json',['api',`repos/${repository}/actions/runs?per_page=${values.limit}`]));
  const runs=source.workflow_runs as Run[];if(!Array.isArray(runs)||runs.length>100)throw new Error('Invalid run listing');
  const rows:ReturnType<typeof jobTiming>[]=[],errors:{run:number;stage:'logs'|'jobs';message:string}[]=[];let next=0;
  await Promise.all(Array.from({length:4},async()=>{while(next<runs.length){const run=runs[next++];try{
    const listing=JSON.parse(await load(`jobs-${run.id}.json`,['api',`repos/${repository}/actions/runs/${run.id}/jobs?per_page=100&filter=latest`]));if(!Array.isArray(listing.jobs)||listing.total_count!==listing.jobs.length)throw new Error('Job listing is incomplete');
    let log='';if(values.logs)try{log=await load(`run-${run.id}.log`,['run','view',String(run.id),'--log','--repo',repository]);}catch(error){errors.push({run:run.id,stage:'logs',message:String(error)});}
    for(const job of listing.jobs as Job[]){const ownLog=log.split('\n').filter(l=>l.startsWith(job.name+'\t')).join('\n');const saved=path.join(out,`baseline-job-${job.id}.log`);rows.push(jobTiming(run,job,ownLog||(existsSync(saved)?readFileSync(saved,'utf8'):'')));}
  }catch(error){errors.push({run:run.id,stage:'jobs',message:String(error)});}}}));
  rows.sort((a,b)=>a.runId-b.runId||a.jobId-b.jobId);
  const groups=Object.entries(Object.groupBy(rows,r=>r.job)).map(([job,items])=>{const complete=items!.filter(i=>i.conclusion==='success'&&i.executionSeconds!==null);return {job,samples:items!.length,successful:complete.length,queueMedian:median(complete.flatMap(i=>i.queueSeconds===null?[]:[i.queueSeconds])),prepareMedian:median(complete.map(i=>i.preparationSeconds!)),verifyMedian:median(complete.map(i=>i.verificationSeconds!)),unclassifiedMedian:median(complete.map(i=>i.unclassifiedSeconds!)),executionMedian:median(complete.map(i=>i.executionSeconds!))};});
  const report={format:1,runs:runs.map(r=>({id:r.id,workflow:r.name,sha:r.head_sha,conclusion:r.conclusion,url:r.html_url,elapsedSeconds:seconds(r.created_at,r.updated_at)})),notes:['Queue is job creation to runner start; dependency waiting is reported separately.','Preparation includes checkout, dependencies, compilation, transfer and teardown.','Historical combined steps without observable boundaries remain unclassified. Cancelled and failed runs are retained but excluded from successful medians.'],jobs:rows,summary:groups,errors};
  writeFileSync(path.join(out,values.label+'-timings.json'),JSON.stringify(report,null,2)+'\n');console.log(JSON.stringify({runs:runs.length,jobs:rows.length,summary:groups,errors},null,2));if(errors.some(e=>e.stage==='jobs'))process.exitCode=1;
}
if(process.argv[1]&&path.resolve(process.argv[1])===fileURLToPath(import.meta.url))main().catch(e=>{console.error(e);process.exitCode=1;});
