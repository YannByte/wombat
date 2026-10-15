import path from 'node:path';
import {CoreError} from '../errors.js';
import type {QueryOptions} from '../client.js';
import type {Request} from '../generated/setup-request.js';
import type {Response} from '../generated/setup-response.js';
import {validate as configResult} from '../generated/validate-config-response.js';
import {discoverWombatSkill} from './codex/skills.js';
import {nativeVersion,type CodexOptions} from './codex/process.js';
import {queryLive} from './live.js';
import type {CoreProcessOptions} from './core.js';
import {skillCapabilities,skillMarketplace,skillRuntimeChecks} from './skill-installation.js';
import packageMetadata from '../../package.json' with {type:'json'};
/** Read-only checks, bounded independently; failures never hide usable stages. */
export async function checkSetup(r:Request,q:QueryOptions,options:CoreProcessOptions&CodexOptions):Promise<Response> {
  if(r.project!=null&&(!path.isAbsolute(r.project)||r.project.length>4096)||r.roots!=null&&(!r.roots.length||r.roots.length>64)) throw new CoreError('INVALID_ARGUMENT','Invalid setup scope');
  const controller=new AbortController(),timer=setTimeout(()=>controller.abort(),8000);
  const query={...q,signal:AbortSignal.any([q.signal??new AbortController().signal,controller.signal])};
  try {
    const results=await Promise.allSettled([
      nativeVersion(query,options),
      skillMarketplace(),
      r.project?discoverWombatSkill(r.project,query,options):Promise.resolve({status:'unavailable' as const,instances:[],errorCode:'PROJECT_REQUIRED'}),
      r.project?queryLive({config:{action:'list',kind:'hook',cachedUsage:true,roots:r.roots,projectRoots:[r.project],scope:{project:r.project},limit:1}},query,options):Promise.resolve(null),
    ]);
    if(q.signal?.aborted)throw new CoreError('CANCELLED','Cancelled');
    const [version,marketplace,skills,config]=results,errorCodes:string[]=[];
    const failure=(r:PromiseRejectedResult)=> r.reason instanceof CoreError?r.reason.code:'SETUP_UNAVAILABLE';
    if(marketplace.status==='rejected')errorCodes.push(failure(marketplace));if(version.status==='rejected')errorCodes.push(failure(version));if(skills.status==='rejected')errorCodes.push(failure(skills));if(config.status==='rejected')errorCodes.push(failure(config));
    if(config.status==='fulfilled'&&config.value!=null&&!configResult(config.value))errorCodes.push('PROTOCOL_ERROR');
    const discovery=skills.status==='fulfilled'?skills.value:{status:'unavailable' as const,instances:[],errorCode:failure(skills)};
    if(discovery.errorCode)errorCodes.push(discovery.errorCode);
    let runtimeChecks:Response['runtimeChecks']=[];
    try{runtimeChecks=await skillRuntimeChecks(discovery,query);}catch(error){if(q.signal?.aborted)throw new CoreError('CANCELLED','Cancelled');errorCodes.push(error instanceof CoreError?error.code:'SKILL_UNAVAILABLE');}
    for(const check of runtimeChecks)if(check.errorCode)errorCodes.push(check.errorCode);
    return {outputVersion:1,checkedAt:new Date().toISOString(),project:r.project??null,nativeVersion:version.status==='fulfilled'?version.value:null,discovery,hooks:config.status==='fulfilled'&&configResult(config.value)?config.value.hookRegistry:null,runtimeCapabilities:[...skillCapabilities],runtimeVersion:packageMetadata.version,runtimeChecks,marketplacePath:marketplace.status==='fulfilled'?marketplace.value:null,errorCodes:[...new Set(errorCodes)]};
  } finally {clearTimeout(timer);}
}
