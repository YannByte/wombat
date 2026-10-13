import {spawn, type ChildProcessWithoutNullStreams} from 'node:child_process';
import {CoreError} from '../../errors.js';
import type {QueryOptions} from '../../client.js';
import {NativeDiagnostics} from '../local-access.js';

export interface CodexOptions { codexBinaryPath?: string; codexTimeoutMs?: number; codexHome?: string }
export function spawnCodex(args: string[], options: CodexOptions): ChildProcessWithoutNullStreams {
  const binary=options.codexBinaryPath ?? process.env.WOMBAT_CODEX_BIN ?? 'codex';
  const script=/\.[cm]?[jt]s$/.test(binary);
  return spawn(script?process.execPath:binary, script?[binary,...args]:args, {
    env:options.codexHome?{...process.env,CODEX_HOME:options.codexHome}:process.env,
    stdio:['pipe','pipe','pipe'], windowsHide:true, detached:process.platform!=='win32',
  });
}
export function stopCodex(child: ChildProcessWithoutNullStreams): void {
  if(child.exitCode!==null||child.signalCode!==null)return;
  try { if(process.platform!=='win32'&&child.pid)process.kill(-child.pid,'SIGTERM');else child.kill(); } catch { }
  const kill=setTimeout(()=>{try{if(process.platform!=='win32'&&child.pid)process.kill(-child.pid,'SIGKILL');else child.kill('SIGKILL');}catch{}},1000);
  kill.unref();child.once('close',()=>clearTimeout(kill));
}
export async function runCodex(args: string[], query: QueryOptions, config: CodexOptions): Promise<string> {
  if(query.signal?.aborted)throw new CoreError('CANCELLED','Cancelled');
  return new Promise((resolve,reject)=>{
    const child=spawnCodex(args,config);const diagnostics=new NativeDiagnostics();let output='',size=0,failure:Error|undefined,done=false;
    const abort=()=>{failure=new CoreError('CANCELLED','Cancelled');stopCodex(child);};
    const stop=()=>{stopCodex(child);};
    const timer=setTimeout(()=>{failure=new CoreError('CODEX_TIMEOUT','Codex request timed out');stopCodex(child);},config.codexTimeoutMs??20_000);
    query.signal?.addEventListener('abort',abort,{once:true});
    process.once('SIGINT',abort);process.once('SIGTERM',abort);process.once('exit',stop);
    const finish=(error?:Error)=>{if(done)return;done=true;clearTimeout(timer);query.signal?.removeEventListener('abort',abort);process.off('SIGINT',abort);process.off('SIGTERM',abort);process.off('exit',stop);error?reject(error):resolve(output);};
    child.stdout.on('data',(chunk:Buffer)=>{size+=chunk.length;if(size>1024*1024){failure=new CoreError('OUTPUT_LIMIT','Codex response too large');stopCodex(child);}else output+=chunk.toString('utf8');});
    child.stderr.on('data',(chunk:Buffer)=>{size+=chunk.length;if(size>1024*1024){failure=new CoreError('OUTPUT_LIMIT','Codex response too large');stopCodex(child);}else diagnostics.write(chunk);});
    child.on('error',error=>{diagnostics.observe(error);finish(diagnostics.error('CODEX_UNAVAILABLE','Codex unavailable'));});
    child.on('close',code=>finish(failure??(code===0?undefined:diagnostics.error('CODEX_UNAVAILABLE','Codex command failed'))));
    child.stdin.end();
    if(query.signal?.aborted)abort();
  });
}
export async function nativeVersion(query: QueryOptions, config: CodexOptions): Promise<string> {
  const text=(await runCodex(['--version'],query,config)).trim();
  // Preserve the complete host identity; recognizing a version grants no capabilities.
  const version=text.length<=256?text.match(/^codex-cli (\d+\.\d+\.\d+(?:-[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?(?:\+[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?)$/)?.[1]:undefined;
  if(!version)throw new CoreError('NATIVE_PROTOCOL_ERROR','Unrecognized Codex version');
  return version;
}
