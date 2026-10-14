import { CoreError, createUsageClient, type UsageClient, type UsageRequest } from '@wombat/client';
import { timingAccess, activityAccess } from './timing.js';
import type { WebHostOptions } from './index.js';
import type { WebReadScope } from './read-scope.js';
import type { backgroundPrices } from './automatic-prices.js';

export function createScopedClient(
  options: WebHostOptions,
  readScope: WebReadScope,
  prices: ReturnType<typeof backgroundPrices>,
): UsageClient {
  const {
    snapshots,
    configViews,
    snapshotSources,
    authorizedSources,
    incompleteSnapshotSources,
    observeProjects,
    discoverProject,
    projectRoots,
    updateGrants,
    scope,
    revalidateFixed,
  } = readScope;
  const timing = timingAccess(options.client, snapshots, () => readScope.roots, revalidateFixed);
  const activity = activityAccess(
    options.client,
    snapshots,
    () => readScope.roots,
    revalidateFixed,
  );
  return createUsageClient({
    timing,
    conversation: async (r, q) => {
      if (!options.client.conversation)
        throw new CoreError('CORE_UNAVAILABLE', 'Conversation access unavailable');
      if (
        r.roots?.length ||
        !snapshots.has(r.snapshotId) ||
        (r.baselineSnapshotId && !snapshots.has(r.baselineSnapshotId))
      )
        throw new CoreError(
          'INVALID_ARGUMENT',
          'Conversation access requires host-published read identities',
        );
      await revalidateFixed(q);
      if (
        !snapshots.has(r.snapshotId) ||
        (r.baselineSnapshotId && !snapshots.has(r.baselineSnapshotId))
      )
        throw new CoreError('VIEW_EXPIRED', 'Read identity is no longer authorized');
      const result = await options.client.conversation({ ...r, roots: readScope.roots ?? [] }, q);
      if (
        !snapshots.has(r.snapshotId) ||
        (r.baselineSnapshotId && !snapshots.has(r.baselineSnapshotId))
      )
        throw new CoreError('VIEW_EXPIRED', 'Read identity is no longer authorized');
      return result;
    },
    monitor: async (r, q) => {
      if (!options.client.monitor)
        throw new CoreError('MONITOR_UNAVAILABLE', 'Monitor unavailable');
      await revalidateFixed(q);
      if (!readScope.sourcesComplete) {
        const view = options.client.live
          ? (
              await options.client.live(
                {
                  query: {
                    action: 'usage',
                    roots: readScope.roots,
                    scope: { allTime: true },
                    limit: 1,
                  },
                  mode: 'fresh',
                },
                q,
              )
            ).result
          : await options.client.query(
              { action: 'usage', roots: readScope.roots, scope: { allTime: true }, limit: 1 },
              q,
            );
        observeProjects(view);
      }
      const allowedScope = (scope: import('@wombat/client').UsageRequest['scope']) =>
        (!scope?.project ||
          readScope.hasProject(scope.project) ||
          readScope.configuredProjects?.includes(scope.project)) &&
        (!scope?.sourceInstanceId || authorizedSources.has(scope.sourceInstanceId));
      const allowedNotification = (
        n: import('@wombat/client').MonitorResult['notifications'][number],
      ) => allowedScope(n.scope) && n.sourceInstanceIds.every((id) => authorizedSources.has(id));
      if (r.action === 'upsert') {
        const settings = await options.client.monitor({ action: 'list' }, q);
        const existing = settings.plans.find((p) => p.id === r.plan.id);
        if (existing && !allowedScope(existing.scope))
          throw new CoreError('NOT_FOUND', 'Monitor plan is outside this host');
        await discoverProject(r.plan.scope?.project, q);
        if (!allowedScope(r.plan.scope))
          throw new CoreError('SOURCE_NOT_AUTHORIZED', 'Source is outside this host');
      }
      if (r.action === 'remove' || r.action === 'acknowledge') {
        const settings = await options.client.monitor({ action: 'list' }, q);
        const allowed =
          r.action === 'remove'
            ? settings.plans.some((p) => p.id === r.id && allowedScope(p.scope))
            : settings.notifications.some(
                (n) => n.id === r.notificationId && allowedNotification(n),
              );
        if (!allowed) throw new CoreError('NOT_FOUND', 'Monitor item is outside this host');
      }
      if (r.action === 'check') {
        if (!snapshots.has(r.snapshotId))
          throw new CoreError('VIEW_EXPIRED', 'Unknown host usage view');
        if (incompleteSnapshotSources.has(r.snapshotId) || !snapshotSources.has(r.snapshotId)) {
          const request: UsageRequest = {
            action: 'usage',
            snapshotId: r.snapshotId,
            scope: { allTime: true },
            limit: 1,
          };
          const view =
            options.client.live && r.snapshotId.startsWith('live:')
              ? (await options.client.live({ query: request, mode: 'cached' }, q)).result
              : await options.client.query(request, q);
          if (view.snapshotRef.snapshotId !== r.snapshotId)
            throw new CoreError('PROTOCOL_ERROR', 'Usage view mismatch');
          snapshotSources.set(
            r.snapshotId,
            new Set(view.quality.sources.map((source) => source.source.id)),
          );
          incompleteSnapshotSources.delete(r.snapshotId);
        }
        const settings = await options.client.monitor({ action: 'list' }, q);
        for (const id of r.ids) {
          const plan = settings.plans.find((p) => p.id === id);
          if (!plan) throw new CoreError('NOT_FOUND', 'Monitor plan not found');
          await discoverProject(plan.scope?.project, q);
          const source = plan.scope?.sourceInstanceId;
          if (source && !snapshotSources.get(r.snapshotId)?.has(source))
            throw new CoreError('SOURCE_NOT_AUTHORIZED', 'Source is outside this read view');
        }
        await revalidateFixed(q);
        if (!snapshots.has(r.snapshotId)) throw new CoreError('VIEW_EXPIRED', 'Read scope changed');
      }
      const result = await options.client.monitor(r, q);
      await revalidateFixed(q);
      return {
        ...result,
        plans: result.plans.filter((p) => allowedScope(p.scope)),
        notifications: result.notifications.filter(allowedNotification),
      };
    },
    setup: async (r, q) => {
      if (!options.client.setup) throw new CoreError('SETUP_UNAVAILABLE', 'Setup unavailable');
      if (r.roots != null) throw new CoreError('INVALID_ARGUMENT', 'Web scope is fixed at startup');
      await revalidateFixed(q);
      await discoverProject(r.project, q);
      return options.client.setup({ ...r, roots: readScope.roots }, q);
    },
    collection: async (r, q) => {
      if (!options.client.collection)
        throw new CoreError('COLLECTION_UNAVAILABLE', 'Collection unavailable');
      if (r.roots != null) throw new CoreError('INVALID_ARGUMENT', 'Web scope is fixed at startup');
      await revalidateFixed(q);
      await discoverProject(r.project, q);
      return options.client.collection({ ...r, roots: readScope.roots }, q);
    },
    query: async (r, q) => {
      const result = await options.client.query(scope(r), q);
      observeProjects(result);
      return result;
    },
    prices: (r, q) => (r.action === 'auto_update' ? prices.update() : options.client.prices(r, q)),
    live: async (r, q) => {
      if (!options.client.live) throw new CoreError('LIVE_UNAVAILABLE', 'Live queries unavailable');
      const result = prices.observe(
        r,
        await options.client.live({ ...r, query: scope(r.query) }, q),
      );
      observeProjects(result.result);
      return result;
    },
    config: async (r, q) => {
      if (!options.client.config)
        throw new CoreError('CONFIG_UNAVAILABLE', 'Configuration queries unavailable');
      if (
        r.roots != null ||
        r.projectRoots != null ||
        (r.readView != null && !configViews.has(r.readView)) ||
        (r.snapshotId != null && !snapshots.has(r.snapshotId))
      )
        throw new CoreError('INVALID_ARGUMENT', 'Web scope is fixed at startup');
      await discoverProject(r.scope?.project, q);
      const projects = projectRoots(r.scope?.project);
      const result = await options.client.config(
        { ...r, roots: readScope.roots, projectRoots: projects },
        q,
      );
      return {
        ...result,
        authorizedSourceRoots: readScope.roots ?? result.authorizedSourceRoots,
        authorizedProjects: projects,
        hostRestartCommand: options.restartCommand ?? null,
      };
    },
    optimize: async (r, q) => {
      if (r.action === 'activity') return activity(r, q);
      if (!options.client.optimize)
        throw new CoreError('OPTIMIZE_UNAVAILABLE', 'Optimization queries unavailable');
      if (
        r.roots != null ||
        r.projectRoots != null ||
        (r.readView != null && !configViews.has(r.readView))
      )
        throw new CoreError('INVALID_ARGUMENT', 'Web scope is fixed at startup');
      await discoverProject(r.project, q);
      return options.client.optimize(
        { ...r, roots: readScope.roots, projectRoots: projectRoots(r.project) },
        q,
      );
    },
    preferences: (r, q) => {
      if (!options.client.preferences)
        throw new CoreError('PREFERENCES_UNAVAILABLE', 'Preferences unavailable');
      return options.client.preferences(r, q);
    },
    directories: async (r, q) => {
      if (!options.client.directories)
        throw new CoreError('DIRECTORY_PICKER_UNAVAILABLE', 'Directory authorization unavailable');
      if (r.path != null || r.action === 'authorize')
        throw new CoreError('INVALID_ARGUMENT', 'Use the host directory selector');
      const result = await options.client.directories(r, q);
      if (r.action === 'list' || r.action === 'confirm' || r.action === 'revoke')
        updateGrants(result);
      return result;
    },
    ...{
      account: (r, q) => {
        if (!options.client.account)
          throw new CoreError('ACCOUNT_UNAVAILABLE', 'Account unavailable');
        return options.client.account(r, q);
      },
      handoff: async (r, q) => {
        if (!options.client.handoff)
          throw new CoreError('HANDOFF_UNAVAILABLE', 'Handoff unavailable');
        if (
          r.roots != null ||
          r.projectRoots != null ||
          (r.readView != null && !configViews.has(r.readView))
        )
          throw new CoreError('INVALID_ARGUMENT', 'Web scope is fixed at startup');
        await discoverProject(r.project, q);
        return options.client.handoff(
          { ...r, roots: readScope.roots, projectRoots: projectRoots(r.project) },
          q,
        );
      },
    },
  });
}
