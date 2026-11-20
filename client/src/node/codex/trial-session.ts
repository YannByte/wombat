import type { QueryOptions } from '../../client.js';
import { CoreError } from '../../errors.js';
import type { CodexInvocation, CodexOptions } from './process.js';
import { connectCodex } from './rpc.js';

type TrialSession = { workspace: string; profile: string; provider: string; model: string; developerInstructions?: string; instructionSources?: readonly string[]; disabledFeatures?: readonly string[]; outputFields?: readonly string[]; effort?: 'low' };
type TrialItem = { type: string; status?: string; exitCode?: number };
type TrialExecution = { items: TrialItem[]; truncated: boolean; finalText?: string };

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
  const allowed = new Set(expected.instructionSources ?? []);
  const loaded = result.instructionSources;
  if (loaded.length !== allowed.size || new Set(loaded).size !== loaded.length || loaded.some(path => typeof path !== 'string' || !allowed.has(path))) {
    throw new CoreError('TRIAL_SESSION_INSTRUCTIONS', 'Codex loaded instructions outside the trial inputs');
  }
}

function id(value: unknown): string {
  if (!record(value) || typeof value.id !== 'string' || value.id.length === 0 || value.id.length > 128)
    throw new CoreError('TRIAL_SESSION_PROTOCOL', 'Codex did not provide a valid trial identity');
  return value.id;
}

function turnFailure(value: unknown) {
  const info = record(value) ? value.codexErrorInfo : undefined;
  const kind = typeof info === 'string' ? info : record(info) ? Object.keys(info)[0] : undefined;
  const known = ['unauthorized', 'modelNotFound', 'usageLimitExceeded', 'rateLimitExceeded', 'badRequest', 'sandboxError', 'contextWindowExceeded', 'httpConnectionFailed', 'responseStreamDisconnected', 'responseStreamConnectionFailed', 'responseTooManyFailedAttempts', 'internalServerError'];
  const details = record(info) && kind && record(info[kind]) ? info[kind] : undefined;
  const httpStatusCode = record(details) ? details.httpStatusCode : undefined;
  return new CoreError('TRIAL_TURN_FAILED', 'Codex trial turn did not complete', {
    reason: kind && known.includes(kind) ? kind : 'unknown',
    ...(typeof httpStatusCode === 'number' && httpStatusCode >= 100 && httpStatusCode <= 599 ? { httpStatusCode } : {}),
  });
}

function mcpSettings(response: unknown) {
  const config = record(response) ? response.config : undefined;
  if (!record(config) || !record(config.mcp_servers))
    throw new CoreError('TRIAL_SESSION_TOOLS', 'Codex did not expose effective MCP settings');
  return { config, servers: config.mcp_servers };
}

export async function disabledTrialMcp(workspace: string, query: QueryOptions, options: CodexOptions, invocation: CodexInvocation) {
  const rpc = await connectCodex(query, options, invocation);
  try {
    const { servers } = mcpSettings(await rpc.request('config/read', { cwd: workspace, includeLayers: false }));
    const names = Object.keys(servers);
    if (names.length > 64 || names.some(name => !/^[a-zA-Z0-9_-]{1,96}$/.test(name)))
      throw new CoreError('TRIAL_SESSION_TOOLS', 'Codex MCP settings cannot be scoped safely');
    return names.map(name => `mcp_servers.${name}.enabled=false`);
  } finally {
    await rpc.close();
  }
}

export async function selectTrialModel(workspace: string, query: QueryOptions, options: CodexOptions, invocation: CodexInvocation, requested?: string) {
  const rpc = await connectCodex(query, options, invocation);
  try {
    const config = await rpc.request('config/read', { cwd: workspace, includeLayers: false });
    const listing = await rpc.request('model/list', { limit: 100, includeHidden: false });
    const models = record(listing) ? listing.data : undefined;
    if (!Array.isArray(models)) throw new CoreError('TRIAL_MODEL', 'Codex did not return the available model catalog');
    const available = new Set<string>();
    let defaultModel: string | undefined;
    for (const row of models) {
      if (!record(row) || typeof row.id !== 'string' || !/^[a-zA-Z0-9][a-zA-Z0-9._-]{0,95}$/.test(row.id)) continue;
      available.add(row.id);
      if (row.isDefault === true) defaultModel = row.id;
    }
    const userConfig = record(config) ? config.config : undefined;
    const configured = record(userConfig) ? userConfig.model : undefined;
    const model = requested ?? (typeof configured === 'string' && available.has(configured) ? configured : defaultModel);
    if (!model || !available.has(model)) throw new CoreError('TRIAL_MODEL', 'The selected model is unavailable for this Codex login');
    return model;
  } finally {
    await rpc.close();
  }
}

