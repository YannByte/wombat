/** Exercise the shipped Skill and structured trial entry on a disposable release project. */
import assert from 'node:assert/strict';
import { mkdir, readFile, readdir, realpath, rm, writeFile } from 'node:fs/promises';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { parseArgs } from 'node:util';
import type { AgentRequest } from '@wombat/client';
import { validate as validateHarness } from '../client/src/generated/validate-harness-response.js';
import { checkBuild } from './build-identity.ts';
import { assertExternalOutputDir, runBoundedCommand, stdoutFromLog } from './verify-e2e-helpers.ts';

const repo = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const { values } = parseArgs({
  args: process.argv.slice(2).filter(arg => arg !== '--'),
  options: {
    release: { type: 'string' }, 'output-dir': { type: 'string' }, 'codex-bin': { type: 'string' },
    'allow-global-instructions': { type: 'boolean', default: false },
  },
});
if (!values.release || !values['output-dir'] || !values['codex-bin'] || !path.isAbsolute(values['codex-bin']))
  throw new Error('Use --release DIRECTORY --output-dir ABSOLUTE_PATH --codex-bin ABSOLUTE_PATH');
if (process.platform !== 'darwin' || process.arch !== 'arm64') throw new Error('This acceptance requires an Apple silicon Mac');
checkBuild(repo);
const output = assertExternalOutputDir(repo, values['output-dir']);
await mkdir(output, { recursive: true, mode: 0o700 });
if ((await readdir(output)).length) throw new Error('Choose a fresh output directory');
const scratch = path.join(await realpath(output), 'fixture');
const project = path.join(scratch, 'project');
const prefix = path.join(scratch, 'install');
const codexHome = path.join(scratch, 'codex');
const launcher = path.join(prefix, 'bin', 'wombat');
await mkdir(project, { recursive: true, mode: 0o700 });
await mkdir(codexHome, { recursive: true, mode: 0o700 });
const target = path.join(project, 'AGENTS.md');
const original = [
  '# Local release workflow',
  'For a routine release, use /bin/sh ../inputs/prepare-release.sh to prepare the package.',
  'The old release policy intentionally skips the installation smoke test for routine releases. Do not run verify-install.sh under this policy.',
  'Write a short and honest release summary as deliverable.md.',
].join('\n') + '\n';
const prepare = "set -eu\nprintf 'release-prepared\\n' > release-ready.txt\n";
const verify = [
  'set -eu',
  '/bin/test "$(/bin/cat release-ready.txt)" = release-prepared',
  '/bin/mkdir -p installed',
  '/bin/cp release-ready.txt installed/build.txt',
  '/bin/test "$(/bin/cat installed/build.txt)" = release-prepared',
  "printf 'install-verified\\n' > installation-status.txt",
].join('\n') + '\n';
await writeFile(target, original);
await writeFile(path.join(project, 'prepare-release.sh'), prepare);
await writeFile(path.join(project, 'verify-install.sh'), verify);
const env = { ...process.env, WOMBAT_DATA_HOME: path.join(scratch, 'data'), WOMBAT_AUTO_PRICES: '0',
  PATH: [path.dirname(launcher), process.env.PATH].filter(Boolean).join(path.delimiter),
  WOMBAT_CODEX_BIN: values['codex-bin'], WOMBAT_CORE_BIN: '' };
