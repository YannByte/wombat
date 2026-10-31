import { spawnSync, type ChildProcess } from 'node:child_process';
import spawn from 'cross-spawn';
import { existsSync, readFileSync, realpathSync, statSync } from 'node:fs';
import { closeSync, createWriteStream, openSync, writeSync } from 'node:fs';
import { once } from 'node:events';
import path from 'node:path';
import { setTimeout as delay } from 'node:timers/promises';
import { builtFiles, sourceIdentity } from './build-identity.ts';
import { hash, hashFile, inventory, type PayloadFile } from './artifact-files.ts';

export const VERIFY_E2E_VERSION = 1;
export function stdoutFromLog(log: string): string {
  const pattern = /\n\[(stdout|stderr)\] /g, parts = [...log.matchAll(pattern)];
  return parts.map((part, index) => part[1] === 'stdout' ? log.slice(part.index! + part[0].length, parts[index + 1]?.index ?? log.length) : '').join('');
}
export type Scope = 'integration' | 'e2e' | 'all';
export interface VerifyOptions {
  scope: Scope;
  outputDir: string;
  resume: boolean;
}
export interface ResumeRecord {
  runKey?: unknown;
  stages?: unknown;
}
export interface BoundedCommandResult {
  exitCode: number | null;
  timedOut: boolean;
  outputLimit: boolean;
  interrupted: boolean;
  closeTimedOut: boolean;
  logError?: string;
  spawnError?: Error;
}

export function parseVerifyArgs(args: string[]): VerifyOptions {
  if (args[0] === '--') args = args.slice(1);
  const values = new Map<string, string>();
  let resume = false;
  const usage = 'Usage: verify:e2e --output-dir ABSOLUTE_PATH [--scope integration|e2e|all] [--resume]';
  for (let i = 0; i < args.length; i++) {
    const key = args[i];
    if (key === '--resume' && !resume) { resume = true; continue; }
    if (!['--scope', '--output-dir'].includes(key) || values.has(key) || !args[i + 1] || args[i + 1].startsWith('--')) throw new Error(usage);
    values.set(key, args[++i]);
  }
  const scope = values.get('--scope') ?? 'all';
  if (scope !== 'integration' && scope !== 'e2e' && scope !== 'all') throw new Error('--scope must be integration, e2e, or all');
  const outputDir = values.get('--output-dir');
  if (!outputDir || !path.isAbsolute(outputDir)) throw new Error('--output-dir must be an explicit absolute path outside the repository');
  return { scope, outputDir: path.resolve(outputDir), resume };
}

export function isWithin(parent: string, candidate: string): boolean {
  const relative = path.relative(path.resolve(parent), path.resolve(candidate));
  return relative === '' || (!path.isAbsolute(relative) && relative !== '..' && !relative.startsWith(`..${path.sep}`));
}

export function assertExternalOutputDir(root: string, outputDir: string): string {
  const absolute = path.resolve(outputDir);
  if (!path.isAbsolute(outputDir) || isWithin(root, absolute)) throw new Error('--output-dir must be outside the repository');
  let ancestor = absolute;
  const missing: string[] = [];
  while (!existsSync(ancestor)) {
    const parent = path.dirname(ancestor);
    if (parent === ancestor) throw new Error('Cannot resolve --output-dir parent');
    missing.unshift(path.basename(ancestor));
    ancestor = parent;
  }
  if (!statSync(ancestor).isDirectory()) throw new Error('--output-dir parent must be a directory');
  const prospective = path.resolve(realpathSync(ancestor), ...missing);
  if (isWithin(realpathSync(root), prospective)) throw new Error('--output-dir resolves inside the repository');
  return absolute;
}

export function plannedStages(scope: Scope): string[] {
  return scope === 'all' ? ['integration', 'e2e'] : [scope];
}

export function artifactIdentity(files: PayloadFile[]): string {
  return hash(JSON.stringify(files));
}

export function acceptanceInputIdentity(root: string): string {
  const files = (['tests/integration', 'tests/e2e', 'tests/helpers', 'plugin'] as const).flatMap(directory =>
    inventory(path.join(root, directory)).map(file => ({ ...file, path: `${directory}/${file.path}` })));
  return hash(JSON.stringify(files));
}

export function runFingerprint(input: {
  sourceSha256: string;
  artifactSha256: string;
  acceptanceInputsSha256: string;
  scope: Scope;
}): string {
  return hash(JSON.stringify({ version: VERIFY_E2E_VERSION, ...input }));
}

export function reusableStage(record: ResumeRecord | undefined, stageName: string, runKey: string): boolean {
  if (!record || record.runKey !== runKey || !Array.isArray(record.stages)) return false;
  return record.stages.some(stage => stage !== null && typeof stage === 'object' && !Array.isArray(stage)
    && (stage as { name?: unknown }).name === stageName && (stage as { status?: unknown }).status === 'passed');
}

