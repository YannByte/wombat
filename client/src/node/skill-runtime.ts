import {constants} from 'node:fs';
import {open,lstat,readdir} from 'node:fs/promises';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {createHash} from 'node:crypto';
import {CoreError} from '../errors.js';
import type {QueryOptions} from '../client.js';
import type {Discovery} from '../generated/skill-discovery.js';
import type {RuntimeCheck} from '../generated/setup-response.js';

export const skillCapabilities=['task-cli-v1','project-report-v1','usage-json-v3','usage-json-v5','usage-compact-v1','inspection-json-v5','config-json-v1','account-json-v1','collection-json-v1','setup-json-v1','agent-api-v1','usage-statistics-v1','monitor-v1','config-content-hashes-v1','config-checks-v1'];
interface File {path:string;size:number;sha256:string;executable:boolean}
async function readResource(file:string,limit:number):Promise<Buffer>{
  const handle=await open(file,constants.O_RDONLY|(constants.O_NOFOLLOW??0));
  try{
    if(!(await handle.stat()).isFile())throw new CoreError('SKILL_UNSAFE_RESOURCE','Skill resource is not a regular file');
    const chunks:Buffer[]=[],buffer=Buffer.alloc(Math.min(65536,limit+1));let size=0;
    for(;;){const {bytesRead}=await handle.read(buffer,0,Math.min(buffer.length,limit-size+1),null);if(!bytesRead)break;size+=bytesRead;if(size>limit)throw new CoreError('SKILL_UNSAFE_RESOURCE','Skill resource limit exceeded');chunks.push(Buffer.from(buffer.subarray(0,bytesRead)));}
    return Buffer.concat(chunks,size);
  }finally{await handle.close();}
}
const hash=(value:string|Buffer)=>createHash('sha256').update(value).digest('hex');
const cancelled=(q:QueryOptions)=>{if(q.signal?.aborted)throw new CoreError('CANCELLED','Cancelled');};
async function exists(file:string){try{await lstat(file);return true;}catch(e){if((e as NodeJS.ErrnoException).code==='ENOENT')return false;throw e;}}
async function inventory(root:string,query:QueryOptions={}):Promise<File[]>{
  const files:File[]=[];let entries=0,total=0;
  async function walk(relative=''){
    cancelled(query);
    const directory=path.join(root,relative),stat=await lstat(directory);
    if(stat.isSymbolicLink()||!stat.isDirectory()||relative.split('/').length>16)throw new CoreError('SKILL_UNSAFE_RESOURCE','Invalid Skill directory');
    for(const name of (await readdir(directory)).sort()){
      cancelled(query);
      if(++entries>500)throw new CoreError('SKILL_UNSAFE_RESOURCE','Too many Skill files');
      const file=relative?relative+'/'+name:name,info=await lstat(path.join(root,file));
      if(info.isSymbolicLink())throw new CoreError('SKILL_UNSAFE_RESOURCE','Symbolic links are not managed Skill resources');
      if(info.isDirectory()){await walk(file);continue;}
      if(!info.isFile()||info.size>1024*1024)throw new CoreError('SKILL_UNSAFE_RESOURCE','Skill resource limit exceeded');
      const content=await readResource(path.join(root,file),1024*1024);if((total+=content.length)>8*1024*1024)throw new CoreError('SKILL_UNSAFE_RESOURCE','Skill total limit exceeded');
      files.push({path:file,size:content.length,sha256:hash(content),executable:(info.mode&0o111)!==0});
    }
  }
  await walk();return files;
}
async function boundedJson(file:string):Promise<unknown>{const info=await lstat(file);if(info.isSymbolicLink()||!info.isFile()||info.size>65536)throw new CoreError('SKILL_UNSAFE_RESOURCE','Invalid Skill manifest');return JSON.parse((await readResource(file,65536)).toString('utf8'));}
const object=(value:unknown):value is Record<string,unknown>=>!!value&&typeof value==='object'&&!Array.isArray(value);
/** Current bundled marketplace, without changing Codex registration or installation. */
export async function skillMarketplace():Promise<string>{
  const directory=path.dirname(fileURLToPath(import.meta.url));
  for(const candidate of [path.join(directory,'skill'),path.resolve(directory,'../../../dist/skill')]){
    const file=path.join(candidate,'.agents/plugins/marketplace.json');
    if(!await exists(file))continue;
    const catalog=await boundedJson(file);
    if(!object(catalog)||catalog.name!=='wombat-local'||!Array.isArray(catalog.plugins)||catalog.plugins.length!==1||!object(catalog.plugins[0])||catalog.plugins[0].name!=='wombat')throw new CoreError('SKILL_MANIFEST_INVALID','Invalid Wombat marketplace');
    return candidate;
  }
  throw new CoreError('SKILL_RESOURCES_MISSING','Build or install Wombat with plugin resources');
}
/** Verify the discovered copy, not just the current bundled marketplace. No host mutations. */
export async function skillRuntimeChecks(discovery:Discovery,query:QueryOptions={}):Promise<RuntimeCheck[]> {
  const enabled=discovery.instances.filter(instance=>instance.enabled);
  if(enabled.length>32)throw new CoreError('SKILL_UNSAFE_RESOURCE','Too many discovered Wombat instances');
  const checks:RuntimeCheck[]=[];
  for(const instance of enabled){
    cancelled(query);
    checks.push(await (async()=>{
    const directory=path.dirname(instance.path);
    const file=path.resolve(directory,'../../wombat-runtime.json');
    const result:RuntimeCheck={path:instance.path,status:'unavailable',pluginVersion:null,skillContentHash:null,requiredCapabilities:[],missingCapabilities:[],errorCode:null};
    try {
      if(!await exists(file))return {...result,status:'unmanaged'};
      const value=await boundedJson(file);
      if(!object(value)||value.format!==1||typeof value.version!=='string'||!/^[0-9A-Za-z][0-9A-Za-z._-]{0,63}$/.test(value.version)||!Array.isArray(value.requiredCapabilities)||value.requiredCapabilities.length>32||value.requiredCapabilities.some(c=>typeof c!=='string'||!c.length||c.length>128)||new Set(value.requiredCapabilities).size!==value.requiredCapabilities.length)throw new CoreError('SKILL_MANIFEST_INVALID','Invalid runtime requirements');
      const expected=value.skillContentHash;
      if(typeof expected!=='string'||!/^[0-9a-f]{64}$/.test(expected))throw new CoreError('SKILL_MANIFEST_INVALID','Invalid Skill identity');
      result.pluginVersion=value.version;
      result.skillContentHash=expected;
      result.requiredCapabilities=value.requiredCapabilities;
      result.missingCapabilities=result.requiredCapabilities.filter(c=>!skillCapabilities.includes(c));
      let actual:string;
      {
        if(!Array.isArray(value.skills)||value.skills.length!==1)throw new CoreError('SKILL_MANIFEST_INVALID','Invalid plugin Skill inventory');
        const skills:{name:string;contentHash:string}[]=[];
        for(const skill of value.skills){
          if(!object(skill)||skill.name!=='wombat'||typeof skill.contentHash!=='string'||!/^[0-9a-f]{64}$/.test(skill.contentHash)||skills.some(s=>s.name===skill.name))throw new CoreError('SKILL_MANIFEST_INVALID','Invalid plugin Skill identity');
          if(hash(JSON.stringify(await inventory(path.join(path.dirname(directory),skill.name),query)))!==skill.contentHash)return {...result,status:'modified',errorCode:'SKILL_RESOURCE_CHANGED'};
          skills.push({name:skill.name,contentHash:skill.contentHash});
        }
        if(skills[0].name!=='wombat')throw new CoreError('SKILL_MANIFEST_INVALID','Missing plugin query entry');
        actual=hash(JSON.stringify(skills));
      }
      if(actual!==expected)return {...result,status:'modified',errorCode:'SKILL_RESOURCE_CHANGED'};
      return {...result,status:result.missingCapabilities.length?'incompatible':'compatible',errorCode:result.missingCapabilities.length?'SKILL_RUNTIME_MISMATCH':null};
    }catch(error){if(query.signal?.aborted)throw error;return {...result,errorCode:error instanceof CoreError?error.code:'SKILL_UNAVAILABLE'};}
    })());
  }
  return checks;
}
