import {spawnSync} from 'node:child_process';
import {appendFileSync,readFileSync} from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';

/** Product instructions, generated contracts and license inventories are executable delivery inputs. */
export function documentationOnly(files:readonly string[]):boolean {
  return files.length>0&&files.every(file=>{
    if(/(?:^|\/)AGENTS\.md$/.test(file)||/^(?:plugin|\.agents|docs\/schemas)\//.test(file)||file==='docs/dependency-licenses.json')return false;
    return /^docs\//.test(file)||/(?:^|\/)README(?:\.[A-Za-z-]+)?\.md$/.test(file)||/^\.github\/(?:CONTRIBUTING|SECURITY)(?:\.zh-CN)?\.md$/.test(file)||/(?:^|\/)README\.i18n\.json$/.test(file);
  });
}
export function gateResults(heavy:boolean,jobs:Record<string,{result:string}>):boolean {
  const values=Object.values(jobs);
  return values.length>0&&values.every(j=>heavy?j.result==='success':j.result==='skipped');
}
export function heavyChecks(event:string|undefined,ref:string|undefined,files:readonly string[]):boolean {
  return event!=='pull_request'||ref==='refs/heads/main'||!documentationOnly(files);
}
function changedFiles():string[] {
  const event=JSON.parse(readFileSync(process.env.GITHUB_EVENT_PATH!,'utf8'));
  const base=process.env.GITHUB_EVENT_NAME==='pull_request'?event.pull_request?.base?.sha:event.before;
  const head=process.env.GITHUB_EVENT_NAME==='pull_request'?event.pull_request?.head?.sha:process.env.GITHUB_SHA;
  if(typeof base!=='string'||!/^([a-f0-9]{40}|[a-f0-9]{64})$/.test(base)||/^0+$/.test(base)||typeof head!=='string'||!/^([a-f0-9]{40}|[a-f0-9]{64})$/.test(head))return [];
  const r=spawnSync('git',['diff','--name-only','-z',...(process.env.GITHUB_EVENT_NAME==='pull_request'?[`${base}...${head}`]:[base,head])],{encoding:'utf8',timeout:10000,maxBuffer:16*1024*1024});
  if(r.error||r.status!==0)return []; // Unknown or oversized comparisons run the complete gate.
  return r.stdout.split('\0').filter(Boolean);
}
if(process.argv[1]&&path.resolve(process.argv[1])===fileURLToPath(import.meta.url)){
  if(process.argv[2]==='gate'){
    const needs=JSON.parse(process.env.CI_NEEDS??'{}') as Record<string,{result:string}>;
    const {changes,...jobs}=needs;
    if(changes?.result!=='success'||!gateResults(process.env.CI_HEAVY==='true',jobs))throw new Error('Required CI jobs failed, were cancelled, or were unexpectedly skipped');
    console.log('All required checks completed, or this is a confirmed documentation-only change.');
  }else{
    const heavy=heavyChecks(process.env.GITHUB_EVENT_NAME,process.env.GITHUB_REF,changedFiles());
    appendFileSync(process.env.GITHUB_OUTPUT!,`heavy=${heavy}\n`);
    console.log(heavy?'Run the complete CI matrix.':'Documentation-only change; heavy checks skipped.');
  }
}
