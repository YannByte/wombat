import type { UsageClient } from '@wombat/client';

/** Apply explicit fixture roots to real native calls; no transport or domain result is mocked. */
export function projectTestClient(client: UsageClient, roots: string[], projectRoots: string[] = []): UsageClient {
  return {
    ...client,
    query: (r, q) => client.query({ ...r, roots: r.snapshotId ? r.roots : r.roots ?? roots }, q),
    live: (r, q) => client.live!({ ...r, query: { ...r.query, roots: r.query.snapshotId ? r.query.roots : r.query.roots ?? roots } }, q),
    config: (r, q) => client.config!({ roots, projectRoots, ...r }, q),
    optimize: (r, q) => client.optimize!({ roots, projectRoots, ...r }, q),
    collection: (r, q) => client.collection!({ roots, ...r }, q),
    setup: (r, q) => client.setup!({ roots, ...r }, q),
  };
}
