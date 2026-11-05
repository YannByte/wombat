import type { CodexOptions } from './codex/process.js';
import type { captureHooks } from './codex/hooks.js';
import type { captureSkills } from './codex/skill-review.js';
import type { ProjectRequest, ArtifactsRequest, ConfigRequest, OptimizeRequest, LiveRequest, QueryOptions, TimingRequest, MonitorRequest, ConversationRequest } from '../client.js';
import { invokeOperation, type CoreProcessOptions } from './core.js';
type ProductRequest = {project:ProjectRequest} | {artifacts: ArtifactsRequest} | {conversation: ConversationRequest} | {monitor: MonitorRequest} | {timing: TimingRequest} | LiveRequest | {config: ConfigRequest} | {optimize: OptimizeRequest};
type NativeRequest = ProductRequest & {nativeHooks?: Awaited<ReturnType<typeof captureHooks>>; nativeSkills?: Awaited<ReturnType<typeof captureSkills>>};
/** One bounded, cancellable foreground child. No socket, listener or detached service. */
export async function queryLocal(request: ProductRequest, options: QueryOptions, config: CoreProcessOptions & CodexOptions): Promise<unknown> {
  let observed: NativeRequest = request;
  const product =
    "project" in request ? {action:"list",review:true,roots:request.project.roots,projectRoots:[request.project.projectRoot]} : "config" in request
      ? request.config
      : "optimize" in request
        ? request.optimize
          : undefined;
  const fresh =
    product &&
    product.action !== "capabilities" &&
    product.action !== "activity";
  const needsHooks = !('config' in request) || request.config.kind === 'hook' ||
    (!request.config.kind && (!request.config.kinds || request.config.kinds.includes('hook')));
  if (fresh && needsHooks) {
    const { captureHooks } = await import("./codex/hooks.js");
    observed = {
      ...request,
      nativeHooks: await captureHooks(
        { roots: product.roots, projectRoots: product.projectRoots },
        options,
        config,
      ),
    };
  }
  const skillRequest = "project" in request
    ? {roots:request.project.roots,projectRoots:[request.project.projectRoot]}
    : "config" in request && request.config.review &&
      (!request.config.kind || request.config.kind === "skill") &&
      (!request.config.kinds || request.config.kinds.includes("skill"))
      ? request.config
      : "optimize" in request && (!request.optimize.baseline || request.optimize.baseline.item.kind === 'skill')
        ? {roots:request.optimize.roots,projectRoots:request.optimize.projectRoots}
        : undefined;
  if (fresh && skillRequest) {
    const { captureSkills } = await import("./codex/skill-review.js");
    observed = { ...observed, nativeSkills: await captureSkills(skillRequest, options, config) };
  }
  return invokeOperation('local_query', {...observed}, options, config);
}
