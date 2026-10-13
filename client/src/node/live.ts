import type {CodexOptions} from './codex/process.js';
import type {captureHooks} from './codex/hooks.js';
import { spawn } from 'node:child_process';
import { createConnection } from 'node:net';
import { setTimeout as delay } from 'node:timers/promises';
import { CoreError } from '../errors.js';
import type { ConfigRequest, OptimizeRequest, LiveRequest, QueryOptions,HandoffRequest,TimingRequest,MonitorRequest } from '../client.js';
import { binaryPath, decode, invokeOperation, type CoreProcessOptions } from './core.js';
import { isLocalAccessDenied, localAccessError } from './local-access.js';

type ProductRequest = {monitor:MonitorRequest} | { timing: TimingRequest } | LiveRequest | { config: ConfigRequest } | { optimize: OptimizeRequest } | {handoff:HandoffRequest};
type NativeRequest = ProductRequest & { nativeHooks?: Awaited<ReturnType<typeof captureHooks>> };
function exchange(socket: string, request: NativeRequest, options: QueryOptions, config: CoreProcessOptions): Promise<unknown> {
  return new Promise((resolve, reject) => {
    const client = createConnection(socket);
    const chunks: Buffer[] = []; let bytes = 0, done = false;
    const finish = (error?: Error, result?: unknown) => {
      if (done) return; done = true; clearTimeout(timer); options.signal?.removeEventListener('abort', abort); client.destroy();
      if (error) reject(error); else resolve(result);
    };
    const abort = () => finish(new CoreError('CANCELLED', '已取消'));
    // Explicit refresh also writes and syncs an immutable generation. Keep the
    // short read deadline separate from that bounded publication operation.
    const timeoutMs = config.timeoutMs ?? ('query' in request && request.query.action === 'refresh' ? 120_000 : 12_000);
    const timer = setTimeout(() => finish(new CoreError('TIMEOUT', '实时用量查询超时')), timeoutMs);
    options.signal?.addEventListener('abort', abort, { once: true });
    if (options.signal?.aborted) { abort(); return; }
    client.on('connect', () => client.write(JSON.stringify(request) + '\n'));
    client.on('error', (error) => finish(isLocalAccessDenied(error) ? localAccessError() : error));
    client.on('data', (chunk: Buffer) => {
      bytes += chunk.length;
      if (bytes > (config.maxResponseBytes ?? 256_000_000)) { finish(new CoreError('OUTPUT_LIMIT', '实时用量响应超过上限')); return; }
      chunks.push(chunk);
      // A complete response is newline-delimited. Named-pipe servers retain their
      // send buffer until the client closes; waiting only for EOF would deadlock.
      if (chunk.includes(10)) {
        try { finish(undefined, decode(Buffer.concat(chunks).toString('utf8'))); }
        catch (error) { finish(error as Error); }
      }
    });
    client.on('end', () => { try { finish(undefined, decode(Buffer.concat(chunks).toString('utf8'))); } catch (e) { finish(e as Error); } });
  });
}
export async function queryLive(request: ProductRequest, options: QueryOptions, config: CoreProcessOptions & CodexOptions): Promise<unknown> {
  if (options.signal?.aborted) throw new CoreError('CANCELLED', '已取消');
  let observed: NativeRequest = request;
  const product = 'config' in request ? request.config : 'optimize' in request ? request.optimize : 'handoff' in request ? request.handoff : undefined;
  const fresh = product && product.action !== 'capabilities' && product.action !== 'activity' && (!product.readView || product.action === 'recheck' || product.action === 'send');
  if (fresh) {
    const {captureHooks} = await import('./codex/hooks.js');
    observed = {...request, nativeHooks: await captureHooks({roots:product.roots,projectRoots:product.projectRoots}, options, config)};
  }
  if ('query' in request && request.query.action === 'refresh') options.onProgress?.('同步本机日志并保存用量');
  const endpoint = await invokeOperation('live_endpoint', {}, options, config) as { protocolVersion?: number; socket?: string };
  if (endpoint.protocolVersion !== 2 || typeof endpoint.socket !== 'string') throw new CoreError('PROTOCOL_ERROR', '实时用量接口版本不兼容');
  let started = false;
  const deadline = Date.now() + 5_000;
  for (;;) {
    try { return await exchange(endpoint.socket, observed, options, config); }
    catch (error) {
      if (!['ENOENT', 'ECONNREFUSED'].includes((error as NodeJS.ErrnoException).code ?? '') || Date.now() >= deadline) throw error;
      if (!started) {
        // This shared service owns its lifecycle. Never attach it to caller process-tree cleanup.
        const binary = binaryPath(config.binaryPath);
        const script = /\.[cm]js$/.test(binary);
        const child = spawn(script ? process.execPath : binary, script ? [binary, '--serve-usage'] : ['--serve-usage'], { detached: true, windowsHide: true, stdio: 'ignore' });
        started = true;
        const retry = () => { started = false; };
        child.once('error', retry); child.once('exit', retry); child.unref();
      }
      try { await delay(75, undefined, { signal: options.signal }); }
      catch { throw new CoreError('CANCELLED', '已取消'); }
    }
  }
}