export async function terminateTree(child: ChildProcess): Promise<boolean> {
  if (!child.pid) return child.exitCode !== null || child.signalCode !== null;
  const alreadyClosed = (child.exitCode !== null || child.signalCode !== null)
    && [child.stdin, child.stdout, child.stderr].every(stream => !stream || stream.closed);
  const closed = alreadyClosed ? Promise.resolve()
    : new Promise<void>(resolve => child.once('close', () => resolve()));
  if (process.platform === 'win32') {
    const killed = spawnSync('taskkill', ['/PID', String(child.pid), '/T', '/F'], { encoding: 'utf8', timeout: 5000, maxBuffer: 64 * 1024, windowsHide: true });
    if (killed.error || killed.status !== 0) child.kill('SIGKILL');
  } else {
    try { process.kill(-child.pid, 'SIGTERM'); } catch { child.kill('SIGTERM'); }
    await Promise.race([closed, delay(1000)]);
    // The direct child can exit while descendants keep running in this process group.
    try { process.kill(-child.pid, 'SIGKILL'); } catch { if (child.exitCode === null && child.signalCode === null) child.kill('SIGKILL'); }
  }
  return await Promise.race([closed.then(() => true), delay(3000).then(() => false)]);
}

export async function runBoundedCommand(options: {
  command: string[]; cwd: string; env: NodeJS.ProcessEnv; logFile: string; timeoutMs: number; maxBytes: number; signal?: AbortSignal;
}): Promise<BoundedCommandResult> {
  const [program, ...args] = options.command;
  if (!program) throw new Error('Command cannot be empty');
  const fd = openSync(options.logFile, 'wx', 0o600);
  writeSync(fd, `start=${new Date().toISOString()}\ntimeoutMs=${options.timeoutMs}\ncommand=${JSON.stringify(options.command)}\n`);
  closeSync(fd);
  const log = createWriteStream(options.logFile, { flags: 'a' });
  let loggedBytes = statSync(options.logFile).size;
  let timedOut = false, outputLimit = false, interrupted = false, closeTimedOut = false;
  let spawnError: Error | undefined, logError: Error | undefined, termination: Promise<boolean> | undefined;
  let resolveForced!: (value: boolean) => void;
  const forced = new Promise<boolean>(resolve => { resolveForced = resolve; });
  const child = spawn(program, args, { cwd: options.cwd, env: options.env, detached: process.platform !== 'win32', stdio: ['ignore', 'pipe', 'pipe'], windowsHide: true });
  const closed = new Promise<{ exitCode: number | null }>(resolve => child.once('close', exitCode => resolve({ exitCode })));
  const stop = (reason: 'timeout' | 'output' | 'signal') => {
    if (reason === 'timeout') timedOut = true;
    if (reason === 'output') outputLimit = true;
    if (reason === 'signal') interrupted = true;
    termination ??= terminateTree(child);
    void termination.then(resolveForced);
  };
  const abort = () => stop('signal');
  options.signal?.addEventListener('abort', abort, { once: true });
  if (options.signal?.aborted) abort();
  const timeout = setTimeout(() => stop('timeout'), options.timeoutMs);
  const writeChunk = (label: string, chunk: Buffer) => {
    if (outputLimit || logError) return;
    const prefix = Buffer.from(`\n[${label}] `), remaining = options.maxBytes - loggedBytes;
    if (remaining <= prefix.length || chunk.byteLength > remaining - prefix.length) {
      const note = Buffer.from('\n[output budget exceeded; process terminated]\n');
      if (remaining > 0) log.write(note.subarray(0, remaining));
      loggedBytes = options.maxBytes; stop('output'); return;
    }
    loggedBytes += prefix.length + chunk.byteLength;
    log.write(prefix); log.write(chunk);
  };
  child.stdout?.on('data', (chunk: Buffer) => writeChunk('stdout', chunk));
  child.stderr?.on('data', (chunk: Buffer) => writeChunk('stderr', chunk));
  child.once('error', error => { spawnError = error; });
  log.once('error', error => { logError = error; stop('output'); });
  let closedResult: { exitCode: number | null } | undefined;
  let forceExpired = false;
  try {
    const result = await Promise.race([closed, forced.then(closeTimedOut => { forceExpired = !closeTimedOut; return undefined; })]);
    closedResult = result;
  } finally {
    clearTimeout(timeout); options.signal?.removeEventListener('abort', abort);
    if (termination) closeTimedOut = !(await termination);
    if (forceExpired || closeTimedOut) { child.stdout?.destroy(); child.stderr?.destroy(); }
    await new Promise<void>(resolve => { if (log.closed) resolve(); else { log.once('finish', () => resolve()); log.once('close', () => resolve()); log.end(); } });
  }
  return { exitCode: closedResult?.exitCode ?? child.exitCode, timedOut, outputLimit, interrupted, closeTimedOut,
    ...(logError ? { logError: logError.message } : {}), ...(spawnError ? { spawnError } : {}) };
}

export function workspaceArtifactFiles(root: string): PayloadFile[] {
  const rootFiles = builtFiles(root).map(file => ({ ...file, path: `dist/${file.path}` }));
  const packageFiles = (['client', 'cli'] as const).flatMap(directory =>
    inventory(path.join(root, directory, 'dist')).map(file => ({ ...file, path: `${directory}/dist/${file.path}` })));
  return [...rootFiles, ...packageFiles];
}

export function currentArtifactIdentity(root: string): string {
  return artifactIdentity(workspaceArtifactFiles(root));
}

export function currentSourceIdentity(root: string): string {
  return sourceIdentity(root);
}
