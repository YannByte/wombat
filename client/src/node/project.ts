import path from 'node:path';
import {stat} from 'node:fs/promises';
import {CoreError} from '../errors.js';
import type {ProjectRequest,ProjectResult,QueryOptions,UsageClient} from '../client.js';

/** Compose existing facts without recomputing measurements or persisting a report lifecycle. */
export async function inspectProject(client:UsageClient, request:ProjectRequest, options:QueryOptions):Promise<ProjectResult> {
  const validDate=(s:string)=>/^\d{4}-\d{2}-\d{2}$/.test(s)&&Number.isFinite(Date.parse(s))&&new Date(s).toISOString().slice(0,10)===s;
  if (!path.isAbsolute(request.projectRoot) || !validDate(request.since) || !validDate(request.until) || request.since>=request.until) throw new CoreError('INVALID_ARGUMENT','Invalid project scope');
  try {new Intl.DateTimeFormat('en',{timeZone:request.timezone});} catch {throw new CoreError('INVALID_ARGUMENT','Invalid project timezone');}
  const directory=await stat(request.projectRoot).catch(()=>undefined);
  if (!directory?.isDirectory()) throw new CoreError('INVALID_ARGUMENT','Project directory required');
  const scope={project:request.projectRoot,since:request.since,until:request.until,timezone:request.timezone,sourceInstanceId:request.sourceInstanceId};
  const roots=request.roots??[];
  async function section<T>(read:()=>Promise<T>, partial:(r:T)=>boolean):Promise<{state:'complete'|'partial'|'unavailable';result:T|null;error:{code:string;message:string}|null}> {
    if(options.signal?.aborted)throw new CoreError('CANCELLED','Cancelled');
    try {
      const result=await read();
      return {state:partial(result)?'partial':'complete',result,error:null};
    }catch(error){
      if(options.signal?.aborted||error instanceof CoreError&&error.code==='CANCELLED')throw error;
      return {state:'unavailable',result:null,error:{code:error instanceof CoreError?error.code:'INTERNAL_ERROR',message:error instanceof Error?error.message:String(error)}};
    }
  }
  const configurationPromise=section(()=>client.config!({action:'list',review:true,roots,projectRoots:[request.projectRoot],scope,limit:10},options),r=>r.coverage.status==='partial');
  const historyPromise=section(()=>client.live!({query:{action:'usage',roots,scope,limit:1,compact:true},mode:'auto'},options),r=>r.result.quality.status==='partial'||r.freshness.initialScan||!['current','fixed'].includes(r.freshness.status));
  const [configuration,checks,history,artifacts,records]=await Promise.all([
    configurationPromise,
    section(async()=>{
      const config=(await configurationPromise).result;
      return client.optimize!({action:'list',roots:config?.authorizedSourceRoots??roots,readView:config?.readView,projectRoots:[request.projectRoot],project:request.projectRoot,sourceInstanceId:request.sourceInstanceId,ruleOverrides:request.ruleOverrides,limit:20},options);
    },r=>r.resultStatus==='partial'),
    historyPromise,
    section(()=>client.artifacts!({action:'inspect',projectRoot:request.projectRoot,paths:[],keepPaths:[],productTemporary:false,codex:false},options),r=>r.partial),
    section(async()=>{const live=(await historyPromise).result;if(!live)throw new CoreError('INDEX_UNAVAILABLE','History unavailable');return client.conversation!({action:'records',snapshotId:live.result.snapshotRef.snapshotId,roots,selection:{projects:[request.projectRoot],sources:request.sourceInstanceId?[request.sourceInstanceId]:[],since:request.since,until:request.until,timezone:request.timezone},limit:10},options);},r=>r.partial),
  ]);
  const sections=[configuration,checks,history,artifacts,records];
  if(sections.every(s=>s.state==='unavailable'))throw new CoreError(configuration.error!.code,configuration.error!.message);
  const result:ProjectResult={outputVersion:1,scope:{...request,roots},partial:sections.some(s=>s.state!=='complete'),configuration,checks,history,artifacts,records};
  if(Buffer.byteLength(JSON.stringify(result))>2*1024*1024)throw new CoreError('OUTPUT_LIMIT','Project evidence exceeds the response budget; inspect selected components');
  return result;
}
