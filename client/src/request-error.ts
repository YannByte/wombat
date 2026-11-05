import {CoreError} from './errors.js';
type ValidationIssue={instancePath:string;schemaPath:string;keyword:string;params:Record<string,unknown>;message?:string};
/** Schema paths and fixed reasons only; never echo values from an invalid request. */
export function requestError(method:'timing'|'conversation'|'agent',errors:readonly ValidationIssue[]|null):CoreError {
 const issues=(errors??[]).filter(e=>!['oneOf','anyOf','const'].includes(e.keyword));
 const specific=issues.filter(e=>e.schemaPath.includes('/properties/')&&e.keyword!=='additionalProperties');
 const chosen=[...(specific.length?specific:issues)].sort((a,b)=>b.instancePath.split('/').length-a.instancePath.split('/').length);
 const fields=chosen.slice(0,4).map(e=>{
  const property=e.keyword==='required'?e.params.missingProperty:e.keyword==='additionalProperties'?e.params.additionalProperty:undefined;
  const path=(e.instancePath+(typeof property==='string'?'/'+property.replaceAll('~','~0').replaceAll('/','~1'):'')||'/').replace(/[\x00-\x1f\x7f]/g,'').slice(0,256);
  return {path,reason:(e.message??e.keyword).slice(0,160)};
 });
 return new CoreError('INVALID_ARGUMENT',`Invalid ${method} request${fields[0]?' at '+fields[0].path+': '+fields[0].reason:''}`,{fields,schemaCommand:`wombat system api${method==='agent'?'':' '+method} --json`});
}
