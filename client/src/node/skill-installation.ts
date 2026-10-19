import {constants} from 'node:fs';
import {open,lstat, readdir, mkdir, mkdtemp, cp, rename, rm, writeFile} from 'node:fs/promises';
import path from 'node:path';
import os from 'node:os';
import {fileURLToPath} from 'node:url';
import {createHash} from 'node:crypto';
import {CoreError} from '../errors.js';
import type {QueryOptions} from '../client.js';
import type {Installation} from '../generated/skill-installation.js';
import type {Discovery} from '../generated/skill-discovery.js';
import type {RuntimeCheck} from '../generated/setup-response.js';
import {discoverWombatSkill} from './codex/skills.js';
import type {CodexOptions} from './codex/process.js';

const MARKER='.wombat-install.json';
export const skillCapabilities=['usage-json-v3','usage-json-v5','usage-compact-v1','inspection-json-v5','config-json-v1','account-json-v1','handoff-json-v1','web-context-v1','collection-json-v1','setup-json-v1','agent-api-v1','usage-statistics-v1','monitor-v1','config-content-hashes-v1'];
interface File {path:string;size:number;sha256:string;executable:boolean}
interface Manifest {format:number;name:string;version:string;source:string;requiredCapabilities:string[];files:File[];contentHash:string}
export interface SkillInstallationOptions extends CodexOptions {resourcesPath?:string}
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
      if(!relative&&name===MARKER)continue;
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
function manifest(value:unknown):Manifest{
  const invalid=():never=>{throw new CoreError('SKILL_MANIFEST_INVALID','Invalid Skill manifest');};
  if(!object(value)||value.format!==1||value.name!=='wombat'||typeof value.version!=='string'||!/^[0-9A-Za-z][0-9A-Za-z._-]{0,63}$/.test(value.version)||typeof value.source!=='string'||!/^[0-9a-f]{40}$/.test(value.source)||!Array.isArray(value.files)||value.files.length>500||!Array.isArray(value.requiredCapabilities)||value.requiredCapabilities.length>32||typeof value.contentHash!=='string'||!/^[0-9a-f]{64}$/.test(value.contentHash))return invalid();
  const files:File[]=[];
  for(const file of value.files){
    if(!object(file)||typeof file.path!=='string'||file.path.includes('\\')||file.path.split('/').some(part=>!part||part==='.'||part==='..')||typeof file.size!=='number'||!Number.isSafeInteger(file.size)||file.size<0||file.size>1024*1024||typeof file.sha256!=='string'||!/^[0-9a-f]{64}$/.test(file.sha256)||typeof file.executable!=='boolean')return invalid();
    files.push({path:file.path,size:file.size,sha256:file.sha256,executable:file.executable});
  }
  if(!files.some(file=>file.path==='SKILL.md')||new Set(files.map(file=>file.path)).size!==files.length||value.contentHash!==hash(JSON.stringify(files)))return invalid();
  const requiredCapabilities:string[]=[];
  for(const capability of value.requiredCapabilities){if(typeof capability!=='string')return invalid();if(!skillCapabilities.includes(capability))throw new CoreError('SKILL_RUNTIME_MISMATCH','Required runtime capability unavailable');requiredCapabilities.push(capability);}
  return {format:1,name:'wombat',version:value.version,source:value.source,files,requiredCapabilities,contentHash:value.contentHash};
}
async function resources(configured?:string){
  const directory=path.dirname(fileURLToPath(import.meta.url));
  for(const candidate of configured?[path.resolve(configured)]:[path.join(directory,'skill'),path.resolve(directory,'../../../dist/skill')]){
    if(await exists(path.join(candidate,'manifest.json')))return {directory:candidate,manifest:manifest(await boundedJson(path.join(candidate,'manifest.json')))};
  }
  throw new CoreError('SKILL_RESOURCES_MISSING','Build or install Wombat with Skill resources');
}
/** Current bundled marketplace, without changing Codex registration or installation. */
export async function skillMarketplace():Promise<string>{
  const source=await resources();
  const catalog=await boundedJson(path.join(source.directory,'.agents/plugins/marketplace.json'));
  if(!object(catalog)||catalog.name!=='wombat-local'||!Array.isArray(catalog.plugins)||!catalog.plugins.some(p=>object(p)&&p.name==='wombat-collection'))throw new CoreError('SKILL_MANIFEST_INVALID','Collection marketplace unavailable');
  return source.directory;
}
/** Verify the discovered copy, not just the current bundled marketplace. No host mutations. */
export async function skillRuntimeChecks(discovery:Discovery,query:QueryOptions={}):Promise<RuntimeCheck[]> {
  const enabled=discovery.instances.filter(instance=>instance.enabled);
  if(enabled.length>32)throw new CoreError('SKILL_UNSAFE_RESOURCE','Too many discovered Wombat instances');
  const checks:RuntimeCheck[]=[];
  for(const instance of enabled){
    cancelled(query);
    checks.push(await (async()=>{
    const directory=path.dirname(instance.path),standalone=instance.name==='wombat';
    const file=standalone?path.join(directory,MARKER):path.resolve(directory,'../../wombat-runtime.json');
    const result:RuntimeCheck={path:instance.path,status:'unavailable',pluginVersion:null,skillContentHash:null,requiredCapabilities:[],missingCapabilities:[],errorCode:null};
    try {
      if(!await exists(file))return {...result,status:'unmanaged'};
      const value=await boundedJson(file);
      if(!object(value)||value.format!==1||typeof value.version!=='string'||!/^[0-9A-Za-z][0-9A-Za-z._-]{0,63}$/.test(value.version)||!Array.isArray(value.requiredCapabilities)||value.requiredCapabilities.length>32||value.requiredCapabilities.some(c=>typeof c!=='string'||!c.length||c.length>128)||new Set(value.requiredCapabilities).size!==value.requiredCapabilities.length)throw new CoreError('SKILL_MANIFEST_INVALID','Invalid runtime requirements');
      const expected=standalone?value.contentHash:value.skillContentHash;
      if(typeof expected!=='string'||!/^[0-9a-f]{64}$/.test(expected))throw new CoreError('SKILL_MANIFEST_INVALID','Invalid Skill identity');
      result.pluginVersion=value.version;
      result.skillContentHash=expected;
      result.requiredCapabilities=value.requiredCapabilities;
      result.missingCapabilities=result.requiredCapabilities.filter(c=>!skillCapabilities.includes(c));
      let actual:string;
      if(!standalone&&value.skills!==undefined){
        if(!Array.isArray(value.skills)||!value.skills.length||value.skills.length>32)throw new CoreError('SKILL_MANIFEST_INVALID','Invalid plugin Skill inventory');
        const skills:{name:string;contentHash:string}[]=[];
        for(const skill of value.skills){
          if(!object(skill)||typeof skill.name!=='string'||!/^[a-z0-9]+(?:-[a-z0-9]+)*$/.test(skill.name)||skill.name.length>64||typeof skill.contentHash!=='string'||!/^[0-9a-f]{64}$/.test(skill.contentHash)||skills.some(s=>s.name===skill.name))throw new CoreError('SKILL_MANIFEST_INVALID','Invalid plugin Skill identity');
          if(hash(JSON.stringify(await inventory(path.join(path.dirname(directory),skill.name),query)))!==skill.contentHash)return {...result,status:'modified',errorCode:'SKILL_RESOURCE_CHANGED'};
          skills.push({name:skill.name,contentHash:skill.contentHash});
        }
        if(skills[0].name!=='wombat')throw new CoreError('SKILL_MANIFEST_INVALID','Missing plugin query entry');
        actual=hash(JSON.stringify(skills));
      }else actual=hash(JSON.stringify(await inventory(directory,query)));
      if(actual!==expected)return {...result,status:'modified',errorCode:'SKILL_RESOURCE_CHANGED'};
      return {...result,status:result.missingCapabilities.length?'incompatible':'compatible',errorCode:result.missingCapabilities.length?'SKILL_RUNTIME_MISMATCH':null};
    }catch(error){if(query.signal?.aborted)throw error;return {...result,errorCode:error instanceof CoreError?error.code:'SKILL_UNAVAILABLE'};}
    })());
  }
  return checks;
}
async function owned(directory:string):Promise<{status:Installation['status'];manifest?:Manifest}>{
  if(!await exists(directory))return {status:'absent'};
  const info=await lstat(directory);
  if(!info.isDirectory()||info.isSymbolicLink()||!await exists(path.join(directory,MARKER)))return {status:'unmanaged'};
  let record:Manifest;try{record=manifest(await boundedJson(path.join(directory,MARKER)));}catch{return {status:'unmanaged'};}
  try{if(hash(JSON.stringify(await inventory(directory)))!==record.contentHash)return {status:'modified',manifest:record};}catch{return {status:'modified',manifest:record};}
  return {status:'installed',manifest:record};
}
/** Explicit local installation only; Codex retains plugin ownership. Never scan logs here. */
export async function manageSkill(action:'install'|'status'|'uninstall',input:{directory?:string;replace?:boolean;cwd?:string}={},query:QueryOptions={},options:SkillInstallationOptions={}):Promise<Installation>{
  cancelled(query);const directory=path.resolve(input.directory??path.join(os.homedir(),'.agents/skills/wombat'));
  if(path.basename(directory)!=='wombat')throw new CoreError('INVALID_ARGUMENT','Skill directory must end in wombat');
  const parent=path.dirname(directory),lock=path.join(parent,'.wombat-install-lock');let locked=false,stage:string|undefined,preserveStage=false;
  try{
    if(action!=='status'){await mkdir(parent,{recursive:true});try{await mkdir(lock,{mode:0o700});locked=true;}catch(e){if((e as NodeJS.ErrnoException).code==='EEXIST')throw new CoreError('SKILL_BUSY','Another Skill installation is active');throw e;}}
    let current=await owned(directory);
    if(action==='install'){
      if(current.status!=='absent'&&(current.status!=='installed'||!input.replace))throw new CoreError('SKILL_INSTALL_CONFLICT','Existing Skill is protected; only an unchanged managed installation can be explicitly replaced');
      const source=await resources(options.resourcesPath);
      if(hash(JSON.stringify(await inventory(path.join(source.directory,'wombat'))))!==source.manifest.contentHash)throw new CoreError('SKILL_RESOURCE_CHANGED','Skill resources failed verification');
      stage=await mkdtemp(path.join(parent,'.wombat-stage-'));const next=path.join(stage,'next'),backup=path.join(stage,'previous');
      await cp(path.join(source.directory,'wombat'),next,{recursive:true,errorOnExist:true,force:false});
      const record=JSON.stringify(source.manifest,null,2)+'\n';if(Buffer.byteLength(record)>65536)throw new CoreError('SKILL_UNSAFE_RESOURCE','Installation manifest limit exceeded');
      await writeFile(path.join(next,MARKER),record,{mode:0o600});
      if(hash(JSON.stringify(await inventory(next)))!==source.manifest.contentHash)throw new CoreError('SKILL_RESOURCE_CHANGED','Staged Skill failed verification');
      cancelled(query);
      const verified=await owned(directory);
      if(verified.status!==current.status||JSON.stringify(verified.manifest)!==JSON.stringify(current.manifest))throw new CoreError('SKILL_INSTALL_CONFLICT','Skill directory changed during installation');
      if(current.status==='installed')await rename(directory,backup);
      try{await rename(next,directory);}catch(error){if(await exists(backup)){try{await rename(backup,directory);}catch(restore){preserveStage=true;throw new AggregateError([error,restore],'Skill restore failed; previous installation retained at '+backup);}}throw error;}
      current={status:'installed',manifest:source.manifest};
    }else if(action==='uninstall'){
      if(current.status!=='absent'){
        if(current.status!=='installed')throw new CoreError('SKILL_INSTALL_CONFLICT','Unmanaged or modified Skill is protected');
        cancelled(query);stage=await mkdtemp(path.join(parent,'.wombat-remove-'));await rename(directory,path.join(stage,'removed'));current={status:'absent'};
      }
    }
    cancelled(query);
    const discovery=await discoverWombatSkill(path.resolve(input.cwd??process.cwd()),query,options);
    return {outputVersion:1,action,directory,status:current.status,version:current.manifest?.version??null,source:current.manifest?.source??null,discovery,runtimeCapabilities:[...skillCapabilities],dataStatus:'not_requested'};
  }finally{if(stage&&!preserveStage)await rm(stage,{recursive:true,force:true});if(locked)await rm(lock,{recursive:true,force:true});}
}
