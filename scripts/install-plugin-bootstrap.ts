/** Shared installer stage. Codex owns plugin state; this never enables or trusts Hooks. */
import {lstatSync, readFileSync, mkdtempSync, rmSync, readdirSync, writeFileSync} from 'node:fs';
import path from 'node:path';
import os from 'node:os';
import {runBoundedCommand, stdoutFromLog} from './verify-e2e-helpers.ts';

const object = (value: unknown): value is Record<string, unknown> => !!value && typeof value === 'object' && !Array.isArray(value);
class PluginStageError extends Error {
  readonly code: string;
  readonly recovery: string;
  constructor(code: string, reason: string, recovery: string) {super(reason); this.code = code; this.recovery = recovery;}
}
const code = (value: unknown): string | undefined => typeof value === 'string' && /^[A-Z][A-Z0-9_]{0,63}$/.test(value) ? value : undefined;
const quote = (value: string): string => "'" + value.replaceAll("'", process.platform === 'win32' ? "''" : "'\\''") + "'";
function verificationProblem(setup: unknown, name: string, requirements: Record<string, unknown>): PluginStageError | undefined {
  const retry = `Run only the read-only check: wombat setup --project ${quote(process.cwd())} --json. Do not reinstall the runtime.`;
  const fail = (value: string, reason: string, recovery = retry) => new PluginStageError(value, reason, recovery);
  if (!object(setup) || setup.outputVersion !== 1 || !object(setup.discovery) || !Array.isArray(setup.discovery.instances) || !Array.isArray(setup.runtimeChecks)) return fail('SETUP_INVALID_RESPONSE', 'The verification response is invalid.');
  const discovery = setup.discovery;
  if (discovery.status !== 'available') {
    const failure = code(discovery.errorCode) ?? ({missing: 'PLUGIN_NOT_DISCOVERED', disabled: 'PLUGIN_DISABLED', ambiguous: 'PLUGIN_AMBIGUOUS'}[String(discovery.status)] ?? 'PLUGIN_DISCOVERY_UNAVAILABLE');
    const reasons: Record<string, string> = {
      LOCAL_ACCESS_DENIED: 'Local permissions prevented Codex plugin discovery. Run the check in a terminal allowed to access Codex.',
      CODEX_DISCONNECTED: 'Codex disconnected during plugin discovery.',
      CODEX_TIMEOUT: 'Codex plugin discovery timed out.',
      SETUP_TIMEOUT: 'Plugin discovery exceeded the setup deadline.',
      CANCELLED: 'Plugin discovery did not complete before cancellation or the setup deadline.',
      SKILL_PARSE_ERROR: 'Codex reported Skill parsing errors in this project.',
      PLUGIN_NOT_DISCOVERED: 'Codex did not discover the installed Wombat Skill in this project.',
      PLUGIN_DISABLED: 'The Wombat Skill is disabled in this project.',
      PLUGIN_AMBIGUOUS: 'More than one Wombat Skill is enabled in this project.',
    };
    const inspect = ['PLUGIN_DISABLED', 'PLUGIN_AMBIGUOUS', 'PLUGIN_NOT_DISCOVERED', 'SKILL_PARSE_ERROR'].includes(failure) ? 'Check this project\'s plugin settings and Skill errors in Codex first. ' : '';
    return fail(failure, reasons[failure] ?? 'Codex plugin discovery is unavailable.', inspect + retry);
  }
  const instances: unknown[] = Array.isArray(discovery.instances) ? discovery.instances.filter(i => object(i) && i.enabled === true) : [];
  const selected = instances.length === 1 && object(instances[0]) ? instances[0] : null;
  if (!selected || selected.name !== name + ':wombat' || typeof selected.path !== 'string' || !path.isAbsolute(selected.path)) return fail('PLUGIN_SELECTION_MISMATCH', 'Codex selected a different or ambiguous Wombat Skill.', 'Select one intended Wombat plugin in Codex. ' + retry);
  const matching = setup.runtimeChecks.filter(check => object(check) && check.path === selected.path);
  if (matching.length !== 1 || !object(matching[0])) {
    const timeout = Array.isArray(setup.errorCodes) && setup.errorCodes.some(c => ['SETUP_TIMEOUT', 'CANCELLED'].includes(String(c)));
    return fail(timeout ? 'SETUP_TIMEOUT' : 'PLUGIN_RUNTIME_CHECK_MISSING', timeout ? 'The setup deadline expired before runtime compatibility could be verified.' : 'No unique runtime check was returned for the discovered Wombat Skill.');
  }
  const check = matching[0];
  if (check.status !== 'compatible') {
    const repair = ['modified', 'incompatible', 'unmanaged'].includes(String(check.status)) ? 'Repair only the plugin with --plugin-only (PowerShell: -PluginOnly); keep any custom prefix. Do not reinstall the runtime.' : retry;
    return fail(code(check.errorCode) ?? ({modified: 'SKILL_RESOURCE_CHANGED', incompatible: 'SKILL_RUNTIME_MISMATCH', unmanaged: 'PLUGIN_UNMANAGED'}[String(check.status)] ?? 'PLUGIN_RUNTIME_UNAVAILABLE'), `The discovered Wombat Skill runtime check is ${['modified', 'incompatible', 'unmanaged', 'unavailable'].includes(String(check.status)) ? check.status : 'invalid'}.`, repair);
  }
  if (check.pluginVersion !== requirements.version) {
    const observed = typeof check.pluginVersion === 'string' && /^[0-9A-Za-z][0-9A-Za-z._-]{0,63}$/.test(check.pluginVersion) ? check.pluginVersion : 'unknown';
    return fail('PLUGIN_VERSION_MISMATCH', `Expected plugin ${requirements.version}; Codex discovered ${observed}.`, 'Refresh only the plugin with --plugin-only (PowerShell: -PluginOnly); keep any custom prefix. Do not reinstall the runtime.');
  }
  if (check.skillContentHash !== requirements.skillContentHash) return fail('PLUGIN_CONTENT_MISMATCH', 'The discovered Skill content differs from the installed runtime bundle.', 'Repair only the plugin with --plugin-only (PowerShell: -PluginOnly); keep any custom prefix. Do not reinstall the runtime.');
}
/** Keep only bounded verification facts, never native stdout, Hook commands or configuration. */
function verificationFacts(setup: unknown): Record<string, unknown> {
  if (!object(setup)) return {};
  const statuses = new Set(['available', 'unavailable', 'missing', 'disabled', 'ambiguous', 'compatible', 'modified', 'incompatible', 'unmanaged']);
  const clean = (value: unknown) => typeof value === 'string' && value.length <= 4096 && !/[\x00-\x1f\x7f]/.test(value) ? value : null;
  const discovery = object(setup.discovery) ? setup.discovery : {};
  return {
    discovery: {status: statuses.has(String(discovery.status)) ? discovery.status : null, errorCode: code(discovery.errorCode) ?? null, instances: Array.isArray(discovery.instances) ? discovery.instances.slice(0, 32).filter(object).map(i => ({name: clean(i.name), path: clean(i.path), enabled: i.enabled === true})) : []},
    runtimeVersion: clean(setup.runtimeVersion),
    runtimeChecks: Array.isArray(setup.runtimeChecks) ? setup.runtimeChecks.slice(0, 32).filter(object).map(c => ({path: clean(c.path), status: statuses.has(String(c.status)) ? c.status : null, pluginVersion: clean(c.pluginVersion), skillContentHash: typeof c.skillContentHash === 'string' && /^[a-f0-9]{64}$/.test(c.skillContentHash) ? c.skillContentHash : null, missingCapabilities: Array.isArray(c.missingCapabilities) ? c.missingCapabilities.slice(0, 32).map(clean).filter(Boolean) : [], errorCode: code(c.errorCode) ?? null})) : [],
    errorCodes: Array.isArray(setup.errorCodes) ? setup.errorCodes.slice(0, 32).map(code).filter(Boolean) : [],
  };
}
function catalog(directory: string): void {
  const file = path.join(directory, '.agents/plugins/marketplace.json'), info = lstatSync(file);
  if (!info.isFile() || info.isSymbolicLink() || info.size > 65536) throw new Error('Invalid bundled plugin marketplace.');
  const value: unknown = JSON.parse(readFileSync(file, 'utf8'));
  if (!object(value) || value.name !== 'wombat-local' || !Array.isArray(value.plugins)) throw new Error('Invalid bundled plugin marketplace.');
  const plugins = value.plugins;
  if (!['wombat', 'wombat-collection'].every(name => plugins.some((plugin: unknown) => object(plugin) && plugin.name === name && object(plugin.source) && plugin.source.source === 'local' && plugin.source.path === (name === 'wombat' ? './plugin' : './collection-plugin')))) throw new Error('Invalid bundled plugin marketplace.');
}
export async function installBundledPlugin(marketplace: string, options: {binary?: string; env?: NodeJS.ProcessEnv; timeoutMs?: number; signal?: AbortSignal} = {}): Promise<'wombat' | 'wombat-collection'> {
  const directory = path.resolve(marketplace);
  catalog(directory);
  const env = options.env ?? process.env, binary = options.binary ?? env.WOMBAT_CODEX_BIN ?? 'codex';
  const prefix = /\.[cm]?js$/.test(binary) ? [process.execPath, binary] : [binary];
  const temporary = mkdtempSync(path.join(os.tmpdir(), 'wombat-plugin-install-'));
  const controller = new AbortController(), abort = () => controller.abort();
  const signal = options.signal ? AbortSignal.any([options.signal, controller.signal]) : controller.signal;
  let stage = 'list', succeeded = false;
  let facts: Record<string, unknown> = {};
  process.once('SIGINT', abort); process.once('SIGTERM', abort);
  try {
    const run = async (nextStage: string, args: string[], executable=prefix, accepted=[0]) => {
      stage = nextStage;
      const log = path.join(temporary, stage + '.log');
      const result = await runBoundedCommand({command: [...executable, ...args], cwd: process.cwd(), env: {...env, WOMBAT_CODEX_BIN: binary}, logFile: log, timeoutMs: options.timeoutMs ?? 20000, maxBytes: 1024 * 1024, signal});
      if (result.exitCode === null || !accepted.includes(result.exitCode) || result.timedOut || result.outputLimit || result.interrupted || result.closeTimedOut || result.spawnError || result.logError) {
        const reason=result.spawnError?'executable unavailable':result.timedOut?'timeout':result.outputLimit?'output limit':result.interrupted?'cancelled':result.closeTimedOut?'cleanup incomplete':result.logError?'log write failed':'exit '+result.exitCode;
        const failure = result.spawnError ? ('code' in result.spawnError ? code(result.spawnError.code) : undefined) ?? 'EXECUTABLE_UNAVAILABLE' : result.timedOut ? 'TIMEOUT' : result.outputLimit ? 'OUTPUT_LIMIT' : result.interrupted ? 'CANCELLED' : result.closeTimedOut ? 'CLEANUP_INCOMPLETE' : result.logError ? 'LOG_WRITE_FAILED' : 'COMMAND_FAILED';
        const recovery = stage === 'verify' ? `Run only the read-only check: wombat setup --project ${quote(process.cwd())} --json. Do not reinstall the runtime.` : 'Retry only the plugin stage with --plugin-only (PowerShell: -PluginOnly); keep any custom prefix. Do not reinstall the runtime.';
        throw new PluginStageError(failure, `Codex plugin ${stage} failed (${reason}).`, recovery);
      }
      return stdoutFromLog(readFileSync(log, 'utf8'));
    };
    const installed: unknown = JSON.parse(await run('list', ['plugin', 'list', '--marketplace', 'wombat-local', '--json']));
    if (!object(installed) || !Array.isArray(installed.installed)) throw new Error('Unrecognized Codex plugin list. Wombat is installed.');
    const active = installed.installed.filter((entry: unknown) => object(entry) && entry.enabled === true && typeof entry.pluginId === 'string' && ['wombat@wombat-local', 'wombat-collection@wombat-local'].includes(entry.pluginId));
    if (active.length > 1) throw new Error('Multiple Wombat plugins are enabled. Choose one in Codex, then rerun the installer.');
    const name = active.some((entry: unknown) => object(entry) && entry.pluginId === 'wombat-collection@wombat-local') ? 'wombat-collection' : 'wombat';
    stage = 'requirements';
    const requirementsFile=path.join(directory,name==='wombat'?'plugin':'collection-plugin','wombat-runtime.json');
    const requirementsInfo=lstatSync(requirementsFile);
    if(!requirementsInfo.isFile()||requirementsInfo.isSymbolicLink()||requirementsInfo.size>65536)throw new Error('Invalid bundled runtime requirements.');
    const requirements:unknown=JSON.parse(readFileSync(requirementsFile,'utf8'));
    if(!object(requirements)||requirements.format!==1||typeof requirements.version!=='string'||!/^[0-9A-Za-z][0-9A-Za-z._-]{0,63}$/.test(requirements.version)||typeof requirements.skillContentHash!=='string'||!/^[a-f0-9]{64}$/.test(requirements.skillContentHash))throw new Error('Invalid bundled runtime requirements.');
    facts = {plugin: name, expected: {pluginVersion: requirements.version, skillContentHash: requirements.skillContentHash}};
    const marketplaces: unknown = JSON.parse(await run('catalogs', ['plugin', 'marketplace', 'list', '--json']));
    if (!object(marketplaces) || !Array.isArray(marketplaces.marketplaces)) throw new Error('Unrecognized Codex marketplace list. Wombat is installed.');
    const previous = marketplaces.marketplaces.find((entry: unknown) => object(entry) && entry.name === 'wombat-local');
    let previousSource: string | undefined;
    if (previous !== undefined) {
      if (!object(previous) || !object(previous.marketplaceSource) || previous.marketplaceSource.sourceType !== 'local' || typeof previous.marketplaceSource.source !== 'string') throw new Error('Existing wombat-local marketplace is not local. Choose its source in Codex before retrying.');
      if (path.resolve(previous.marketplaceSource.source) !== directory) {
        previousSource = previous.marketplaceSource.source;
        await run('unregister', ['plugin', 'marketplace', 'remove', 'wombat-local']);
      }
    }
    let registered = false;
    try {
      await run('marketplace', ['plugin', 'marketplace', 'add', directory]);
      registered = true;
      const result: unknown = JSON.parse(await run('install', ['plugin', 'add', name + '@wombat-local', '--json']));
      if (!object(result) || result.pluginId !== name + '@wombat-local' || typeof result.installedPath !== 'string' || !path.isAbsolute(result.installedPath)) throw new Error('Unrecognized Codex plugin installation result. Check the plugin in Codex.');
    } catch (error) {
      const failedStage = stage;
      if (previousSource && !signal.aborted) {
        try {
          if (registered) await run('restore-remove', ['plugin', 'marketplace', 'remove', 'wombat-local']);
          await run('restore', ['plugin', 'marketplace', 'add', previousSource]);
        } catch {
          throw new Error('Plugin installation and marketplace recovery failed. Wombat is installed; restore wombat-local in Codex and rerun the installer.');
        }
      }
      stage = failedStage;
      throw error;
    }
    // Installation and native discovery are separate stages. Verification never rolls
    // back a successful installation or treats unrelated Hook gaps as a query failure.
    let setup: unknown;
    const output = await run('verify',['setup','--project',process.cwd(),'--json'],[process.execPath,path.resolve(directory,'../wombat.js')],[0,2]);
    try {setup = JSON.parse(output);} catch {throw new PluginStageError('SETUP_INVALID_RESPONSE', 'The verification response is not valid JSON.', 'Run wombat setup --json to check the installed runtime. Do not reinstall the runtime.');}
    facts = {...facts, ...verificationFacts(setup)};
    const problem = verificationProblem(setup, name, requirements);
    if (problem) throw problem;
    succeeded = true;
    return name;
  } catch (error) {
    // Raw command output can contain unrelated configuration. Retain a private,
    // bounded diagnostic summary instead, including after the installer exits.
    for (const file of readdirSync(temporary)) rmSync(path.join(temporary, file), {force: true});
    const failure = error instanceof PluginStageError ? error : new PluginStageError('PLUGIN_INSTALL_FAILED', error instanceof SyntaxError ? 'Codex returned invalid JSON.' : error instanceof Error ? error.message : 'Plugin installation failed.', 'Check this stage before retrying only the plugin installation with --plugin-only (PowerShell: -PluginOnly).');
    const report = path.join(temporary, 'diagnostic.json');
    const diagnostic = {outputVersion: 1, stage, code: failure.code, project: process.cwd(), checkedAt: new Date().toISOString(), ...facts};
    let retained = false;
    try {writeFileSync(report, JSON.stringify(diagnostic, null, 2) + '\n', {mode: 0o600}); retained = true;} catch { /* The original failure remains actionable if diagnostics cannot be saved. */ }
    throw new Error(`Wombat is installed. Plugin ${stage} failed [${failure.code}]: ${failure.message} ${failure.recovery} Installed files and data are retained.${retained ? ' Diagnostic summary: ' + report : ' Diagnostic summary could not be saved.'}`);
  } finally {
    process.off('SIGINT', abort); process.off('SIGTERM', abort);
    if (succeeded) rmSync(temporary, {recursive: true, force: true});
  }
}
if (process.argv[1] === '-' || path.basename(process.argv[1] ?? '') === 'install-plugin-bootstrap.ts') {
  const marketplace = process.argv[2];
  if (!marketplace || process.argv.length !== 3) {console.error('A bundled plugin marketplace path is required.'); process.exitCode = 2;}
  else void installBundledPlugin(marketplace).then(name => console.log(`Installed and verified ${name} through Codex for ${process.cwd()}. Start a new Codex project conversation and ask $${name}:wombat about your usage. Hook trust is optional for log queries.`), error => {console.error(error instanceof Error ? error.message : 'Plugin installation failed. Wombat is installed.'); process.exitCode = 1;});
}
