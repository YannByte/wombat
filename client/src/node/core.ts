import { spawn } from 'node:child_process';
import { accessSync, constants, existsSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { CoreError } from '../errors.js';
import type { QueryOptions } from '../client.js';
import type { Request } from '../generated/usage-request.js';
import type { PricingRequest,PreferencesRequest,DirectoriesRequest,TimingRequest,CollectionRequest,ConversationRequest } from '../client.js';

const MAX_RESPONSE = 256_000_000;
export interface CoreProcessOptions {
  binaryPath?: string;
  timeoutMs?: number;
  maxResponseBytes?: number;
}
export function binaryPath(configuredPath?: string): string {
  const dir = path.dirname(fileURLToPath(import.meta.url));
  const name = process.platform === 'win32' ? 'wombat-core.exe' : 'wombat-core';
  const override = configuredPath ?? process.env.WOMBAT_CORE_BIN;
  if (!override && existsSync(path.join(dir, 'native')) && !existsSync(path.join(dir, 'native', `${process.platform}-${process.arch}`, name)))
    throw new CoreError('UNSUPPORTED_PLATFORM', `此安装包不包含 ${process.platform}/${process.arch} 内核`);
  const candidates = override ? [path.resolve(override)]
    : [path.join(dir, 'native', `${process.platform}-${process.arch}`, name), path.join(dir, name), path.join(dir, '..', name), path.join(dir, '..', '..', '..', 'dist', name)];
  for (const file of candidates) { try { accessSync(file, constants.X_OK); return file; } catch { /* Try the next build location. */ } }
  throw new CoreError('CORE_UNAVAILABLE', 'Rust 内核未构建或不可执行，请先运行 pnpm build');
}
export function decode<T>(output: string): T {
  let result: { ok: boolean; value?: T; error?: string; code?: string; details?: unknown };
  try { result = JSON.parse(output); } catch { throw new CoreError('PROTOCOL_ERROR', 'Rust 内核返回了无效 JSON'); }
  if (!result || typeof result !== 'object' || typeof result.ok !== 'boolean') {
    throw new CoreError('PROTOCOL_ERROR', 'Rust 内核返回了无效响应');
  }
  if (!result.ok) throw new CoreError(result.code || 'CORE_ERROR', result.error || 'Rust 内核执行失败', result.details ?? undefined);
  if (!Object.hasOwn(result, 'value')) throw new CoreError('PROTOCOL_ERROR', 'Rust 内核响应缺少 value');
  return result.value as T;
}
export function invokeCore(request: Request, options: QueryOptions, processOptions: CoreProcessOptions): Promise<unknown> {
  return invokeOperation('usage_app', request, options, processOptions);
}
export function invokePricesCore(request: PricingRequest & { document?: string; attempt_id?: string; error_code?: string }, options: QueryOptions, processOptions: CoreProcessOptions): Promise<unknown> {
  return invokeOperation('prices', request, options, processOptions);
}
export function invokeOperation(op: 'conversation' | 'collection' | 'monitor' | 'collection_ingest' | 'timing' | 'usage_app' | 'prices' | 'live_endpoint' | 'preferences' | 'directories' | 'account_history' | 'native_account' | 'native_allowance_gate' | 'native_hook_context', request: ConversationRequest | CollectionRequest | TimingRequest | Request | PricingRequest | PreferencesRequest | DirectoriesRequest | Record<string, unknown>, options: QueryOptions, processOptions: CoreProcessOptions): Promise<unknown> {
  if (options.signal?.aborted) return Promise.reject(new CoreError('CANCELLED', '已取消'));
  const binary = binaryPath(processOptions.binaryPath);
  const maxResponseBytes = processOptions.maxResponseBytes ?? MAX_RESPONSE;
  const timeoutMs = processOptions.timeoutMs ?? 600_000;
  if (!Number.isSafeInteger(maxResponseBytes) || maxResponseBytes <= 0 || !Number.isSafeInteger(timeoutMs) || timeoutMs <= 0) {
    throw new CoreError('INVALID_ARGUMENT', '内核超时和响应上限必须是正整数');
  }
  return new Promise((resolve, reject) => {
    // Explicit developer overrides may be Node fixtures; Windows cannot execute shebang scripts.
    const script = /\.[cm]js$/.test(binary);
    const child = spawn(script ? process.execPath : binary, script ? [binary] : [], { windowsHide: true, stdio: ['pipe', 'pipe', 'pipe'], detached: process.platform !== 'win32' });
    const chunks: Buffer[] = []; let bytes = 0; let errors = ''; let progress = ''; let failure: Error | null = null;
    let killTimer: ReturnType<typeof setTimeout> | undefined;
    const stop = () => {
      try { if (process.platform !== 'win32' && child.pid) process.kill(-child.pid, 'SIGTERM'); else child.kill('SIGTERM'); } catch { /* Already exited. */ }
      killTimer ??= setTimeout(() => { try { if (process.platform !== 'win32' && child.pid) process.kill(-child.pid, 'SIGKILL'); else child.kill('SIGKILL'); } catch { /* Already exited. */ } }, 1_000);
      killTimer.unref();
    };
    const abort = () => { failure = new CoreError('CANCELLED', '已取消'); stop(); };
    const interrupt = () => { abort(); };
    const onExit = () => { stop(); };
    process.once('SIGINT', interrupt);
    process.once('SIGTERM', interrupt);
    process.once('exit', onExit);
    options.signal?.addEventListener('abort', abort, { once: true });
    const timer = setTimeout(() => { failure = new CoreError('TIMEOUT', 'Rust 内核执行超时'); stop(); }, timeoutMs);
    const cleanup = () => { clearTimeout(timer); if (killTimer) clearTimeout(killTimer); options.signal?.removeEventListener('abort', abort);
      process.removeListener('SIGINT', interrupt); process.removeListener('SIGTERM', interrupt); process.removeListener('exit', onExit); };
    child.stdout.on('data', (chunk: Buffer) => {
      bytes += chunk.length;
      if (bytes > maxResponseBytes) { failure = new CoreError('OUTPUT_LIMIT', 'Rust 内核响应超过上限，请缩小扫描范围'); stop(); }
      else chunks.push(chunk);
    });
    child.stderr.setEncoding('utf8');
    child.stderr.on('data', (chunk: string) => {
      errors = (errors + chunk).slice(-8000); progress += chunk;
      let end: number;
      while ((end = progress.indexOf('\n')) >= 0) {
        const row = progress.slice(0, end); progress = progress.slice(end + 1);
        try { const event = JSON.parse(row); if (typeof event.stage === 'string') options.onProgress?.(event.stage); } catch { /* Non-progress diagnostics remain in stderr. */ }
      }
      if (progress.length > 8000) progress = progress.slice(-8000);
    });
    child.stdin.on('error', error => { failure ||= error; });
    child.on('error', error => { cleanup(); reject(error); });
    child.on('close', code => {
      cleanup();
      if (failure) { reject(failure); return; }
      if (code !== 0) { reject(new CoreError('CORE_ERROR', `Rust 内核退出码 ${String(code)}：${errors.slice(-1000)}`)); return; }
      try { resolve(decode<unknown>(Buffer.concat(chunks).toString('utf8'))); } catch (error) { reject(error); }
    });
    child.stdin.end(JSON.stringify({ op, args: request }));
  });
}
