/** Extract and exercise the exact GitHub Release archive for the current platform. */
import assert from 'node:assert/strict';
import {spawn, spawnSync, type ChildProcess} from 'node:child_process';
import {appendFileSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, writeFileSync} from 'node:fs';
import {once} from 'node:events';
import os from 'node:os';
import path from 'node:path';
import {setTimeout as delay} from 'node:timers/promises';
import {fileURLToPath} from 'node:url';
import {pathToFileURL} from 'node:url';
import {parseArgs} from 'node:util';
import {hashFile, inventory} from './artifact-files.ts';
import {releaseNodeVersion, type GitHubReleaseSet} from './github-release.ts';
import {nodeRuntimeBinary} from './native-platforms.ts';
import {repositorySlug} from './release-policy.ts';
import {toolCommand} from './run-tool.ts';
import {previousReleaseTag, readPublishedReleaseTags} from './release-history.ts';
import {updateInstalled} from '../cli/src/update-cli.ts';
import {prepareFirstInstall} from './first-install-state.ts';
import {installSourceArgs} from './release-installation.ts';

const {values} = parseArgs({args: process.argv.slice(2).filter(arg => arg !== '--'), options: {set: {type: 'string'}}});
assert(values.set, '--set is required');
const setFile = path.resolve(values.set);
const set: GitHubReleaseSet = JSON.parse(readFileSync(setFile, 'utf8'));
const packageMetadata = JSON.parse(readFileSync(path.resolve('package.json'), 'utf8'));
const repository = repositorySlug(typeof packageMetadata.repository === 'string'
  ? packageMetadata.repository : packageMetadata.repository.url);
assert.equal(set.format, 1);
assert.equal(set.assets.length, set.targets.length);
const target = process.platform + '-' + process.arch;
const asset = set.assets.find(item => item.target === target);
assert(asset, `Release set does not contain ${target}`);
for (const item of set.assets) {
  assert.equal(path.basename(item.archive), item.archive);
  const archive = path.join(path.dirname(setFile), item.archive);
  assert.equal(hashFile(archive), item.sha256, `Archive hash mismatch: ${item.archive}`);
}

