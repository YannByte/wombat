import {constants,lstatSync,openSync,readSync,closeSync,fstatSync} from 'node:fs';
import {CoreError} from '@wombat/client';
import {t} from '@wombat/client/locale';
const MAX_BYTES=2*1024*1024;
/** Reports are untrusted data; generated request validation checks the selected baseline. */
export function readBaseline(file:string):unknown {
  try {
    const meta=lstatSync(file);
    if(!meta.isFile()||meta.isSymbolicLink()||meta.size>MAX_BYTES)throw new CoreError('INVALID_ARGUMENT',t('cli.baseline.invalid'));
    const descriptor=openSync(file,constants.O_RDONLY|(constants.O_NOFOLLOW??0)|(constants.O_NONBLOCK??0));
    try {
      const actual=fstatSync(descriptor);
      if(!actual.isFile()||actual.dev!==meta.dev||actual.ino!==meta.ino||actual.size>MAX_BYTES)throw new CoreError('INVALID_ARGUMENT',t('cli.baseline.changed'));
      const bytes=Buffer.allocUnsafe(MAX_BYTES+1);
      let used=0;
      while(used<bytes.length){const count=readSync(descriptor,bytes,used,bytes.length-used,null);if(count===0)break;used+=count;}
      if(used>MAX_BYTES)throw new CoreError('INPUT_LIMIT',t('cli.baseline.invalid'));
      try{return JSON.parse(new TextDecoder('utf-8',{fatal:true}).decode(bytes.subarray(0,used))) as unknown;}
      catch{throw new CoreError('INVALID_ARGUMENT',t('cli.baseline.json'));}
    }finally{closeSync(descriptor);}
  }catch(error){
    if(error instanceof CoreError)throw error;
    const code=error!=null&&typeof error==='object'&&'code' in error?error.code:undefined;
    throw new CoreError(code==='ENOENT'?'ENOENT':code==='EACCES'?'EACCES':'BASELINE_UNAVAILABLE',t('cli.baseline.unavailable'));
  }
}
