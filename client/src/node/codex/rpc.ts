import { Duplex } from 'node:stream';
import { StringDecoder } from 'node:string_decoder';
import WebSocket from 'ws';
import packageMetadata from '../../../package.json' with { type: 'json' };
import { CoreError } from '../../errors.js';
import type { QueryOptions } from '../../client.js';
import { spawnCodex, stopCodex, type CodexOptions } from './process.js';
import { NativeDiagnostics } from '../local-access.js';

type Method =
  | 'skills/list'
  | 'hooks/list'
  | 'config/read'
  | 'account/read'
  | 'account/rateLimits/read'
  | 'account/usage/read'
  | 'thread/start'
  | 'thread/queue/add'
  | 'thread/read';
type Pending = {
  resolve: (value: unknown) => void;
  reject: (error: Error) => void;
  timer: ReturnType<typeof setTimeout>;
};
const MAX_BYTES = 8 * 1024 * 1024;

/** Native JSON-RPC only. No generic RPC method crosses the product boundary. */
export async function connectCodex(managed: boolean, query: QueryOptions, options: CodexOptions) {
  if (query.signal?.aborted) throw new CoreError('CANCELLED', 'Cancelled');
  const child = spawnCodex(managed ? ['app-server', 'proxy'] : ['app-server'], options);
  const pending = new Map<number, Pending>();
  const diagnostics = new NativeDiagnostics();
  let terminalError: Error | undefined;
  let sequence = 0,
    bytes = 0,
    closed = false;
  const socket = managed
    ? new WebSocket('ws://localhost', {
        createConnection: () => Duplex.from({ readable: child.stdout, writable: child.stdin }),
        perMessageDeflate: false,
        maxPayload: MAX_BYTES,
        handshakeTimeout: options.codexTimeoutMs ?? 20_000,
      })
    : undefined;
  const fail = (error: Error) => {
    if (closed) return;
    closed = true;
    terminalError = error;
    for (const w of pending.values()) {
      clearTimeout(w.timer);
      w.reject(error);
    }
    pending.clear();
    socket?.terminate();
    stopCodex(child);
    cleanup();
  };
  const abort = () => fail(new CoreError('CANCELLED', 'Cancelled'));
  const disconnected = () => terminalError ?? diagnostics.error('CODEX_DISCONNECTED', 'Codex disconnected');
  const close = () => fail(disconnected());
  const cleanup = () => {
    query.signal?.removeEventListener('abort', abort);
    process.off('SIGINT', abort);
    process.off('SIGTERM', abort);
    process.off('exit', close);
  };
  query.signal?.addEventListener('abort', abort, { once: true });
  process.once('SIGINT', abort);
  process.once('SIGTERM', abort);
  process.once('exit', close);
  const receive = (raw: string) => {
    let message: unknown;
    try {
      message = JSON.parse(raw);
    } catch {
      fail(new CoreError('NATIVE_PROTOCOL_ERROR', 'Invalid native response'));
      return;
    }
    if (
      !message ||
      typeof message !== 'object' ||
      !('id' in message) ||
      typeof message.id !== 'number'
    )
      return;
    const w = pending.get(message.id);
    if (!w) return;
    pending.delete(message.id);
    clearTimeout(w.timer);
    if ('error' in message)
      w.reject(new CoreError('CODEX_REQUEST_REJECTED', 'Codex rejected the request'));
    else if ('result' in message) { diagnostics.clear(); w.resolve(message.result); }
    else w.reject(new CoreError('NATIVE_PROTOCOL_ERROR', 'Invalid native response'));
  };
  const count = (chunk: Buffer | string) => {
    bytes += Buffer.byteLength(chunk);
    if (bytes > MAX_BYTES) {
      fail(new CoreError('OUTPUT_LIMIT', 'Native response too large'));
      return false;
    }
    return true;
  };
  child.stderr.on('data', (chunk: Buffer) => { if (count(chunk)) diagnostics.write(chunk); });
  const streamError = (error: Error) => { diagnostics.observe(error); close(); };
  child.stdin.on('error', streamError);
  child.stdout.on('error', streamError);
  child.on('error', (error) => { diagnostics.observe(error); fail(diagnostics.error('CODEX_UNAVAILABLE', 'Codex unavailable')); });
  child.on('close', close);
  let text = '';
  const decoder = new StringDecoder('utf8');
  if (socket) {
    socket.on('message', (data) => {
      const raw = data.toString();
      if (count(raw)) receive(raw);
    });
    socket.on('error', streamError);
    socket.on('close', close);
  } else
    child.stdout.on('data', (chunk: Buffer) => {
      if (!count(chunk)) return;
      text += decoder.write(chunk);
      let end: number;
      while ((end = text.indexOf('\n')) >= 0) {
        const row = text.slice(0, end);
        text = text.slice(end + 1);
        if (row.trim()) receive(row);
      }
    });
  const send = (value: unknown) => {
    if (closed) throw disconnected();
    const json = JSON.stringify(value);
    if (Buffer.byteLength(json) > MAX_BYTES)
      throw new CoreError('OUTPUT_LIMIT', 'Native request too large');
    if (socket) socket.send(json);
    else child.stdin.write(json + '\n');
  };
  const request = (method: string, params: unknown) =>
    new Promise<unknown>((resolve, reject) => {
      if (closed) {
        reject(disconnected());
        return;
      }
      const id = ++sequence,
        timer = setTimeout(() => {
          pending.delete(id);
          reject(new CoreError('CODEX_TIMEOUT', 'Codex request timed out'));
        }, options.codexTimeoutMs ?? 20_000);
      pending.set(id, { resolve, reject, timer });
      try {
        send({ jsonrpc: '2.0', id, method, params });
      } catch (error) {
        pending.delete(id);
        clearTimeout(timer);
        reject(error);
      }
    });
  try {
    if (query.signal?.aborted) {
      abort();
      throw new CoreError('CANCELLED', 'Cancelled');
    }
    if (socket)
      await new Promise<void>((resolve, reject) => {
        const timer = setTimeout(
          () => reject(new CoreError('CODEX_TIMEOUT', 'Codex connection timed out')),
          options.codexTimeoutMs ?? 20_000,
        );
        socket.once('open', () => {
          clearTimeout(timer);
          resolve();
        });
        socket.once('error', () => {
          clearTimeout(timer);
          reject(terminalError ?? diagnostics.error('CODEX_UNAVAILABLE', 'Codex unavailable'));
        });
        socket.once('close', () => {
          clearTimeout(timer);
          reject(disconnected());
        });
      });
    await request('initialize', {
      clientInfo: { name: 'wombat', version: packageMetadata.version },
      capabilities: { experimentalApi: true },
    });
    send({ jsonrpc: '2.0', method: 'initialized' });
    return { request: (method: Method, params: unknown) => request(method, params), close };
  } catch (error) {
    close();
    throw error;
  }
}
