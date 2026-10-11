import assert from 'node:assert/strict';
import { existsSync, mkdirSync, mkdtempSync, readFileSync, realpathSync, renameSync, rmSync, writeFileSync } from 'node:fs';
import { createRequire } from 'node:module';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { checkBuild } from './build-identity.ts';
import { runBoundedCommand, type BoundedCommandResult } from './verify-e2e-helpers.ts';
import { toolCommand } from './run-tool.ts';
import {prepareFirstInstall} from './first-install-state.ts';
import {
  acceptanceInputIdentity, assertExternalOutputDir, currentArtifactIdentity, currentSourceIdentity, externalFileIdentity, externalPackageIdentity,
  isWithin, parseVerifyArgs, plannedStages, reusableStage, runFingerprint, type Scope,
} from './verify-e2e-helpers.ts';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const BUILD_TIMEOUT = 30 * 60_000;
const STAGE_TIMEOUT: Record<string, number> = { integration: 20 * 60_000, e2e: 30 * 60_000, browser: 12 * 60_000 };
const BUILD_LOG_BUDGET = 32 * 1024 * 1024;
const STAGE_LOG_BUDGET = 16 * 1024 * 1024;
const reportName = 'e2e-report.json';
const controller = new AbortController();
let cleanupSandbox: (() => void) | undefined;

interface StageResult {
  name: string;
  status: 'passed' | 'failed';
  command: string[];
  startedAt: string;
  completedAt: string;
  elapsedMs: number;
  timeoutMs: number;
  exitCode: number | null;
  logFile?: string;
  reused?: boolean;
  reusedFrom?: string;
  error?: string;
}
interface Report {
  format: 1;
  runnerVersion: 1;
  runId: string;
  status: 'running' | 'passed' | 'failed';
  scope: Scope;
  sourceSha256?: string;
  artifactSha256?: string;
  acceptanceInputsSha256?: string;
  runKey?: string;
  platform: string;
  startedAt: string;
  completedAt?: string;
  outputDir: string;
  parameters: { playwrightModule?: string; browserExecutable?: string };
  effectiveArtifacts: { cliEntry: string; coreBinary: string };
  browserInputs?: Awaited<ReturnType<typeof browserArtifact>>;
  preparation?: ReturnType<typeof prepareFirstInstall>;
  stages: StageResult[];
  failure?: { stage: string; message: string };
  boundaries: string[];
}

function timestamp() { return new Date().toISOString(); }
function describeError(error: unknown): string { return (error instanceof Error ? error.message : String(error)).slice(0, 4000); }
function writeReport(outputDir: string, report: Report) {
  const file = path.join(outputDir, reportName), temp = `${file}.${process.pid}.tmp`;
  writeFileSync(temp, JSON.stringify(report, null, 2) + '\n', { mode: 0o600 });
  renameSync(temp, file);
}
function progress(event: Record<string, unknown>) { process.stderr.write(JSON.stringify({ at: timestamp(), ...event }) + '\n'); }

function addStage(report: Report, result: StageResult, outputDir: string) {
  report.stages.push(result);
  writeReport(outputDir, report);
}

async function stage(report: Report, outputDir: string, name: string, command: string[], timeoutMs: number, maxBytes: number, env: NodeJS.ProcessEnv): Promise<boolean> {
  const started = timestamp(), start = performance.now();
  const logName = `stage-${name}-${report.runId}.log`;
  const logPath = path.join(outputDir, logName);
  progress({ event: 'stage-start', stage: name, timeoutMs, logFile: logName });
  let outcome: BoundedCommandResult;
  try { outcome = await runBoundedCommand({ command, cwd: root, env, logFile: logPath, timeoutMs, maxBytes, signal: controller.signal }); }
  catch (error) {
    const completedAt = timestamp(), message = describeError(error);
    addStage(report, { name, status: 'failed', command, startedAt: started, completedAt, elapsedMs: Math.round(performance.now() - start), timeoutMs, exitCode: null, logFile: logName, error: message }, outputDir);
    report.status = 'failed'; report.failure = { stage: name, message }; writeReport(outputDir, report);
    progress({ event: 'stage-failed', stage: name, error: message }); return false;
  }
  const message = outcome.timedOut ? `Timed out after ${timeoutMs} ms` : outcome.outputLimit ? `Stage exceeded ${maxBytes} bytes of log output` : outcome.interrupted ? 'Interrupted' : outcome.closeTimedOut ? 'Child did not close after process-tree termination' : outcome.logError ?? outcome.spawnError?.message;
  const passed = !message && outcome.exitCode === 0;
  const error = message ?? (passed ? undefined : `Command exited with status ${String(outcome.exitCode)}`);
  const completedAt = timestamp();
  addStage(report, { name, status: passed ? 'passed' : 'failed', command, startedAt: started, completedAt, elapsedMs: Math.round(performance.now() - start), timeoutMs, exitCode: outcome.exitCode, logFile: logName, ...(error ? { error } : {}) }, outputDir);
  progress({ event: passed ? 'stage-passed' : 'stage-failed', stage: name, elapsedMs: Math.round(performance.now() - start), exitCode: outcome.exitCode, ...(error ? { error } : {}) });
  if (!passed) { report.status = 'failed'; report.failure = { stage: name, message: error! }; writeReport(outputDir, report); }
  return passed;
}

