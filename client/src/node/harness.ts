import type { Request } from '../generated/agent-request.js';
import { validate as validateHarnessResponse } from '../generated/validate-harness-response.js';
import { CoreError } from '../errors.js';
import type { QueryOptions } from '../client.js';
import { invokeOperation } from './core.js';
import {
  projectInstructionSources, runProjectHarnessTask, verifyProjectHarnessLoad,
} from './codex/task-trial.js';

type HarnessRequest = Extract<Request, { method: 'harness' }>['params'];

export async function runHarness(request: HarnessRequest, query: QueryOptions) {
  const call = async (operation: Record<string, unknown>, signal = query.signal) => {
    const value = await invokeOperation('harness_state', operation, { ...query, signal }, {
      timeoutMs: 15_000, maxResponseBytes: 1024 * 1024,
    });
    if (!validateHarnessResponse(value)) throw new CoreError('PROTOCOL_ERROR', 'Invalid Harness trial response');
    return value;
  };
  const options = { signal: query.signal ?? new AbortController().signal };
  if (request.action === 'plan') {
    if (process.platform !== 'darwin' || process.arch !== 'arm64')
      throw new CoreError('TRIAL_PROBE_PLATFORM', 'Harness trials require an Apple silicon Mac');
    return call({
      ...request,
      includeGlobalInstructions: request.includeGlobalInstructions ?? false,
      globalInstructionSources: await projectInstructionSources(),
    });
  }
  if (request.action === 'run') {
    const current = await call({ action: 'status', trialId: request.trialId });
    const globalSources = await projectInstructionSources();
    if (JSON.stringify(globalSources) !== JSON.stringify(current.plan.globalInstructionSources) ||
        (globalSources.length && !current.plan.includeGlobalInstructions))
      throw new CoreError('TRIAL_SESSION_INSTRUCTIONS', 'Review the global Codex instructions in the trial plan first');
    const { trialId, original, plan, directory } = await call({ action: 'begin', trialId: request.trialId });
    try {
      const result = await runProjectHarnessTask({ ...plan, original }, directory, { ...options, stageTimeoutMs: plan.turnTimeoutSeconds * 1000 });
      return await call({
        action: 'finish', trialId,
        outcome: {
          model: result.model, candidate: result.candidate, rationale: result.rationale, report: result.report,
          stages: result.results.map(({ stage, checks }) => ({ stage, checks })),
        },
      });
    } catch (error) {
      const code = error instanceof CoreError ? error.code : 'TRIAL_EXECUTION';
      // Settlement needs its own timeout even when the model request was cancelled.
      const details = error instanceof CoreError ? error.details : undefined;
      const stage = details && typeof details === 'object' && 'stage' in details && typeof details.stage === 'string' ? details.stage : undefined;
      const failed = await call({ action: 'fail', trialId, code, stage }, new AbortController().signal);
      if (options.signal.aborted) throw error;
      return failed;
    }
  }
  if (request.action === 'verify') {
    const state = await call({ action: 'status', trialId: request.trialId });
    if (state.phase !== 'applied' || !state.revision)
      throw new CoreError('TRIAL_CONFLICT', 'Apply a reviewed trial version before checking its load');
    await verifyProjectHarnessLoad(state.plan.project, state.plan.target, state.revision, options);
    return call({ action: 'loaded', trialId: request.trialId, revision: state.revision });
  }
  return call(request);
}