export async function runTrialSession(expected: TrialSession, query: QueryOptions, options: CodexOptions, invocation: CodexInvocation, prompt: string): Promise<TrialExecution> {
  let threadId: string | undefined;
  let complete!: (turn: unknown) => void;
  const completed = new Promise<unknown>(resolve => { complete = resolve; });
  const observed: { turnId: string; item: TrialItem }[] = [];
  let finalMessage: { turnId: string; text: string } | undefined;
  let truncated = false;
  const rpc = await connectCodex(query, options, invocation, (method, params) => {
    if (!threadId || !record(params) || params.threadId !== threadId) return;
    if (method === 'turn/completed') { complete(params.turn); return; }
    const item = params.item;
    if (typeof params.turnId !== 'string' || params.turnId.length > 128 || !record(item) ||
        typeof item.type !== 'string' || !/^[a-zA-Z]{1,32}$/.test(item.type)) return;
    if (expected.outputFields && item.type === 'agentMessage' && (item.phase === 'final_answer' || item.phase == null) &&
      typeof item.text === 'string' && Buffer.byteLength(item.text) <= 96 * 1024)
      finalMessage = { turnId: params.turnId, text: item.text };
    if (observed.length >= 128) { truncated = true; return; }
    observed.push({ turnId: params.turnId, item: {
      type: item.type,
      ...(typeof item.status === 'string' && /^[a-zA-Z]{1,32}$/.test(item.status) ? { status: item.status } : {}),
      ...(typeof item.exitCode === 'number' && Number.isSafeInteger(item.exitCode) ? { exitCode: item.exitCode } : {}),
    } });
  });
  try {
    if (expected.disabledFeatures) {
      const { config, servers } = mcpSettings(await rpc.request('config/read', { cwd: expected.workspace, includeLayers: false }));
      const features = config.features;
      if (Object.values(servers).some(server => !record(server) || server.enabled !== false) ||
        !record(features) || expected.disabledFeatures.some(feature => features[feature] !== false) || config.web_search !== 'disabled')
        throw new CoreError('TRIAL_SESSION_TOOLS', 'Codex has active or unverified external tools');
    }
    const result = await rpc.request('thread/start', {
      cwd: expected.workspace,
      runtimeWorkspaceRoots: [expected.workspace],
      permissions: expected.profile,
      model: expected.model,
      modelProvider: expected.provider,
      approvalPolicy: 'never',
      ephemeral: true,
      ...(expected.developerInstructions ? { developerInstructions: expected.developerInstructions } : {}),
    });
    validateTrialSession(result, expected);
    threadId = id(record(result) ? result.thread : undefined);
    let deadline: ReturnType<typeof setTimeout> | undefined;
    try {
      const timeout = new Promise<never>((_, reject) => {
        deadline = setTimeout(() => reject(new CoreError('CODEX_TIMEOUT', 'Codex trial turn timed out')), options.codexTimeoutMs ?? 20_000);
      });
      const run = async () => {
        const outputSchema = expected.outputFields && {
          type: 'object', properties: Object.fromEntries(expected.outputFields.map(key => [key, { type: 'string' }])),
          required: expected.outputFields, additionalProperties: false,
        };
        const started = await rpc.request('turn/start', { threadId, input: [{ type: 'text', text: prompt }],
          ...(outputSchema ? { outputSchema } : {}), ...(expected.effort ? { effort: expected.effort } : {}) });
        const turnId = id(record(started) ? started.turn : undefined);
        const turn = await completed;
        if (id(turn) !== turnId) throw new CoreError('TRIAL_SESSION_PROTOCOL', 'Codex completed an unexpected trial turn');
        if (!record(turn) || turn.status !== 'completed') throw turnFailure(record(turn) ? turn.error : undefined);
        return { items: observed.filter(entry => entry.turnId === turnId).map(entry => entry.item), truncated,
          ...(finalMessage?.turnId === turnId ? { finalText: finalMessage.text } : {}) };
      };
      return await Promise.race([run(), timeout, rpc.disconnectedWith.then(error => { throw error; })]);
    } finally {
      clearTimeout(deadline);
    }
  } finally {
    await rpc.close();
  }
}
