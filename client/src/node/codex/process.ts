import {spawn, type ChildProcessWithoutNullStreams} from 'node:child_process';
import {CoreError} from '../../errors.js';
import type {QueryOptions} from '../../client.js';
import {NativeDiagnostics} from '../local-access.js';

export interface CodexOptions { codexBinaryPath?: string; codexTimeoutMs?: number; codexHome?: string }
export interface CodexInvocation { cwd?: string; env?: NodeJS.ProcessEnv; appServerConfig?: readonly string[] }

export function spawnCodex(args: string[], options: CodexOptions, invocation: CodexInvocation = {}): ChildProcessWithoutNullStreams {
  const binary=options.codexBinaryPath ?? process.env.WOMBAT_CODEX_BIN ?? 'codex';
  const script=/\.[cm]?[jt]s$/.test(binary);
  const env=invocation.env ?? process.env;
  return spawn(script?process.execPath:binary, script?[binary,...args]:args, {
    cwd:invocation.cwd,
    env:options.codexHome?{...env,CODEX_HOME:options.codexHome}:env,
    stdio:['pipe','pipe','pipe'], windowsHide:true, detached:process.platform!=='win32',
  });
}
const stopping = new WeakSet<ChildProcessWithoutNullStreams>();

export function stopCodex(child: ChildProcessWithoutNullStreams): void {
  if (stopping.has(child) || child.pid === undefined) return;
  stopping.add(child);
  const group = process.platform !== 'win32' ? -child.pid : undefined;
  const send = (signal: NodeJS.Signals) => {
    try {
      if (group !== undefined) return process.kill(group, signal);
      if (child.exitCode === null && child.signalCode === null) return child.kill(signal);
    } catch { /* The child or group may have exited while stopping. */ }
    return false;
  };
  if (!send('SIGTERM')) return;

  // POSIX descendants can keep running after the leader has exited or closed its pipes.
  const force = () => {
    clearTimeout(deadline);
    child.off('close', force);
    send('SIGKILL');
  };
  const deadline = setTimeout(force, 1000);
  deadline.unref();
  if (child.stdout.closed && child.stderr.closed) force();
  else child.once('close', force);
}
export async function runCodex(args: string[], query: QueryOptions, config: CodexOptions, invocation?: CodexInvocation): Promise<string> {
  if(query.signal?.aborted)throw new CoreError('CANCELLED','Cancelled');
  return new Promise((resolve,reject)=>{
    const child=spawnCodex(args,config,invocation);const diagnostics=new NativeDiagnostics();let output='',size=0,failure:Error|undefined,done=false;
    const fail=(error:Error)=>{if(!failure){failure=error;stopCodex(child);}};
    const abort=()=>fail(new CoreError('CANCELLED','Cancelled'));
    const stop=()=>{stopCodex(child);};
    const timer=setTimeout(()=>fail(new CoreError('CODEX_TIMEOUT','Codex request timed out')),config.codexTimeoutMs??20_000);
    query.signal?.addEventListener('abort',abort,{once:true});
    process.once('SIGINT',abort);process.once('SIGTERM',abort);process.once('exit',stop);
    const finish=(error?:Error)=>{if(done)return;done=true;clearTimeout(timer);query.signal?.removeEventListener('abort',abort);process.off('SIGINT',abort);process.off('SIGTERM',abort);process.off('exit',stop);error?reject(error):resolve(output);};
    child.stdout.on('data',(chunk:Buffer)=>{size+=chunk.length;if(size>1024*1024)fail(new CoreError('OUTPUT_LIMIT','Codex response too large'));else output+=chunk.toString('utf8');});
    child.stderr.on('data',(chunk:Buffer)=>{size+=chunk.length;if(size>1024*1024)fail(new CoreError('OUTPUT_LIMIT','Codex response too large'));else diagnostics.write(chunk);});
    child.on('error',error=>{diagnostics.observe(error);finish(diagnostics.error('CODEX_UNAVAILABLE','Codex unavailable'));});
    child.on('close',code=>finish(failure??(code===0?undefined:diagnostics.error('CODEX_UNAVAILABLE','Codex command failed'))));
    child.stdin.end();
    if(query.signal?.aborted)abort();
  });
}
export async function nativeVersion(query: QueryOptions, config: CodexOptions, invocation?: CodexInvocation): Promise<string> {
  const text=(await runCodex(['--version'],query,config,invocation)).trim();
  // Preserve the complete host identity; recognizing a version grants no capabilities.
  const version=text.length<=256?text.match(/^codex-cli (\d+\.\d+\.\d+(?:-[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?(?:\+[0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*)?)$/)?.[1]:undefined;
  if(!version)throw new CoreError('NATIVE_PROTOCOL_ERROR','Unrecognized Codex version');
  return version;
}