const installEnv = { ...env, CODEX_HOME: codexHome };
const timeout = AbortSignal.timeout(34 * 60_000);
const report = { status: 'running', platform: process.platform + '-' + process.arch, events: [] as unknown[] };
const reportPath = path.join(output, 'installed-harness-report.json');
const save = () => writeFile(reportPath, JSON.stringify(report, null, 2) + '\n', { mode: 0o600 });
const pass = async (stage: string, evidence: unknown = {}) => {
  report.events.push({ stage, evidence });
  await save();
  console.log(JSON.stringify({ stage, status: 'passed' }));
};
const run = async (stage: string, command: string[], runEnv: NodeJS.ProcessEnv, input?: string, expected = 0) => {
  const logFile = path.join(output, stage + '.log');
  const inputFile = path.join(scratch, stage + '.input.json');
  const invocation = input === undefined ? command : ['/bin/sh', '-c', 'input=$1; shift; exec "$@" < "$input"', 'wombat-test', inputFile, ...command];
  if (input !== undefined) await writeFile(inputFile, input, { mode: 0o600, flag: 'wx' });
  const result = await runBoundedCommand({ command: invocation, cwd: project, env: runEnv, logFile,
    timeoutMs: stage === 'trial' ? 32 * 60_000 : 90_000, maxBytes: 2 * 1024 * 1024, signal: timeout });
  assert.equal(result.exitCode, expected, stage);
  assert.ok(!result.timedOut && !result.outputLimit && !result.interrupted && !result.closeTimedOut && !result.spawnError && !result.logError, stage);
  return stdoutFromLog(await readFile(logFile, 'utf8'));
};
const state = async (stage: string, params: Extract<AgentRequest, { method: 'harness' }>['params']) => {
  const value: unknown = JSON.parse(await run(stage, [launcher, 'system', 'call', '--json'], env, JSON.stringify({ method: 'harness', params })));
  assert.ok(validateHarness(value), stage);
  return value;
};
let failed = false;
try {
  await save();
  await run('install', ['/bin/sh', path.join(repo, 'scripts/install/install.sh'), '--prefix', prefix,
    '--base-url', pathToFileURL(path.resolve(values.release)).href, '--no-modify-path', '--plugin'], installEnv);
  const doctor = JSON.parse(await run('discover', [launcher, 'system', 'doctor', '--project', project, '--json'], installEnv));
  assert.equal(doctor.discovery.status, 'available');
  const skill = doctor.discovery.instances.find((instance: {name: string}) => instance.name === 'wombat:wombat');
  assert.ok(skill?.path.startsWith(codexHome + path.sep));
  assert.equal(doctor.runtimeChecks.find((entry: {path: string}) => entry.path === skill.path)?.status, 'compatible');
  assert.match(await readFile(path.join(path.dirname(skill.path), 'references', 'verify.md'), 'utf8'), /system api harness/);
  const api = JSON.parse(await run('schema', [launcher, 'system', 'api', 'harness', '--output-schema', '--json'], installEnv));
  assert.equal(api.selectedMethod, 'harness');
  assert.ok(api.inputSchema && api.outputSchema);
  await pass('installed-skill-and-runtime');
  const planned = await state('plan', {
    action: 'plan', project, files: ['prepare-release.sh', 'verify-install.sh'], includeGlobalInstructions: values['allow-global-instructions'],
    task: 'Prepare this project for a routine local release. Use /bin/sh ../inputs/prepare-release.sh. For the installation validation step, when project instructions require it, use /bin/sh ../inputs/verify-install.sh. Follow the active project workflow and write the final observed results to deliverable.md. Perform all actions only inside the trial and do not publish.',
    checks: [
      { kind: 'file', id: 'normal-release-path', path: 'release-ready.txt', equals: 'release-prepared\n' },
      { kind: 'command', id: 'installation-command', command: '/bin/sh ../inputs/verify-install.sh' },
      { kind: 'file', id: 'installation-output', path: 'installation-status.txt', equals: 'install-verified\n' },
    ],
  });
  assert.equal(planned.phase, 'planned');
  assert.equal(planned.plan.executions, 4);
  if (planned.plan.globalInstructionSources.length && !planned.plan.includeGlobalInstructions)
    throw new Error('Global instruction reuse requires explicit acceptance authorization');
  const { trialId } = planned;
  const result = await state('trial', { action: 'run', trialId });
  assert.equal(result.phase, 'complete');
  assert.ok(result.outcome);
  const { model, stages } = result.outcome;
  await pass('observed-four-runs', { model, stages, recommendation: result.recommendation });
  assert.deepEqual(stages.map(stage => stage.stage), ['baseline', 'general', 'domain', 'composed']);
  assert.deepEqual(stages[0].checks.map(check => check.passed), [true, false, false], 'reproduce the skipped installation step');
  assert.deepEqual(stages[3].checks.map(check => check.passed), [true, true, true], 'verify installation and preserve regular packaging');
  assert.equal(await readFile(target, 'utf8'), original);
  assert.deepEqual((await readdir(project)).sort(), ['AGENTS.md', 'prepare-release.sh', 'verify-install.sh']);
  assert.equal(result.recommendation, 'review_composed');
  const revision = result.revision;
  assert.ok(revision);
  const applied = await state('apply', { action: 'apply', trialId, revision });
  assert.equal(applied.phase, 'applied');
  assert.equal(await readFile(target, 'utf8'), result.outcome.candidate);
  const loaded = await state('load', { action: 'verify', trialId });
  assert.equal(loaded.loaded, true);
  await pass('applied-and-loaded', { revision });
  const extra = result.outcome.candidate + '\nLater user edit.\n';
  await writeFile(target, extra);
  const conflict = JSON.parse(await run('undo-conflict', [launcher, 'system', 'call', '--json'], env,
    JSON.stringify({ method: 'harness', params: { action: 'undo', trialId, revision } }), 1));
  assert.equal(conflict.error.code, 'TRIAL_CONFLICT');
  assert.equal(await readFile(target, 'utf8'), extra);
  await writeFile(target, result.outcome.candidate);
  const undone = await state('undo', { action: 'undo', trialId, revision });
  assert.equal(undone.phase, 'undone');
  assert.equal(await readFile(target, 'utf8'), original);
  assert.equal(await readFile(path.join(project, 'prepare-release.sh'), 'utf8'), prepare);
  assert.equal(await readFile(path.join(project, 'verify-install.sh'), 'utf8'), verify);
  await pass('undo-and-preserve-later-edits');
  report.status = 'passed';
} catch (error) {
  failed = true;
  report.status = 'failed';
  report.events.push({ failure: error instanceof Error ? error.message : String(error) });
  process.exitCode = 1;
} finally {
  if (!failed) await rm(scratch, { recursive: true, force: true, maxRetries: 3 });
  await save();
  console.log(JSON.stringify({ status: report.status, report: reportPath }));
}
