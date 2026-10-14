import { type ServerResponse, type IncomingMessage } from 'node:http';
import { timingSafeEqual } from 'node:crypto';
import { CoreError, type UsageClient } from '@wombat/client';
import { publishTiming } from './timing.js';
import { BODY_LIMIT, OUTPUT_LIMIT } from './limits.js';
import type { loadAssets } from './assets.js';
import type { WebReadScope } from './read-scope.js';

export function createHttpHandler({
  assets,
  authorization,
  client,
  rawClient,
  readScope,
  isClosing,
  host,
  origin,
}: {
  assets: Awaited<ReturnType<typeof loadAssets>>;
  authorization: Buffer;
  client: UsageClient;
  rawClient: UsageClient;
  readScope: WebReadScope;
  isClosing: () => boolean;
  host: () => string;
  origin: () => string;
}) {
  const active = new Set<AbortController>();
  const { snapshots, configViews, snapshotSources, incompleteSnapshotSources, updateGrants } =
    readScope;
  async function handle(req: IncomingMessage, res: ServerResponse): Promise<void> {
    res.setHeader('X-Content-Type-Options', 'nosniff');
    res.setHeader('Referrer-Policy', 'no-referrer');
    res.setHeader('Cache-Control', 'no-store');
    res.setHeader(
      'Content-Security-Policy',
      "default-src 'none'; script-src 'self'; style-src 'self' 'unsafe-inline'; img-src 'self'; connect-src 'self'; base-uri 'none'; frame-ancestors 'none'; form-action 'self'",
    );
    const reject = (status: number) => {
      res.writeHead(status);
      res.end();
    };
    if (isClosing() || req.headers.host !== host()) return reject(403);
    if (req.url?.startsWith('/api/')) {
      const supplied = Buffer.from(req.headers.authorization ?? '');
      if (
        supplied.length !== authorization.length ||
        !timingSafeEqual(supplied, authorization) ||
        req.headers.origin !== origin()
      )
        return reject(403);
      if (req.method !== 'POST') return reject(405);
      if (
        ![
          '/api/query',
          '/api/live',
          '/api/prices',
          '/api/config',
          '/api/optimize',
          '/api/preferences',
          '/api/directories',
          '/api/account',
          '/api/handoff',
          '/api/timing',
          '/api/conversation',
          '/api/collection',
          '/api/setup',
          '/api/monitor',
        ].includes(req.url)
      )
        return reject(404);
      if (req.headers['content-type'] !== 'application/json') return reject(415);
      if (Number(req.headers['content-length'] ?? 0) > BODY_LIMIT) return reject(413);
      if (active.size >= 8) return reject(429);
      const controller = new AbortController();
      active.add(controller);
      const timer = setTimeout(() => {
        controller.abort();
        res.destroy();
      }, 120_000);
      const abort = () => controller.abort();
      res.once('close', abort);
      let sent = 0;
      const send = (frame: unknown) => {
        if (controller.signal.aborted) throw new CoreError('CANCELLED', 'Cancelled');
        const line = JSON.stringify(frame) + '\n';
        sent += Buffer.byteLength(line);
        if (sent > OUTPUT_LIMIT) throw new CoreError('OUTPUT_LIMIT', 'Response too large');
        res.write(line);
      };
      try {
        let size = 0;
        const chunks: Buffer[] = [];
        for await (const chunk of req) {
          size += chunk.length;
          if (size > BODY_LIMIT) {
            reject(413);
            return;
          }
          chunks.push(chunk);
        }
        let request: unknown;
        try {
          request = JSON.parse(Buffer.concat(chunks).toString('utf8'));
        } catch {
          reject(400);
          return;
        }
        res.setHeader('Content-Type', 'application/x-ndjson; charset=utf-8');
        const query = {
          signal: controller.signal,
          onProgress: (stage: string) => send({ type: 'progress', stage }),
        };
        // Revocation from another CLI/window is authoritative at the next request.
        if (
          req.url !== '/api/directories' &&
          req.url !== '/api/account' &&
          req.url !== '/api/timing' &&
          req.url !== '/api/conversation' &&
          rawClient.directories
        )
          updateGrants(await rawClient.directories({ action: 'list' }, query));
        // createUsageClient validates each unknown payload before the typed transport runs.
        const value =
          req.url === '/api/monitor'
            ? await client.monitor!(
                request as Parameters<NonNullable<UsageClient['monitor']>>[0],
                query,
              )
            : req.url === '/api/setup'
              ? await client.setup!(
                  request as Parameters<NonNullable<UsageClient['setup']>>[0],
                  query,
                )
              : req.url === '/api/collection'
                ? await client.collection!(
                    request as Parameters<NonNullable<UsageClient['collection']>>[0],
                    query,
                  )
                : req.url === '/api/conversation'
                  ? await client.conversation!(
                      request as Parameters<NonNullable<UsageClient['conversation']>>[0],
                      query,
                    )
                  : req.url === '/api/timing'
                    ? await client.timing!(
                        request as Parameters<NonNullable<UsageClient['timing']>>[0],
                        query,
                      )
                    : req.url === '/api/query'
                      ? await client.query(request as Parameters<UsageClient['query']>[0], query)
                      : req.url === '/api/account'
                        ? await client.account!(
                            request as Parameters<NonNullable<UsageClient['account']>>[0],
                            query,
                          )
                        : req.url === '/api/handoff'
                          ? await client.handoff!(
                              request as Parameters<NonNullable<UsageClient['handoff']>>[0],
                              query,
                            )
                          : req.url === '/api/prices'
                            ? await client.prices(
                                request as Parameters<UsageClient['prices']>[0],
                                query,
                              )
                            : req.url === '/api/config'
                              ? await client.config!(
                                  request as Parameters<NonNullable<UsageClient['config']>>[0],
                                  query,
                                )
                              : req.url === '/api/optimize'
                                ? await client.optimize!(
                                    request as Parameters<NonNullable<UsageClient['optimize']>>[0],
                                    query,
                                  )
                                : req.url === '/api/directories'
                                  ? await client.directories!(
                                      request as Parameters<
                                        NonNullable<UsageClient['directories']>
                                      >[0],
                                      query,
                                    )
                                  : req.url === '/api/preferences'
                                    ? await client.preferences!(
                                        request as Parameters<
                                          NonNullable<UsageClient['preferences']>
                                        >[0],
                                        query,
                                      )
                                    : await client.live!(
                                        request as Parameters<NonNullable<UsageClient['live']>>[0],
                                        query,
                                      );
        const result =
          'result' in value ? value.result : 'snapshotRef' in value ? value : undefined;
        if ('readView' in value) {
          if (typeof value.readView === 'string') configViews.add(value.readView);
          if (configViews.size > 128) configViews.delete(configViews.values().next().value!);
          if ('usageRevision' in value && value.usageRevision) snapshots.add(value.usageRevision);
        }
        if (result) {
          snapshots.add(result.snapshotRef.snapshotId);
          snapshotSources.set(
            result.snapshotRef.snapshotId,
            new Set(result.quality.sources.map((s) => s.source.id)),
          );
          if (result.quality.detailSummary?.omittedSources)
            incompleteSnapshotSources.add(result.snapshotRef.snapshotId);
          else incompleteSnapshotSources.delete(result.snapshotRef.snapshotId);
          if (snapshotSources.size > 128)
            snapshotSources.delete(snapshotSources.keys().next().value!);
        }
        if (req.url === '/api/timing') {
          // Narrow the independently validated timing result; never publish share aliases.
          publishTiming(value as import('@wombat/client').TimingResult, snapshots);
        }
        if (snapshots.size > 128) snapshots.delete(snapshots.values().next().value!);
        send({ type: 'result', value });
      } catch (error) {
        if (!controller.signal.aborted) {
          const failure =
            error instanceof CoreError ? error : new CoreError('INTERNAL_ERROR', 'Request failed');
          // Reserve a separate bounded terminal error, even after an oversized result.
          res.end(
            JSON.stringify({ type: 'error', code: failure.code, message: failure.message }) + '\n',
          );
        }
      } finally {
        clearTimeout(timer);
        active.delete(controller);
        res.off('close', abort);
        res.end();
      }
      return;
    }
    if (req.method !== 'GET' && req.method !== 'HEAD') return reject(405);
    const pathname = new URL(req.url ?? '/', origin()).pathname;
    const asset = assets.get(pathname === '/' ? '/index.html' : pathname);
    if (!asset) return reject(404);
    res.setHeader('Content-Type', asset.type);
    res.end(req.method === 'HEAD' ? undefined : asset.body);
  }
  return {
    handle,
    cancelAll: () => {
      for (const controller of active) controller.abort();
    },
  };
}
