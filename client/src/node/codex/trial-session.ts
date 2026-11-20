import type { QueryOptions } from '../../client.js';
import { CoreError } from '../../errors.js';
import type { CodexInvocation, CodexOptions } from './process.js';
import { connectCodex } from './rpc.js';

type TrialSession = { workspace: string; profile: string; provider: string; model: string };

function record(value: unknown): value is Record<string, unknown> {
  return value !== null && typeof value === 'object' && !Array.isArray(value);
}

/** Session metadata preflight. Command isolation is verified separately. */
export function validateTrialSession(result: unknown, expected: TrialSession) {
  if (!record(result) || result.cwd !== expected.workspace || result.modelProvider !== expected.provider || result.model !== expected.model ||
      result.approvalPolicy !== 'never' || !record(result.activePermissionProfile) ||
      result.activePermissionProfile.id !== expected.profile || result.activePermissionProfile.extends !== null ||
      !Array.isArray(result.runtimeWorkspaceRoots) || result.runtimeWorkspaceRoots.length !== 1 ||
      result.runtimeWorkspaceRoots[0] !== expected.workspace || !Array.isArray(result.instructionSources)) {
    throw new CoreError('TRIAL_SESSION_SCOPE', 'Codex did not confirm the requested session boundaries');
  }
  if (result.instructionSources.length !== 0) {
    throw new CoreError('TRIAL_SESSION_INSTRUCTIONS', 'Codex loaded instructions outside the trial inputs');
  }
}

function id(value: unknown): string {
  if (!record(value) || typeof value.id !== 'string' || value.id.length === 0 || value.id.length > 128)
    throw new CoreError('TRIAL_SESSION_PROTOCOL', 'Codex did not provide a valid trial identity');
  return value.id;
}

export async function runTrialSession(expected: TrialSession, query: QueryOptions, options: CodexOptions, invocation: CodexInvocation, prompt: string) {
  let threadId: string | undefined;
  let complete!: (turn: unknown) => void;
  const completed = new Promise<unknown>(resolve => { complete = resolve; });
  const rpc = await connectCodex(query, options, invocation, params => {
    if (threadId && record(params) && params.threadId === threadId) complete(params.turn);
  });
  try {
    const result = await rpc.request('thread/start', {
      cwd: expected.workspace,
      runtimeWorkspaceRoots: [expected.workspace],
      permissions: expected.profile,
      model: expected.model,
      modelProvider: expected.provider,
      approvalPolicy: 'never',
      ephemeral: true,
    });
    validateTrialSession(result, expected);
    threadId = id(record(result) ? result.thread : undefined);
    let deadline: ReturnType<typeof setTimeout> | undefined;
    try {
      const timeout = new Promise<never>((_, reject) => {
        deadline = setTimeout(() => reject(new CoreError('CODEX_TIMEOUT', 'Codex trial turn timed out')), options.codexTimeoutMs ?? 20_000);
      });
      const run = async () => {
        const started = await rpc.request('turn/start', { threadId, input: [{ type: 'text', text: prompt }] });
        const turnId = id(record(started) ? started.turn : undefined);
        const turn = await completed;
        if (id(turn) !== turnId) throw new CoreError('TRIAL_SESSION_PROTOCOL', 'Codex completed an unexpected trial turn');
        if (!record(turn) || turn.status !== 'completed') throw new CoreError('TRIAL_TURN_FAILED', 'Codex trial turn did not complete');
      };
      await Promise.race([run(), timeout, rpc.disconnectedWith.then(error => { throw error; })]);
    } finally {
      clearTimeout(deadline);
    }
  } finally {
    await rpc.close();
  }
}
