import {invokeOperation} from '../core.js';
import {CoreError} from '../../errors.js';
import type {AccountResult,AccountTransport} from '../../client.js';
import type {CoreProcessOptions} from '../core.js';
import {captureAccount,normalizeAccount,unavailableCapture} from './observation.js';
import {connectCodex} from './rpc.js';
import {nativeVersion,type CodexOptions} from './process.js';

/** Account/auth, allowance and activity fail independently; no credential reads. */
export function createAccountTransport(options:CoreProcessOptions & CodexOptions):AccountTransport {
  let cached:AccountResult|undefined,generation=0;
  return async(request,query)=>{
    if(request.action==='history')return invokeOperation('account_history',{...request},query,options);
    const turn=++generation;
    let version:string|undefined,capture=unavailableCapture();
    try{
      version=await nativeVersion(query,options);
      const rpc=await connectCodex(query,options);
      try{capture=await captureAccount(rpc,true,query);}finally{rpc.close();}
    }catch(error){if(query.signal?.aborted)throw new CoreError('CANCELLED','Cancelled');capture=unavailableCapture(error);}
    if(turn!==generation)throw new CoreError('ACCOUNT_CHANGED','A newer account observation is active');
    const result=await normalizeAccount(capture,request.action,version??null,query,options);
    if(query.signal?.aborted)throw new CoreError('CANCELLED','Cancelled');
    if(turn!==generation)throw new CoreError('ACCOUNT_CHANGED','A newer account observation is active');
    if(cached&&result.account.errorCode!=='ACCOUNT_CHANGED'&&((result.identity&&result.identity.id===cached.identity?.id)||result.account.status==='unavailable')) {
      if(result.account.status==='unavailable'&&cached.identity){result.identity=cached.identity;result.account={...cached.account,status:'stale',errorCode:result.account.errorCode};}
      if(result.allowance.status==='unavailable'&&cached.allowance.checkedAt){result.windows=cached.windows.map(w=>({...w,status:'stale'}));result.buckets=cached.buckets.map(b=>({...b,status:'stale',individualLimit:b.individualLimit?{...b.individualLimit,status:'stale'}:null}));result.resetCredits=cached.resetCredits;result.ordinaryUsageAllowed=null;result.modelRestriction=null;result.allowance={...cached.allowance,status:'stale',errorCode:result.allowance.errorCode};}
      if(result.activity.status==='unavailable'&&cached.activity.checkedAt){result.summary=cached.summary;result.activity={...cached.activity,status:'stale',errorCode:result.activity.errorCode};}
    }
    cached=result;
    return result;
  };
}