function commandFor(name: string, options: ReturnType<typeof parseVerifyArgs>, root: string): string[] {
  if (name === 'build') { const [program, args] = toolCommand('corepack', ['pnpm', 'run', 'build']); return [program, ...args]; }
  if (name === 'integration') { const [program, args] = toolCommand('corepack', ['pnpm', 'run', 'test:integration']); return [program, ...args]; }
  if (name === 'e2e') { const [program, args] = toolCommand('corepack', ['pnpm', 'run', 'test:e2e']); return [program, ...args]; }
  if (name === 'browser') {
    const args = ['pnpm', 'run', 'verify:event-upgrade:browser', '--', '--playwright-module', path.resolve(root, options.playwrightModule!)];
    if (options.browserExecutable) args.push('--browser-executable', path.resolve(root, options.browserExecutable));
    const [program, childArgs] = toolCommand('corepack', args); return [program, ...childArgs];
  }
  throw new Error(`Unknown acceptance stage: ${name}`);
}

async function browserArtifact(options: ReturnType<typeof parseVerifyArgs>, root: string): Promise<{
  playwright?: { path: string; sha256: string };
  playwrightPackage?: { path: string; name: string; version: string; sha256: string };
  playwrightCore?: { path: string; name: string; version: string; sha256: string };
  browserExecutable?: { path: string; sha256: string } | { unresolved: true };
  runtimeIdentityUnresolved?: true;
}> {
  if (options.scope === 'api') return {};
  const module = externalFileIdentity(path.resolve(root, options.playwrightModule!));
  let playwrightPackage: Awaited<ReturnType<typeof externalPackageIdentity>> | undefined;
  let playwrightCore: Awaited<ReturnType<typeof externalPackageIdentity>> | undefined;
  let runtimeIdentityUnresolved: true | undefined;
  const findPackage = (entry: string, names: string[]) => {
    let directory = path.dirname(realpathSync(entry));
    for (;;) {
      const manifestPath = path.join(directory, 'package.json');
      if (existsSync(manifestPath)) {
        try {
          const manifest = JSON.parse(readFileSync(manifestPath, 'utf8')) as { name?: unknown };
          if (typeof manifest.name === 'string' && names.includes(manifest.name)) return directory;
        } catch { /* Continue to the parent; package identity remains unresolved. */ }
      }
      const parent = path.dirname(directory);
      if (parent === directory) return undefined;
      directory = parent;
    }
  };
  try {
    const modulePackageRoot = findPackage(module.path, ['playwright', 'playwright-core', '@playwright/test']);
    if (modulePackageRoot) playwrightPackage = externalPackageIdentity(modulePackageRoot);
    else runtimeIdentityUnresolved = true;
    const require = createRequire(module.path);
    const coreRoot = findPackage(require.resolve('playwright-core'), ['playwright-core']);
    if (coreRoot) playwrightCore = externalPackageIdentity(coreRoot);
    else runtimeIdentityUnresolved = true;
  } catch { runtimeIdentityUnresolved = true; }
  if (options.browserExecutable) return { playwright: module, ...(playwrightPackage ? { playwrightPackage } : {}), ...(playwrightCore ? { playwrightCore } : {}),
    browserExecutable: externalFileIdentity(path.resolve(root, options.browserExecutable)), ...(runtimeIdentityUnresolved ? { runtimeIdentityUnresolved } : {}) };
  try {
    const imported: unknown = await import(pathToFileURL(module.path).href);
    if (imported && typeof imported === 'object' && 'chromium' in imported && imported.chromium && typeof imported.chromium === 'object'
      && 'executablePath' in imported.chromium && typeof imported.chromium.executablePath === 'function') {
      const executable = (imported.chromium.executablePath as () => string)();
      if (executable && existsSync(executable)) return { playwright: module, ...(playwrightPackage ? { playwrightPackage } : {}), ...(playwrightCore ? { playwrightCore } : {}),
        browserExecutable: externalFileIdentity(executable), ...(runtimeIdentityUnresolved ? { runtimeIdentityUnresolved } : {}) };
    }
  } catch { /* The existing browser verifier reports module API/load errors at its stage. */ }
  return { playwright: module, ...(playwrightPackage ? { playwrightPackage } : {}), ...(playwrightCore ? { playwrightCore } : {}),
    browserExecutable: { unresolved: true }, runtimeIdentityUnresolved: true };
}

