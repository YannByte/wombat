import assert from 'node:assert/strict';
import { link, mkdir, mkdtemp, rm, symlink, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { test } from 'node:test';
import { checkTrialProbeArtifact, verifyHarnessTrial } from '../src/node/codex/trial-probe.js';
import { runTrialSession, validateTrialSession } from '../src/node/codex/trial-session.js';

test('trial artifact verification requires bounded, exact, unlinked file content', async () => {
  const directory = await mkdtemp(path.join(tmpdir(), 'wombat-trial-artifact-'));
  const artifact = path.join(directory, 'result.txt');
  const external = path.join(directory, 'external');
  const rejected = () => assert.rejects(checkTrialProbeArtifact(directory, '7\n'), { code: 'TRIAL_PROBE_ARTIFACT' });
  try {
    await rejected();
    await writeFile(artifact, '7\n');
    await checkTrialProbeArtifact(directory, '7\n');
    await writeFile(artifact, '8\n');
    await rejected();
    await writeFile(artifact, '7\n' + 'x'.repeat(4096));
    await rejected();
    await rm(artifact);
    await mkdir(artifact);
    await rejected();
    await rm(artifact, { recursive: true });
    await writeFile(external, '7\n');
    await link(external, artifact);
    await rejected();
    await rm(artifact);
    if (process.platform !== 'win32') {
      await symlink(external, artifact);
      await rejected();
    }
  } finally {
    await rm(directory, { recursive: true, force: true });
  }
});

test('trial probe rejects an unsupported host, invalid binary path and prior cancellation', async () => {
  const signal = new AbortController().signal;
  if (process.platform !== 'darwin' || process.arch !== 'arm64') {
    await assert.rejects(verifyHarnessTrial(process.execPath, signal), { code: 'TRIAL_PROBE_PLATFORM' });
    return;
  }
  await assert.rejects(verifyHarnessTrial('codex', signal), { code: 'TRIAL_PROBE_BINARY' });
  await assert.rejects(verifyHarnessTrial(process.execPath, AbortSignal.abort()), { code: 'CANCELLED' });
});

test('trial session preflight rejects scope changes and undeclared instructions', () => {
  const workspace = path.join(tmpdir(), 'synthetic-workspace');
  const expected = { workspace, profile: 'trial', provider: 'fixture', model: 'fixture-model' };
  const accepted = {
    cwd: workspace,
    model: expected.model,
    modelProvider: expected.provider,
    approvalPolicy: 'never',
    activePermissionProfile: { id: expected.profile, extends: null },
    runtimeWorkspaceRoots: [workspace],
    instructionSources: [],
  };
  validateTrialSession(accepted, expected);
  for (const mismatch of [
    { cwd: tmpdir() },
    { model: 'other' },
    { modelProvider: 'other' },
    { approvalPolicy: 'on-request' },
    { activePermissionProfile: null },
    { activePermissionProfile: { id: expected.profile, extends: ':workspace' } },
    { runtimeWorkspaceRoots: [workspace, tmpdir()] },
    { instructionSources: undefined },
  ]) assert.throws(() => validateTrialSession({ ...accepted, ...mismatch }, expected), { code: 'TRIAL_SESSION_SCOPE' });
  assert.throws(() => validateTrialSession({ ...accepted, instructionSources: [path.join(tmpdir(), 'AGENTS.md')] }, expected), {
    code: 'TRIAL_SESSION_INSTRUCTIONS',
  });
});

test('a trial sends one turn to the validated thread and requires its completion', async () => {
  const workspace = await mkdtemp(path.join(tmpdir(), 'wombat-trial-thread-'));
  const codexBinaryPath = path.join(workspace, 'codex.cjs');
  const expected = { workspace, profile: 'trial', provider: 'fixture', model: 'fixture-model' };
  try {
    await writeFile(codexBinaryPath, `
      const readline = require('node:readline');
      const send = value => process.stdout.write(JSON.stringify(value) + '\\n');
      readline.createInterface({ input: process.stdin }).on('line', raw => {
        const request = JSON.parse(raw);
        const result = value => send({ id: request.id, result: value });
        if (request.method === 'initialize') result({});
        if (request.method === 'thread/start') result({
          thread: { id: 'trial-thread' }, cwd: request.params.cwd, runtimeWorkspaceRoots: request.params.runtimeWorkspaceRoots,
          model: request.params.model, modelProvider: request.params.modelProvider, approvalPolicy: request.params.approvalPolicy,
          activePermissionProfile: { id: request.params.permissions, extends: null }, instructionSources: [],
        });
        if (request.method !== 'turn/start') return;
        if (request.params.threadId !== 'trial-thread' || request.params.input[0].text !== 'synthetic task') process.exit(2);
        send({ method: 'turn/completed', params: { threadId: 'unrelated', turn: { id: 'other', status: 'completed' } } });
        if (process.env.MODE !== 'no-completion') send({ method: 'turn/completed', params: {
          threadId: 'trial-thread', turn: { id: process.env.MODE === 'other-turn' ? 'other' : 'one', status: process.env.MODE === 'failed' ? 'failed' : 'completed' },
        } });
        result({ turn: { id: 'one' } });
      });
    `);
    const run = (mode: string) => runTrialSession(expected, {}, { codexBinaryPath, codexTimeoutMs: 750 },
      { cwd: workspace, env: { ...process.env, MODE: mode } }, 'synthetic task');
    await run('ok');
    await assert.rejects(run('other-turn'), { code: 'TRIAL_SESSION_PROTOCOL' });
    await assert.rejects(run('failed'), { code: 'TRIAL_TURN_FAILED' });
    await assert.rejects(run('no-completion'), { code: 'CODEX_TIMEOUT' });
  } finally {
    await rm(workspace, { recursive: true, force: true });
  }
});
