import path from 'node:path';
import type {Discovery, Instance} from '../../generated/skill-discovery.js';
import type {QueryOptions} from '../../client.js';
import {CoreError} from '../../errors.js';
import {connectCodex} from './rpc.js';
import type {CodexOptions} from './process.js';
export type {Discovery, Instance};
const object=(v:unknown):v is Record<string,unknown>=>!!v&&typeof v==='object'&&!Array.isArray(v);

export async function discoverWombatSkill(cwd:string,query:QueryOptions={},options:CodexOptions={}):Promise<Discovery>{
  try{
    const rpc=await connectCodex(query,options);
    try{
      const response=await rpc.request('skills/list',{cwds:[cwd],forceReload:true});
      if(!object(response)||!Array.isArray(response.data))throw new CoreError('NATIVE_PROTOCOL_ERROR','Invalid Skill discovery');
      const entries=response.data.filter(e=>object(e)&&e.cwd===cwd);
      if(entries.length!==1||!object(entries[0])||!Array.isArray(entries[0].skills)||!Array.isArray(entries[0].errors))throw new CoreError('NATIVE_PROTOCOL_ERROR','Invalid Skill discovery');
      const instances:Instance[]=[];
      for(const raw of entries[0].skills){
        if(!object(raw)||typeof raw.name!=='string')throw new CoreError('NATIVE_PROTOCOL_ERROR','Invalid Skill metadata');
        if(raw.name!=='wombat:wombat')continue;
        if(typeof raw.path!=='string'||!path.isAbsolute(raw.path)||typeof raw.enabled!=='boolean')throw new CoreError('NATIVE_PROTOCOL_ERROR','Invalid Wombat Skill');
        const existing=instances.find(s=>s.path===raw.path);
        if(existing&&(existing.name!==raw.name||existing.enabled!==raw.enabled))throw new CoreError('NATIVE_PROTOCOL_ERROR','Conflicting Wombat Skill metadata');
        if(!existing)instances.push({name:raw.name,path:raw.path,enabled:raw.enabled});
      }
      const enabled=instances.filter(s=>s.enabled);
      // A parse failure may hide a competing instance. Never silently choose through errors.
      const parseErrorPaths=entries[0].errors.slice(0,32).flatMap(e=>object(e)&&typeof e.path==='string'&&e.path.length<=4096&&!/[\x00-\x1f\x7f]/.test(e.path)?[e.path]:[]);
      return {status:entries[0].errors.length?'unavailable':enabled.length>1?'ambiguous':enabled.length===1?'available':instances.length?'disabled':'missing',instances,errorCode:entries[0].errors.length?'SKILL_PARSE_ERROR':null,parseErrorPaths};
    }finally{rpc.close();}
  }catch(error){
    if(error instanceof CoreError&&error.code==='CANCELLED')throw error;
    return {status:'unavailable',instances:[],errorCode:error instanceof CoreError?error.code:'CODEX_UNAVAILABLE'};
  }
}
