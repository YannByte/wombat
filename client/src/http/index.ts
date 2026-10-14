import { CoreError, createUsageClient, type QueryOptions, type UsageClient } from '../index.js';

/** Browser transport only. Business payloads are validated by the generated contract. */
export function createHttpClient(options: { origin: string; token: string; fetch?: typeof fetch; maxBytes?: number }): UsageClient {
  const send = async (operation: string, request: unknown, query: QueryOptions): Promise<unknown> => {
    const signal = AbortSignal.any([query.signal ?? new AbortController().signal, AbortSignal.timeout(125_000)]);
    let reader: ReadableStreamDefaultReader<Uint8Array> | undefined;
    try {
      const response = await (options.fetch ?? fetch)(new URL(`/api/${operation}`, options.origin), {
        method: 'POST', credentials: 'omit', cache: 'no-store', redirect: 'error', signal,
        headers: { 'Content-Type': 'application/json', Authorization: `Bearer ${options.token}` }, body: JSON.stringify(request),
      });
      if (!response.ok) throw new CoreError(`HTTP_${response.status}`, `HTTP ${response.status}`);
      if (!response.headers.get('content-type')?.startsWith('application/x-ndjson') || !response.body)
        throw new CoreError('PROTOCOL_ERROR', 'Invalid stream');
      reader = response.body.getReader();
      const decoder = new TextDecoder('utf-8', { fatal: true });
      let pending = '', bytes = 0;
      for (;;) {
        const { value, done } = await reader.read();
        if (done) break;
        bytes += value.byteLength;
        if (bytes > (options.maxBytes ?? 16 * 1024 * 1024)) throw new CoreError('OUTPUT_LIMIT', 'Response too large');
        pending += decoder.decode(value, { stream: true });
        let newline: number;
        while ((newline = pending.indexOf('\n')) >= 0) {
          const frame: unknown = JSON.parse(pending.slice(0, newline)); pending = pending.slice(newline + 1);
          if (!frame || typeof frame !== 'object' || !('type' in frame)) throw new CoreError('PROTOCOL_ERROR', 'Invalid frame');
          if (frame.type === 'progress' && 'stage' in frame && typeof frame.stage === 'string') query.onProgress?.(frame.stage);
          else if (frame.type === 'result' && 'value' in frame) return frame.value;
          else if (frame.type === 'error' && 'code' in frame && typeof frame.code === 'string' && 'message' in frame && typeof frame.message === 'string')
            throw new CoreError(frame.code, frame.message);
          else throw new CoreError('PROTOCOL_ERROR', 'Invalid frame');
        }
      }
      throw new CoreError('PROTOCOL_ERROR', 'Incomplete stream');
    } catch (error) {
      if (query.signal?.aborted) throw new CoreError('CANCELLED', 'Cancelled');
      if (signal.aborted) throw new CoreError('TIMEOUT', 'Request timed out');
      if (error instanceof CoreError) throw error;
      throw new CoreError('TRANSPORT_ERROR', error instanceof Error ? error.message : String(error));
    } finally { await reader?.cancel().catch(() => {}); }
  };
  return createUsageClient({
    setup:(r,q)=>send('setup',r,q),
    monitor: (r,q) => send('monitor',r,q),
    collection: (r,q) => send('collection',r,q),
    query: (r, q) => send('query', r, q),
    prices: (r, q) => send('prices', r, q),
    live: (r, q) => send('live', r, q),
    config: (r, q) => send('config', r, q),
    optimize: (r, q) => send('optimize', r, q),
    preferences: (r, q) => send('preferences', r, q),
    directories: (r, q) => send('directories', r, q),
    conversation: (r,q) => send('conversation',r,q),
    timing: (r, q) => send('timing', r, q),
    account: (r, q) => send('account', r, q),
    handoff: (r, q) => send('handoff', r, q),
  });
}