function loadPrevious(file: string): Report | undefined {
  if (!existsSync(file)) return undefined;
  try {
    const value: unknown = JSON.parse(readFileSync(file, 'utf8'));
    return value && typeof value === 'object' && !Array.isArray(value) ? value as Report : undefined;
  } catch { return undefined; }
}

async function main(): Promise<number> {
  const options = parseVerifyArgs(process.argv.slice(2));
  const outputDir = assertExternalOutputDir(root, options.outputDir);
  const realRoot = realpathSync(root);
  mkdirSync(outputDir, { recursive: true, mode: 0o700 });
  const realOutput = realpathSync(outputDir);
  assert(!isWithin(realRoot, realOutput), '--output-dir must resolve outside the repository');
  const previous = options.resume ? loadPrevious(path.join(realOutput, reportName)) : undefined;
  const runId = `${new Date().toISOString().replaceAll(':', '-')}-${process.pid}`;
  const report: Report = { format: 1, runnerVersion: 1, runId, status: 'running', scope: options.scope, platform: `${process.platform}-${process.arch}`,
    startedAt: timestamp(), outputDir: realOutput, parameters: { ...(options.playwrightModule ? { playwrightModule: path.resolve(root, options.playwrightModule) } : {}), ...(options.browserExecutable ? { browserExecutable: path.resolve(root, options.browserExecutable) } : {}) },
    effectiveArtifacts: { cliEntry: path.join(root, 'dist/wombat.js'), coreBinary: path.join(root, 'dist', process.platform === 'win32' ? 'wombat-core.exe' : 'wombat-core') },
    stages: [], boundaries: ['Integration and E2E suites use isolated source and data homes plus synthetic fixtures; this runner does not inspect real Agent logs.', 'Browser acceptance uses the existing synthetic event-upgrade verifier; no screenshots are produced.', 'A passing run covers only the selected scope and current platform; it does not replace human review of claims, privacy, or untested platforms.'] };
  const sandbox = realpathSync(mkdtempSync(path.join(os.tmpdir(), 'wombat-verify-e2e-')));
  cleanupSandbox = () => rmSync(sandbox, { recursive: true, force: true });
  const preparation = prepareFirstInstall(sandbox, [path.join(sandbox, 'data')]);
  report.preparation = preparation;
  progress({event: 'first-install-prepared', ...preparation});
  mkdirSync(path.join(sandbox, 'source'), { recursive: true }); mkdirSync(path.join(sandbox, 'data'), { recursive: true });
  const env: NodeJS.ProcessEnv = { ...process.env, CODEX_HOME: path.join(sandbox, 'source'), WOMBAT_DATA_HOME: path.join(sandbox, 'data'), WOMBAT_AUTO_PRICES: '0',
    WOMBAT_CORE_BIN: path.join(root, 'dist', process.platform === 'win32' ? 'wombat-core.exe' : 'wombat-core'), WOMBAT_CLI_ENTRY: path.join(root, 'dist', 'wombat.js') };
  delete env.WOMBAT_CODEX_BIN;
  let activeStage = 'preflight';
  const writeInitial = () => writeReport(realOutput, report);
  writeInitial();
  try {
    activeStage = 'build-validation';
    let sourceBefore = currentSourceIdentity(root);
    const acceptanceInputsSha256 = acceptanceInputIdentity(root);
    report.sourceSha256 = sourceBefore; report.acceptanceInputsSha256 = acceptanceInputsSha256;
    let buildValid = false, artifactSha256: string | undefined;
    try { checkBuild(root); artifactSha256 = currentArtifactIdentity(root); buildValid = true; } catch { buildValid = false; }
    if (!buildValid) {
      activeStage = 'build';
      const buildCommand = commandFor('build', options, root);
      if (!await stage(report, realOutput, 'build', buildCommand, BUILD_TIMEOUT, BUILD_LOG_BUDGET, env)) return finish(report, realOutput);
      activeStage = 'build-validation';
      if (currentSourceIdentity(root) !== sourceBefore) throw new Error('Source identity changed during pnpm build; rebuild from the current source before acceptance');
      checkBuild(root);
    } else {
      progress({ event: 'build-verified', sourceSha256: sourceBefore });
    }
    artifactSha256 ??= currentArtifactIdentity(root);
    if (currentSourceIdentity(root) !== sourceBefore) throw new Error('Source identity changed after build validation');
    report.sourceSha256 = sourceBefore; report.artifactSha256 = artifactSha256;
    activeStage = 'external-browser-inputs';
    const browser = await browserArtifact(options, root);
    report.browserInputs = browser;
    const runKey = runFingerprint({ sourceSha256: sourceBefore, artifactSha256, acceptanceInputsSha256, scope: options.scope, ...browser });
    report.runKey = runKey;
    const old = previous?.runKey === runKey ? previous : undefined;
    if (buildValid) report.stages.unshift({ name: 'build', status: 'passed', command: ['checkBuild'], startedAt: report.startedAt, completedAt: timestamp(), elapsedMs: 0, timeoutMs: BUILD_TIMEOUT, exitCode: 0, reused: true, reusedFrom: 'existing verified artifact' });
    writeInitial();
    const verifyCoreInputs = (where: string) => {
      if (currentSourceIdentity(root) === sourceBefore && currentArtifactIdentity(root) === artifactSha256
        && acceptanceInputIdentity(root) === acceptanceInputsSha256) return;
      report.runKey = undefined;
      report.status = 'failed';
      report.failure = { stage: `identity-${where}`, message: 'Source, acceptance tests, or build artifact changed during acceptance' };
      writeInitial();
      throw new Error(report.failure.message);
    };
    const verifyBrowserInputs = async (where: string) => {
      if (JSON.stringify(await browserArtifact(options, root)) === JSON.stringify(browser)) return;
      report.runKey = undefined;
      report.status = 'failed';
      report.failure = { stage: `browser-identity-${where}`, message: 'Playwright or browser executable identity changed during acceptance' };
      writeInitial();
      throw new Error(report.failure.message);
    };
    for (const name of plannedStages(options.scope)) {
      activeStage = name;
      verifyCoreInputs(`before-${name}`);
      if (name === 'browser') await verifyBrowserInputs('before');
      const canReuse = options.resume && old && !(name === 'browser' && (browser.runtimeIdentityUnresolved || (browser.browserExecutable && 'unresolved' in browser.browserExecutable)))
        && reusableStage(old, name, runKey);
      if (canReuse) {
        const prior = old!.stages.find(item => item.name === name && item.status === 'passed')!;
        report.stages.push({ ...prior, reused: true, reusedFrom: old!.runId }); writeInitial();
        progress({ event: 'stage-reused', stage: name, sourceSha256: sourceBefore, artifactSha256 });
        verifyCoreInputs(`after-${name}`);
        if (name === 'browser') await verifyBrowserInputs('after');
        continue;
      }
      const command = commandFor(name, options, root);
      if (!await stage(report, realOutput, name, command, STAGE_TIMEOUT[name], STAGE_LOG_BUDGET, env)) return finish(report, realOutput);
      verifyCoreInputs(`after-${name}`);
      if (name === 'browser') await verifyBrowserInputs('after');
    }
    activeStage = 'final-identity-validation';
    const finalSource = currentSourceIdentity(root), finalArtifact = currentArtifactIdentity(root);
    if (finalSource !== sourceBefore || finalArtifact !== artifactSha256 || acceptanceInputIdentity(root) !== acceptanceInputsSha256) {
      report.runKey = undefined;
      throw new Error('Source, acceptance tests, or build artifact changed during acceptance; results are not reusable');
    }
    const finalBrowser = await browserArtifact(options, root);
    if (JSON.stringify(finalBrowser) !== JSON.stringify(browser)) {
      report.runKey = undefined;
      throw new Error('Playwright or browser executable identity changed during acceptance');
    }
    checkBuild(root);
    report.status = 'passed';
    return finish(report, realOutput);
  } catch (error) {
    report.runKey = undefined;
    report.status = 'failed'; report.failure ??= { stage: activeStage, message: describeError(error) };
    return finish(report, realOutput);
  }
}

function finish(report: Report, outputDir: string): number {
  report.completedAt = timestamp();
  writeReport(outputDir, report);
  process.stdout.write(JSON.stringify(report, null, 2) + '\n');
  return report.status === 'passed' ? 0 : 1;
}

const interrupted = () => {
  controller.abort();
};
process.once('SIGINT', interrupted); process.once('SIGTERM', interrupted);
let exit = 1;
try { exit = await main(); }
catch (error) {
  const message = describeError(error);
  process.stderr.write(`${message}\n`);
  process.stdout.write(JSON.stringify({ format: 1, runnerVersion: 1, status: 'failed', stage: 'preflight', error: message }) + '\n');
}
finally { cleanupSandbox?.(); }
process.removeListener('SIGINT', interrupted); process.removeListener('SIGTERM', interrupted);
process.exitCode = exit;