const scratch = mkdtempSync(path.join(os.tmpdir(), 'wombat-github-release-'));
let service: ChildProcess | undefined;
const stage = (name: string) => console.log(`Release archive verification: ${name}`);
async function within<T>(work: Promise<T>, timeoutMs: number, label: string): Promise<T> {
  let timer: NodeJS.Timeout | undefined;
  try {
    return await Promise.race([work, new Promise<T>((_, reject) => {
      timer = setTimeout(() => reject(new Error(`${label} timed out after ${timeoutMs} ms`)), timeoutMs);
    })]);
  } finally {
    if (timer) clearTimeout(timer);
  }
}
function validatePublishedRelease(directory: string, tag: string, targetArchive: string): boolean {
  try {
    const previousSet = JSON.parse(readFileSync(path.join(directory, 'release-set.json'), 'utf8')) as GitHubReleaseSet;
    const previousAsset = previousSet.assets.find(item => item.target === target);
    if (previousSet.version !== tag.slice(1) || !previousAsset || previousAsset.archive !== targetArchive) return false;
    const archive = path.join(directory, targetArchive);
    return hashFile(archive) === previousAsset.sha256
      && inventory(directory).find(file => file.path === targetArchive)?.size === previousAsset.bytes
      && readFileSync(path.join(directory, 'SHA256SUMS'), 'utf8').split(/\r?\n/)
        .includes(`${previousAsset.sha256}  ${targetArchive}`);
  } catch { return false; }
}
function downloadPublishedRelease(tag: string, targetArchive: string): string {
  const cacheRoot = process.env.WOMBAT_RELEASE_CACHE
    ? path.resolve(process.env.WOMBAT_RELEASE_CACHE) : path.join(os.tmpdir(), 'wombat-release-cache');
  const directory = path.join(cacheRoot, tag, target);
  if (validatePublishedRelease(directory, tag, targetArchive)) {
    stage(`reuse verified ${tag} ${target} cache`);
    return pathToFileURL(directory).href;
  }
  rmSync(directory, {recursive: true, force: true}); mkdirSync(directory, {recursive: true});
  const result = spawnSync('gh', ['release', 'download', tag, '--repo', repository, '--dir', directory,
    '--pattern', 'release-set.json', '--pattern', 'SHA256SUMS', '--pattern', targetArchive],
  {encoding: 'utf8', timeout: 600_000, maxBuffer: 8 * 1024 * 1024});
  assert.ifError(result.error); assert.equal(result.status, 0, result.stderr + result.stdout);
  assert(validatePublishedRelease(directory, tag, targetArchive), `Downloaded ${tag} ${target} assets failed identity or checksum validation`);
  return pathToFileURL(directory).href;
}
const stopService = async () => {
  if (!service?.pid || service.exitCode !== null || service.signalCode !== null) return;
  const exited = once(service, 'exit');
  if (process.platform === 'win32') {
    const killed = spawnSync('taskkill', ['/PID', String(service.pid), '/T', '/F'], {encoding: 'utf8', windowsHide: true});
    assert.ifError(killed.error);
  } else assert.equal(service.kill('SIGKILL'), true, 'Failed to stop installed shared service');
  await within(exited, 10_000, 'Installed shared service shutdown');
};
try {
  stage('first-install preparation');
  const prefix = path.join(scratch, 'first-install-prefix');
  const dataHome = path.join(scratch, 'data');
  const preparation = prepareFirstInstall(scratch, [prefix, dataHome]);
  const installEnv: NodeJS.ProcessEnv = {
    ...process.env, WOMBAT_AUTO_PRICES: '0', WOMBAT_DATA_HOME: dataHome,
    CODEX_HOME: path.join(scratch, 'source'), NO_COLOR: '1',
  };
  delete installEnv.WOMBAT_CORE_BIN; delete installEnv.NODE_OPTIONS; delete installEnv.NODE_PATH;
  delete installEnv.WOMBAT_INSTALL_ROOT; delete installEnv.WOMBAT_ENDPOINT_FILE;
  stage('archive identity and extraction');
  const releaseDirectory = path.dirname(setFile);
  const unpackedAt = performance.now();
  const unpack = spawnSync(...toolCommand('tar', ['-xzf', asset.archive, '-C', scratch]), {cwd: releaseDirectory, encoding: 'utf8', timeout: 120_000});
  assert.ifError(unpack.error); assert.equal(unpack.status, 0, unpack.stderr);
  const installMs = Math.round(performance.now() - unpackedAt);
  const unpacked = path.join(scratch, 'wombat');
  const release = JSON.parse(readFileSync(path.join(unpacked, 'release.json'), 'utf8'));
  assert.deepEqual({version: release.version, source: release.source, sourceSha256: release.sourceSha256, target: release.target, runtime: release.runtime},
    {version: set.version, source: set.source, sourceSha256: set.sourceSha256, target, runtime: {name: 'node', version: releaseNodeVersion}});
  stage('candidate first installation');
  const installAt = performance.now();
  const firstInstall = process.platform === 'win32'
    ? spawnSync('powershell.exe', ['-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', path.resolve('scripts/install/install.ps1'),
      '-Prefix', prefix, '-NoModifyPath', ...installSourceArgs(set.version, pathToFileURL(releaseDirectory).href, false, true)],
      {env: installEnv, encoding: 'utf8', timeout: 300_000, maxBuffer: 8 * 1024 * 1024, windowsHide: true})
    : spawnSync('sh', [path.resolve('scripts/install/install.sh'), '--prefix', prefix, '--no-modify-path',
      ...installSourceArgs(set.version, pathToFileURL(releaseDirectory).href, false, false)],
      {env: installEnv, encoding: 'utf8', timeout: 300_000, maxBuffer: 8 * 1024 * 1024});
  assert.ifError(firstInstall.error); assert.equal(firstInstall.status, 0, firstInstall.stderr + firstInstall.stdout);
  const firstInstallMs = Math.round(performance.now() - installAt);
  const managedRoot = path.join(prefix, 'lib', 'wombat');
  assert(!existsSync(path.join(managedRoot, 'service-registration.json')), 'First-install fixture registered a native service');
  const currentId = readFileSync(path.join(managedRoot, 'current.txt'), 'utf8').trim();
  assert.deepEqual(readdirSync(path.join(managedRoot, 'versions')), [currentId], 'First install contains historical versions');
  const installed = path.join(managedRoot, 'versions', currentId);
  assert.deepEqual(JSON.parse(readFileSync(path.join(installed, 'release.json'), 'utf8')), release);
  const cli = path.join(installed, 'lib', 'wombat.js');
  const core = path.join(installed, 'lib', process.platform === 'win32' ? 'wombat-core.exe' : 'wombat-core');
  const runtime = path.join(installed, 'runtime', nodeRuntimeBinary(target));
  const launcher = path.join(prefix, 'bin', process.platform === 'win32' ? 'wombat.cmd' : 'wombat');
  assert(existsSync(cli)); assert(existsSync(core)); assert(existsSync(runtime)); assert(existsSync(launcher));

  stage('offline doctor and product queries');
  const env: NodeJS.ProcessEnv = {...installEnv, PATH: ''};
  const run = (program: string, args: string[], status = 0, runEnv = env) => {
    const result = spawnSync(program, args, {cwd: scratch, env: runEnv, encoding: 'utf8', timeout: 120_000, maxBuffer: 8 * 1024 * 1024,
      windowsVerbatimArguments: process.platform === 'win32' && /cmd\.exe$/i.test(program)});
    assert.ifError(result.error); assert.equal(result.status, status, result.stderr + result.stdout); return result.stdout;
  };
  const query = (args: string[], status = 0) => JSON.parse(run(runtime, [cli, ...args, '--json'], status));
  assert.equal(query(['--version']).version, set.version);
  const emptyPathDoctor = query(['doctor'], 1);
  assert.equal(emptyPathDoctor.checks.find((check: {id: string}) => check.id === 'path')?.status, 'fail');
  assert.equal(emptyPathDoctor.scannedSources, false); assert.equal(emptyPathDoctor.networkUsed, false);
  const doctor = JSON.parse(run(runtime, [cli, 'doctor', '--json'], 2, {...env, PATH: path.join(prefix, 'bin')}));
  assert.equal(doctor.action, 'doctor'); assert.equal(doctor.scannedSources, false); assert.equal(doctor.networkUsed, false);
  assert.equal(doctor.checks.find((check: {id: string}) => check.id === 'installation')?.status, 'pass');
  assert.equal(doctor.checks.find((check: {id: string}) => check.id === 'path')?.status, 'pass');
  assert.equal(doctor.checks.find((check: {id: string}) => check.id === 'core')?.status, 'pass');
  service = spawn(core, ['--serve-usage'], {env, windowsHide: true, stdio: 'ignore'});
  await once(service, 'spawn');
  await delay(150);
  assert.equal(service.exitCode, null, 'Installed shared service exited during startup');
  assert.equal(query(['usage', '--cached'], 1).error.code, 'NO_SNAPSHOT');
  const sessions = path.join(env.CODEX_HOME!, 'sessions'); mkdirSync(sessions, {recursive: true});
  const log = path.join(sessions, 'synthetic.jsonl');
  const row = (id: string) => JSON.stringify({timestamp: new Date().toISOString(), type: 'event_msg', payload: {type: 'token_usage_record', thread_id: 'thread', turn_id: 'turn', response_id: id, usage: {input_tokens: 100, cached_input_tokens: 0, output_tokens: 10, total_tokens: 110}}}) + '\n';
  writeFileSync(log, JSON.stringify({type: 'session_meta', payload: {id: 'thread'}}) + '\n' + JSON.stringify({type: 'turn_context', payload: {turn_id: 'turn', model: 'gpt-5.4', model_provider: 'openai'}}) + '\n' + row('one'));
  assert.equal(query(['usage', '--fresh']).summary.tokens.total, 110);
  const snapshot = query(['refresh']); appendFileSync(log, row('two'));
  assert.equal(query(['usage', '--fresh']).summary.tokens.total, 220);
  assert.equal(query(['usage', '--snapshot', snapshot.snapshotRef.snapshotId]).summary.tokens.total, 110);
  assert.equal(query(['threads', '--fresh']).summary.tokens.total, 220);

  if (process.platform === 'win32') {
    const command = path.join(process.env.SystemRoot ?? 'C:\\Windows', 'System32', 'cmd.exe');
    assert.equal(JSON.parse(run(command, ['/d', '/s', '/c', `""${launcher}" --version --json"`])).version, set.version);
  } else assert.equal(JSON.parse(run(launcher, ['--version', '--json'])).version, set.version);

  stage('previous-release download and clean install');
  const previousTag = previousReleaseTag(readPublishedReleaseTags(repository), set.version);
  assert(previousTag, `No previous public release tag is available for the ${set.version} upgrade test`);
  stage(`selected ${previousTag} as the preceding public release`);
  const previousArchive = `wombat-${target}.tar.gz`;
  const previousBaseUrl = downloadPublishedRelease(previousTag, previousArchive);
  const upgradePrefix = path.join(scratch, 'upgrade-prefix');
  const installer = process.platform === 'win32'
    ? spawnSync('powershell.exe', ['-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', path.resolve('scripts/install/install.ps1'),
      '-Prefix', upgradePrefix, '-NoModifyPath', ...installSourceArgs(previousTag, previousBaseUrl, false, true)],
      {cwd: path.resolve('.'), env: installEnv, encoding: 'utf8', timeout: 300_000, maxBuffer: 8 * 1024 * 1024, windowsHide: true})
    : spawnSync('sh', [path.resolve('scripts/install/install.sh'), '--prefix', upgradePrefix, '--no-modify-path',
      ...installSourceArgs(previousTag, previousBaseUrl, false, false)],
      {cwd: path.resolve('.'), env: installEnv, encoding: 'utf8', timeout: 300_000, maxBuffer: 8 * 1024 * 1024});
  assert.ifError(installer.error); assert.equal(installer.status, 0, installer.stderr + installer.stdout);
  const upgradeRoot = path.join(upgradePrefix, 'lib', 'wombat');
  assert(!existsSync(path.join(upgradeRoot, 'service-registration.json')), 'Previous-release fixture registered a native service');
  const previousId = readFileSync(path.join(upgradeRoot, 'current.txt'), 'utf8').trim();
  const previousEntry = path.join(upgradeRoot, 'versions', previousId, 'lib', 'wombat.js');
  stage('managed installation upgrade');
  const updated = await within(updateInstalled({entryFile: previousEntry, baseUrl: pathToFileURL(releaseDirectory).href}),
    180_000, 'Managed installation upgrade');
  assert.deepEqual({currentVersion: updated.currentVersion, availableVersion: updated.availableVersion, updateAvailable: updated.updateAvailable, updated: updated.updated},
    {currentVersion: previousTag.slice(1), availableVersion: set.version, updateAvailable: true, updated: true});
  assert.equal(updated.serviceRestart, undefined, 'Isolated upgrade unexpectedly restarted a native service');
  const upgradedId = readFileSync(path.join(upgradeRoot, 'current.txt'), 'utf8').trim();
  assert.notEqual(upgradedId, previousId); assert(existsSync(path.join(upgradeRoot, 'versions', previousId)));
  const upgradedLauncher = path.join(upgradePrefix, 'bin', process.platform === 'win32' ? 'wombat.cmd' : 'wombat');
  if (process.platform === 'win32') {
    const command = path.join(process.env.SystemRoot ?? 'C:\\Windows', 'System32', 'cmd.exe');
    assert.equal(JSON.parse(run(command, ['/d', '/s', '/c', `""${upgradedLauncher}" --version --json"`])).version, set.version);
  } else assert.equal(JSON.parse(run(upgradedLauncher, ['--version', '--json'])).version, set.version);

  stage('Web startup and browser flow');
  await stopService(); service = undefined;
  run(process.execPath, ['--test', fileURLToPath(new URL('../tests/e2e/web.test.ts', import.meta.url))], 0,
    {...env, WOMBAT_WEB_TEST_ENTRY: cli, WOMBAT_WEB_TEST_CORE: core, WOMBAT_WEB_TEST_NODE: runtime});
  console.log(JSON.stringify({version: set.version, source: set.source, target, runtime: run(runtime, ['--version']).trim(),
    archive: asset.archive, archiveBytes: asset.bytes, installedBytes: inventory(installed).reduce((total, file) => total + file.size, 0),
    extractMs: installMs, firstInstallMs, preparation, firstInstall: true, launcher: true, upgradedFrom: previousTag.slice(1), emptyAppPath: true, live: true, append: true, fixedSnapshot: true, web: true}));
} finally {
  await stopService();
  rmSync(scratch, {recursive: true, force: true, maxRetries: 20, retryDelay: 500});
}
