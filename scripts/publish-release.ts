import { spawnSync } from 'node:child_process';
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

import { releaseArchive } from './github-release.ts';
import { currentNativeTarget, nativeTargets, type NativeTarget } from './native-platforms.ts';
import { releaseVersion, validateVersion } from './release-version.ts';
import { consistencyErrors } from './prepare-release.ts';
import { readReleaseNotesInput } from './generate-release-notes.ts';
import { readCiCandidate } from './ci-release-candidate.ts';
import { verifyReleaseAssets } from './check-release-assets.ts';
import { cacheReleaseAssets, releaseCacheDirectory, type PublishedAsset } from './release-cache.ts';
import { repositorySlug } from './release-policy.ts';
import { readRemoteRef } from './github-release-ref.ts';
import { toolCommand } from './run-tool.ts';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const apiVersion = '2026-03-10';
const managedReleaseFiles = new Set([
  'docs/i18n/records/README.i18n.json', 'README.md', 'README.zh-CN.md',
  'cli/package.json', 'client/package.json', 'core/Cargo.lock', 'core/Cargo.toml',
  'docs/dependency-licenses.json', 'docs/guides/installation.en.md',
  'docs/guides/installation.i18n.json', 'docs/guides/installation.md',
  'scripts/install/install.sh', 'scripts/install/install.ps1', 'package.json', 'ui/package.json', 'web/package.json',
  'scripts/release-notes/current.json',
]);

export interface PublishOptions {
  version: string;
  branch: string;
  repository?: string;
}

export interface WorkflowRun {
  conclusion: string;
  databaseId: number;
  event: string;
  headBranch: string;
  headSha: string;
  status: string;
  url: string;
}

export interface ReleaseView {
  assets: PublishedAsset[];
  isDraft: boolean;
  isImmutable: boolean;
  isPrerelease: boolean;
  tagName: string;
  url: string;
}

function usage(): never {
  throw new Error('Usage: corepack pnpm release:publish [-- --version <root-version> --branch main --repo owner/repo --status | --verify-published [--hosted]]');
}

export function parsePublishArgs(argv: string[], canonicalVersion = releaseVersion(root)): PublishOptions & {status: boolean; verifyPublished: boolean; hosted: boolean} {
  const args = argv.filter(arg => arg !== '--');
  let version = '';
  let branch = 'main';
  let repository: string | undefined;
  let status = false;
  let verifyPublished = false;
  let hosted = false;
  const seen = new Set<string>();
  for (let index = 0; index < args.length; index++) {
    const arg = args[index];
    if (seen.has(arg)) throw new Error(`Duplicate release option: ${arg}`);
    seen.add(arg);
    if (arg === '--status') { status = true; continue; }
    if (arg === '--verify-published') { verifyPublished = true; continue; }
    if (arg === '--hosted') { hosted = true; continue; }
    if (arg !== '--version' && arg !== '--branch' && arg !== '--repo') usage();
    const value = args[++index];
    if (!value || value.startsWith('-')) usage();
    if (arg === '--version') version = value;
    else if (arg === '--branch') branch = value;
    else repository = value;
  }
  version ||= canonicalVersion;
  validateVersion(version);
  if (version !== canonicalVersion) throw new Error(`Requested version ${version} differs from package.json ${canonicalVersion}; run release:prepare -- --version ${version} first.`);
  if (!/^[A-Za-z0-9._/-]+$/.test(branch) || branch.startsWith('/') || branch.endsWith('/')) {
    throw new Error(`Invalid release branch: ${branch}`);
  }
  if (repository && !/^[A-Za-z0-9_.-]+\/[A-Za-z0-9_.-]+$/.test(repository)) {
    throw new Error(`Invalid GitHub repository: ${repository}`);
  }
  if (status && verifyPublished) throw new Error('Choose status inspection or published verification');
  if (hosted && !verifyPublished) throw new Error('Hosted execution requires --verify-published');
  return { version, branch, repository, status, verifyPublished, hosted };
}

export function expectedReleaseAssets(): string[] {
  return [
    ...nativeTargets.map(releaseArchive),
    'SHA256SUMS', 'release-set.json', 'install.sh', 'install.ps1',
  ].sort();
}

export function selectWorkflowRun(
  runs: WorkflowRun[], source: string, headBranch: string, event = 'push', afterRunId = 0,
): WorkflowRun | undefined {
  return runs.filter(run => run.event === event && run.headSha === source && run.headBranch === headBranch && run.databaseId > afterRunId)
    .sort((a, b) => b.databaseId - a.databaseId)[0];
}

export function assertHostedVerificationJobs(evidence: unknown): void {
  if (!record(evidence) || !Array.isArray(evidence.jobs) || evidence.total_count !== evidence.jobs.length) throw new Error('Hosted verification job evidence is incomplete');
  const jobs = evidence.jobs.filter(job => record(job) && job.name === 'Verify public installation and update');
  if (jobs.length !== 1 || !record(jobs[0]) || jobs[0].status !== 'completed' || jobs[0].conclusion !== 'success') throw new Error('The public installation and update job did not succeed');
}

export function releaseRecoveryState(
  head: string, tagSource: string | undefined, release: ReleaseView | undefined, run: WorkflowRun | undefined,
): 'prepare' | 'wait' | 'verify' {
  if (!tagSource) {
    if (release) throw new Error('GitHub Release exists without its remote tag; inspect the remote identity before continuing.');
    return 'prepare';
  }
  if (head !== tagSource) throw new Error('The existing tag differs from HEAD; prepare a new version after fixes. Public tags cannot move.');
  if (release && !release.isDraft) return 'verify';
  if (run?.status === 'completed' && run.conclusion !== 'success') {
    throw new Error(`Tagged workflow failed or was cancelled: ${run.url}. Analyze the failure and prepare a new version; do not retag or publish a partial draft.`);
  }
  if (run?.status === 'completed') throw new Error('Tagged workflow succeeded but no complete published Release exists; inspect the publication stage.');
  return 'wait';
}

export function publishedReleaseErrors(view: ReleaseView, version: string): string[] {
  const tag = `v${version}`;
  const errors: string[] = [];
  if (view.tagName !== tag) errors.push(`Release tag is ${view.tagName}, expected ${tag}`);
  if (view.isDraft) errors.push('Release is still a draft');
  if (!view.isImmutable) errors.push('Release is not immutable');
  if (view.isPrerelease !== version.includes('-')) errors.push('Release prerelease state does not match the version');
  const actual = view.assets.map(asset => asset.name).sort();
  const expected = expectedReleaseAssets();
  if (JSON.stringify(actual) !== JSON.stringify(expected)) {
    errors.push(`Release assets differ: expected ${expected.join(', ')}, found ${actual.join(', ')}`);
  }
  for (const asset of view.assets) if (!Number.isSafeInteger(asset.size) || asset.size <= 0) {
    errors.push(`Release asset has an invalid size: ${asset.name}`);
  }
  for (const asset of view.assets) if (!/^sha256:[0-9a-f]{64}$/.test(asset.digest)) errors.push(`Release asset SHA-256 is missing: ${asset.name}`);
  return errors;
}

function stage(name: string): void {
  console.log(`\n=== ${name} ===`);
}

function command(program: string, args: string[], timeout = 60_000, inherit = false) {
  return spawnSync(...toolCommand(program, args), {
    cwd: root,
    encoding: inherit ? undefined : 'utf8',
    stdio: inherit ? 'inherit' : 'pipe',
    timeout,
    maxBuffer: 32 * 1024 * 1024,
  });
}

function failure(program: string, args: string[], result: ReturnType<typeof command>): string {
  const stderr = typeof result.stderr === 'string' ? result.stderr.trim() : '';
  return result.error?.message || stderr || `exit ${String(result.status)}`;
}

function required(program: string, args: string[], timeout?: number): string {
  const result = command(program, args, timeout);
  if (result.error || result.status !== 0) {
    throw new Error(`${program} ${args.join(' ')} failed: ${failure(program, args, result)}`);
  }
  return String(result.stdout ?? '').trim();
}

function run(program: string, args: string[], timeout = 60_000): void {
  console.log(`+ ${program} ${args.join(' ')}`);
  const result = command(program, args, timeout, true);
  if (result.error || result.status !== 0) {
    throw new Error(`${program} ${args.join(' ')} failed: ${failure(program, args, result)}`);
  }
}

function parsedJson(program: string, args: string[], timeout?: number): unknown {
  const output = required(program, args, timeout);
  try {
    return JSON.parse(output);
  } catch {
    throw new Error(`${program} returned invalid JSON for ${args.join(' ')}`);
  }
}

function record(value: unknown): value is Record<string, unknown> {
  return typeof value === 'object' && value !== null && !Array.isArray(value);
}

function packageMetadata(): { version: string; repository: string } {
  const value: unknown = JSON.parse(readFileSync(path.join(root, 'package.json'), 'utf8'));
  if (!record(value) || typeof value.version !== 'string') throw new Error('package.json version is missing');
  const repository = typeof value.repository === 'string' ? value.repository
    : record(value.repository) && typeof value.repository.url === 'string' ? value.repository.url : undefined;
  if (!repository) throw new Error('package.json repository URL is missing');
  return { version: value.version, repository };
}

function changedFiles(): string[] {
  const values = [
    required('git', ['diff', '--name-only', '-z']),
    required('git', ['diff', '--cached', '--name-only', '-z']),
    required('git', ['ls-files', '--others', '--exclude-standard', '-z']),
  ];
  return [...new Set(values.flatMap(value => value.split('\0')).filter(Boolean))].sort();
}

function assertBranch(branch: string): void {
  const current = required('git', ['symbolic-ref', '--quiet', '--short', 'HEAD']);
  if (current !== branch) throw new Error(`Release must run from ${branch}; current branch is ${current}`);
}

function workflowRuns(repository: string, workflow: string, source: string): WorkflowRun[] {
  const value = parsedJson('gh', [
    'run', 'list', '--repo', repository, '--workflow', workflow, '--commit', source, '--limit', '20',
    '--json', 'conclusion,databaseId,event,headBranch,headSha,status,url',
  ]);
  if (!Array.isArray(value)) throw new Error(`gh run list returned an invalid list for ${workflow}`);
  return value.map((item, index) => {
    if (!record(item) || typeof item.conclusion !== 'string' || typeof item.databaseId !== 'number'
      || typeof item.event !== 'string' || typeof item.headBranch !== 'string'
      || typeof item.headSha !== 'string' || typeof item.status !== 'string' || typeof item.url !== 'string') {
      throw new Error(`gh run list returned an invalid ${workflow} run at index ${index}`);
    }
    return item as unknown as WorkflowRun;
  });
}

function waitForRun(repository: string, workflow: string, source: string, headBranch: string, event = 'push', afterRunId = 0): WorkflowRun {
  const deadline = Date.now() + 180_000;
  let selected: WorkflowRun | undefined;
  while (Date.now() < deadline) {
    selected = selectWorkflowRun(workflowRuns(repository, workflow, source), source, headBranch, event, afterRunId);
    if (selected) break;
    Atomics.wait(new Int32Array(new SharedArrayBuffer(4)), 0, 0, 3_000);
  }
  if (!selected) throw new Error(`No ${workflow} ${event} run appeared for ${source} on ${headBranch}`);
  if (selected.status !== 'completed') {
    run('gh', ['run', 'watch', String(selected.databaseId), '--repo', repository, '--compact', '--exit-status', '--interval', '30'], 90 * 60_000);
    selected = selectWorkflowRun(workflowRuns(repository, workflow, source), source, headBranch, event, afterRunId);
  }
  if (!selected || selected.status !== 'completed' || selected.conclusion !== 'success') {
    throw new Error(`${workflow} did not complete successfully: ${selected?.url ?? 'unknown run'}`);
  }
  console.log(`${workflow}: ${selected.url}`);
  return selected;
}

function immutableReleasesEnabled(repository: string): void {
  const value = parsedJson('gh', [
    'api', '-H', 'Accept: application/vnd.github+json', '-H', `X-GitHub-Api-Version: ${apiVersion}`,
    `repos/${repository}/immutable-releases`,
  ]);
  if (!record(value) || value.enabled !== true) throw new Error('GitHub immutable Releases must be enabled before tagging');
}

function releaseView(repository: string, tag: string): ReleaseView {
  const value = parsedJson('gh', [
    'release', 'view', tag, '--repo', repository,
    '--json', 'assets,isDraft,isImmutable,isPrerelease,tagName,url',
  ]);
  if (!record(value) || !Array.isArray(value.assets) || typeof value.isDraft !== 'boolean'
    || typeof value.isImmutable !== 'boolean' || typeof value.isPrerelease !== 'boolean'
    || typeof value.tagName !== 'string' || typeof value.url !== 'string') {
    throw new Error('gh release view returned an invalid response');
  }
  const assets = value.assets.map((asset, index) => {
    if (!record(asset) || typeof asset.name !== 'string' || typeof asset.size !== 'number' || typeof asset.digest !== 'string') {
      throw new Error(`gh release view returned an invalid asset at index ${index}`);
    }
    return { name: asset.name, size: asset.size, digest: asset.digest };
  });
  return { ...value, assets } as ReleaseView;
}

function optionalReleaseView(repository: string, tag: string): ReleaseView | undefined {
  const lookup = command('gh', ['api', '--silent', `repos/${repository}/releases/tags/${tag}`]);
  if (lookup.status === 0 && !lookup.error) return releaseView(repository, tag);
  if (!lookup.error && /HTTP 404/.test(String(lookup.stderr))) return undefined;
  throw new Error(`Cannot determine remote Release state: ${failure('gh', [], lookup)}`);
}

export function installSourceArgs(version: string, baseUrl: string, publicDownload: boolean, windows: boolean): string[] {
  // Isolated archive acceptance must not register a user-session service that
  // outlives its temporary prefix or competes with the active installation.
  const isolated = windows ? ['-NoService'] : ['--no-service'];
  if (!publicDownload) return [...(windows ? ['-Version', version, '-BaseUrl', baseUrl] : ['--version', version, '--base-url', baseUrl]), ...isolated];
  return [...(version.includes('-') ? (windows ? ['-Version', version] : ['--version', version]) : []), ...isolated];
}

export async function readPublicInstaller(repository: string, installer: 'install.sh' | 'install.ps1', expected: string, fetcher = fetch): Promise<string> {
  const response = await fetcher(`https://raw.githubusercontent.com/${repository}/main/scripts/install/${installer}`, { signal: AbortSignal.timeout(60_000) });
  if (!response.ok) throw new Error(`Public installer URL failed: ${installer} (${response.status})`);
  const content = await response.text();
  if (Buffer.byteLength(content) > 1024 * 1024 || content !== expected) {
    throw new Error(`Public installer differs from the verification source: ${installer}`);
  }
  return content;
}

export function archiveAttestationArgs(archive: string, bundle: string, metadata: unknown, version: string, source: string): string[] {
  validateVersion(version);
  if (!/^[0-9a-f]{40}$/.test(source) || !record(metadata) || metadata.version !== version) throw new Error('Attestation source metadata differs from the immutable release');
  const repository = typeof metadata.repository === 'string' ? metadata.repository
    : record(metadata.repository) && typeof metadata.repository.url === 'string' ? metadata.repository.url : undefined;
  if (!repository) throw new Error('Attestation source repository is missing');
  const signer = repositorySlug(repository);
  return ['attestation', 'verify', archive, '--bundle', bundle, '--repo', signer,
    '--cert-identity', `https://github.com/${signer}/.github/workflows/release.yml@refs/tags/v${version}`,
    '--source-digest', source, '--source-ref', `refs/tags/v${version}`, '--deny-self-hosted-runners'];
}

async function cleanInstall(version: string, source: string, repository: string, scratch: string, releaseDirectory: string, publicDownload: boolean): Promise<void> {
  const prefix = path.join(scratch, 'install');
  const baseUrl = pathToFileURL(releaseDirectory).href.replace(/\/$/, '');
  // The maintained main-branch installers can change after an immutable release.
  const publicInstallers = path.join(scratch, 'public-installers');
  mkdirSync(publicInstallers);
  for (const installer of ['install.sh', 'install.ps1'] as const) {
    const expected = readFileSync(path.join(root, 'scripts', 'install', installer), 'utf8');
    writeFileSync(path.join(publicInstallers, installer), await readPublicInstaller(repository, installer, expected));
  }
  const installerDirectory = publicDownload ? publicInstallers : releaseDirectory;
  let entryFile: string;
  let runtime: string;
  if (process.platform === 'win32') {
    run('powershell.exe', ['-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', path.join(installerDirectory, 'install.ps1'),
      ...installSourceArgs(version, baseUrl, publicDownload, true), '-Prefix', prefix, '-NoModifyPath'], 10 * 60_000);
    const installRoot = path.join(prefix, 'lib', 'wombat');
    const releaseId = readFileSync(path.join(installRoot, 'current.txt'), 'utf8').trim();
    if (!/^[0-9A-Za-z._-]+$/.test(releaseId)) throw new Error('Windows installation pointer is invalid');
    const installed = path.join(installRoot, 'versions', releaseId);
    runtime = path.join(installed, 'runtime', 'node.exe');
    entryFile = path.join(installed, 'lib', 'wombat.js');

  } else {
    run('sh', [path.join(installerDirectory, 'install.sh'), ...installSourceArgs(version, baseUrl, publicDownload, false), '--prefix', prefix, '--no-modify-path'], 10 * 60_000);

    const installRoot = path.join(prefix, 'lib', 'wombat');
    const releaseId = readFileSync(path.join(installRoot, 'current.txt'), 'utf8').trim();
    if (!/^[0-9A-Za-z._-]+$/.test(releaseId)) throw new Error('Installation pointer is invalid');
    entryFile = path.join(installRoot, 'versions', releaseId, 'lib', 'wombat.js');
    runtime = path.join(installRoot, 'versions', releaseId, 'runtime', 'node');
  }
  const versionResult = parsedJson(runtime, [entryFile, '--version', '--json']);
  if (!record(versionResult) || versionResult.version !== version) throw new Error('Installed CLI version differs from the published version');
  const installedRelease: unknown = JSON.parse(readFileSync(path.join(path.dirname(path.dirname(entryFile)), 'release.json'), 'utf8'));
  if (!record(installedRelease) || installedRelease.version !== version || installedRelease.source !== source) {
    throw new Error('Clean installation differs from the published release identity');
  }
  const update = parsedJson(runtime, [entryFile, 'update', '--check', '--json', ...(version.includes('-') ? ['--version', version] : [])]);
  if (!record(update)) throw new Error('Installed updater returned an invalid response');
  if (!update.checked || update.updated || update.updateAvailable
    || update.currentVersion !== version || update.availableVersion !== version) {
    throw new Error(`Clean update check returned an invalid result: ${JSON.stringify(update)}`);
  }
  console.log(JSON.stringify(update));
}

async function main(): Promise<void> {
  if (process.argv.slice(2).some(arg => arg === '-h' || arg === '--help')) {
    console.log('Usage: corepack pnpm release:publish [-- --version <root-version> --branch main --repo owner/repo --status | --verify-published [--hosted]]');
    return;
  }
  const options = parsePublishArgs(process.argv.slice(2));
  const tag = `v${options.version}`;
  if (!options.verifyPublished) assertBranch(options.branch);
  run('gh', ['auth', 'status'], 60_000);

  const metadata = packageMetadata();
  const repository = options.repository ?? repositorySlug(metadata.repository);
  const origin = required('git', ['remote', 'get-url', 'origin']);
  if (repositorySlug(origin) !== repository) throw new Error(`origin ${origin} does not match ${repository}`);

  const existingTag = readRemoteRef(repository, `refs/tags/${tag}`);
  const existingRelease = optionalReleaseView(repository, tag);
  const head = required('git', ['rev-parse', 'HEAD']);
  if (options.verifyPublished) {
    if (!existingTag || !existingRelease) throw new Error('Published verification requires an existing tag and Release; no publication will be attempted');
    const errors = publishedReleaseErrors(existingRelease, options.version);
    if (errors.length) throw new Error(errors.join('\n'));
    if (command('git', ['merge-base', '--is-ancestor', existingTag, head]).status !== 0) throw new Error('Published source is not an ancestor of this branch');
    if (changedFiles().length || readRemoteRef(repository, `refs/heads/${options.branch}`) !== head) throw new Error('Published verification requires a clean checkout matching the pushed branch');
    if (options.hosted) {
      const prior = Math.max(0, ...workflowRuns(repository, 'ci.yml', head).map(item => item.databaseId));
      run('gh', ['workflow', 'run', 'ci.yml', '--repo', repository, '--ref', options.branch, '-f', 'target=published-release']);
      const verification = waitForRun(repository, 'ci.yml', head, options.branch, 'workflow_dispatch', prior);
      const evidence = parsedJson('gh', ['api', `repos/${repository}/actions/runs/${verification.databaseId}/jobs?per_page=100`]);
      assertHostedVerificationJobs(evidence);
      console.log(JSON.stringify({version: options.version, source: existingTag, verificationSource: head, releaseUrl: existingRelease.url, verificationUrl: verification.url, publicDownload: true}));
      return;
    }
    const view = await verifyPublishedRelease(repository, options.version, existingTag, true);
    console.log(JSON.stringify({version: options.version, source: existingTag, releaseUrl: view.url, immutable: view.isImmutable, publicDownload: true}));
    return;
  }
  const existingRun = existingTag ? selectWorkflowRun(workflowRuns(repository, 'release.yml', existingTag), existingTag, tag) : undefined;
  if (options.status) {
    let recovery: string;
    try { recovery = releaseRecoveryState(head, existingTag, existingRelease, existingRun); }
    catch (error) { recovery = `blocked: ${error instanceof Error ? error.message : String(error)}`; }
    console.log(JSON.stringify({version: options.version, tag, source: head, tagSource: existingTag,
      recovery, releaseUrl: existingRelease?.url, workflow: existingRun, versionErrors: consistencyErrors(root),
      changedFiles: changedFiles()}, null, 2));
    return;
  }
  const recovery = releaseRecoveryState(head, existingTag, existingRelease, existingRun);
  readReleaseNotesInput(root);
  const remoteRef = (ref: string) => readRemoteRef(repository, ref);
  const remoteMainSource = remoteRef(`refs/heads/${options.branch}`);
  if (remoteMainSource && command('git', ['cat-file', '-e', `${remoteMainSource}^{commit}`]).status !== 0) {
    run('git', ['fetch', '--no-tags', 'origin', options.branch], 120_000);
  }
  let source: string;
  let ci: WorkflowRun;
  if (existingTag) {
    const dirty = changedFiles();
    if (dirty.length) throw new Error(`Resume an existing release from a clean tree; found: ${dirty.join(', ')}`);
    const errors = consistencyErrors(root);
    if (errors.length) throw new Error(errors.join('\n'));
    const remoteMain = remoteRef(`refs/heads/${options.branch}`);
    if (!remoteMain || command('git', ['merge-base', '--is-ancestor', existingTag, remoteMain]).status !== 0) {
      throw new Error(`${tag} source ${existingTag} is not on origin/${options.branch}`);
    }
    source = existingTag;
    stage('Resume existing immutable release identity');
    ci = waitForRun(repository, 'ci.yml', source, options.branch);
    immutableReleasesEnabled(repository);
  } else {
    let dirty = changedFiles();
    const unexpected = dirty.filter(file => !managedReleaseFiles.has(file));
    if (unexpected.length) throw new Error(`Release automation will not commit unrelated files: ${unexpected.join(', ')}`);
    if (consistencyErrors(root).length) {
      stage('Synchronize root-version mirrors');
      run('corepack', ['pnpm', 'release:prepare'], 10 * 60_000);
    }
    dirty = changedFiles();
    const changedDuringChecks = dirty.filter(file => !managedReleaseFiles.has(file));
    if (changedDuringChecks.length) throw new Error(`Unrelated changes appeared during preparation: ${changedDuringChecks.join(', ')}`);
    const remoteMainBefore = remoteRef(`refs/heads/${options.branch}`);
    source = required('git', ['rev-parse', 'HEAD']);
    if (dirty.length || remoteMainBefore !== source) {
      stage('Local source checks; platform gates run once in exact-source CI');
      run('corepack', ['pnpm', 'repo:check'], 10 * 60_000);
    }
    if (required('git', ['rev-parse', 'HEAD']) !== source || releaseVersion(root) !== options.version) {
      throw new Error('Release source or root version changed during source checks; inspect before resuming');
    }
    dirty = changedFiles();
    if (dirty.length) {
      stage('Commit release candidate');
      if (dirty.some(file => !managedReleaseFiles.has(file))) throw new Error('Working tree changed during source checks; inspect before resuming');
      run('git', ['add', '--', ...dirty]);
      run('git', ['diff', '--cached', '--check']);
      run('git', ['commit', '-m', `Prepare ${tag}`]);
      source = required('git', ['rev-parse', 'HEAD']);
    }
    const remoteMain = remoteRef(`refs/heads/${options.branch}`);
    if (remoteMain !== source) {
      if (remoteMain && command('git', ['merge-base', '--is-ancestor', remoteMain, source]).status !== 0) {
        throw new Error(`Local ${options.branch} is not a fast-forward of origin/${options.branch}`);
      }
      stage('Push release candidate');
      run('git', ['push', 'origin', `${options.branch}:${options.branch}`], 5 * 60_000);
    }
    if (changedFiles().length) throw new Error('Working tree changed during release preparation');
    stage('Wait for exact-source CI');
    ci = waitForRun(repository, 'ci.yml', source, options.branch);
    readCiCandidate(repository, ci.databaseId, source, options.branch);
    immutableReleasesEnabled(repository);
    stage('Pre-tag verification');
    if (required('git', ['rev-parse', 'HEAD']) !== source || releaseVersion(root) !== options.version) {
      throw new Error('Release source or root version changed while waiting for CI; inspect before tagging');
    }
    run('corepack', ['pnpm', 'release:preflight', '--', '--version', options.version, '--branch', options.branch, '--repo', repository], 5 * 60_000);
    stage('Create and push immutable release tag');
    if (command('git', ['show-ref', '--verify', '--quiet', `refs/tags/${tag}`]).status !== 0) {
      run('git', ['tag', '-a', tag, source, '-m', `Wombat ${tag}`]);
    } else if (required('git', ['rev-list', '-n', '1', tag]) !== source) {
      throw new Error(`Local ${tag} does not point to ${source}`);
    }
    run('git', ['push', 'origin', `refs/tags/${tag}`], 5 * 60_000);
  }

  stage('Wait for GitHub Release');
  const releaseRun = recovery === 'verify' ? existingRun : waitForRun(repository, 'release.yml', source, tag);
  const view = await verifyPublishedRelease(repository, options.version, source);

  console.log(JSON.stringify({
    version: options.version, tag, source, repository, ciUrl: ci.url,
    releaseRunUrl: releaseRun?.url, releaseUrl: view.url, immutable: view.isImmutable,
    assets: view.assets.map(asset => asset.name).sort(),
  }, null, 2));
}

export function verificationDownloads(view: ReleaseView, target: NativeTarget): PublishedAsset[] {
  if (!nativeTargets.includes(target)) throw new Error('Unsupported verification target');
  const names = new Set(['install.sh', 'install.ps1', 'release-set.json', 'SHA256SUMS', releaseArchive(target)]);
  const errors = publishedReleaseErrors(view, view.tagName.replace(/^v/, ''));
  if (errors.length) throw new Error(errors.join('\n'));
  return view.assets.filter(asset => names.has(asset.name));
}

/** Whole-release signatures and metadata cover every target; local installation needs only this host's archive. */
export async function verifyPublishedRelease(repository: string, version: string, source: string, publicDownload = false): Promise<ReleaseView> {
  const tag = `v${version}`;
  if (readRemoteRef(repository, `refs/tags/${tag}`) !== source) throw new Error('Published tag source differs from the requested verification identity');
  const view = releaseView(repository, tag);
  const releaseErrors = publishedReleaseErrors(view, version);
  if (releaseErrors.length) throw new Error(releaseErrors.join('\n'));

  stage('Verify published assets and attestations');
  run('gh', ['release', 'verify', tag, '--repo', repository], 5 * 60_000);
  const target = currentNativeTarget();
  const download = releaseCacheDirectory(process.env.WOMBAT_RELEASE_CACHE ?? path.join(os.tmpdir(), 'wombat-release-cache'), repository, version, source);
  const cached = cacheReleaseAssets(download, verificationDownloads(view, target), (missing, destination) => {
    run('gh', ['release', 'download', tag, '--repo', repository, '--dir', destination,
      ...missing.flatMap(asset => ['--pattern', asset.name])], 15 * 60_000);
  }, file => run('gh', ['release', 'verify-asset', tag, file, '--repo', repository], 5 * 60_000));
  for (const result of cached) console.log(`${path.basename(result.file)}: ${result.reused ? 'verified cache' : 'verified download'}`);
  const archives = verifyReleaseAssets(download, version, source, {target, assets: view.assets});
  const sourceMetadata = parsedJson('git', ['show', `${source}:package.json`]);
  const scratch = mkdtempSync(path.join(os.tmpdir(), `wombat-${tag}-`));
  try {
    for (const archive of archives) {
      const asset = view.assets.find(item => item.name === path.basename(archive));
      if (!asset) throw new Error('Archive is missing from the signed release');
      // Fetch from the current repository; verify the historical signing identity without following an old namespace.
      const response = parsedJson('gh', ['api', `repos/${repository}/attestations/${asset.digest}`]);
      if (!record(response) || !Array.isArray(response.attestations) || !response.attestations.length
        || !response.attestations.every(item => record(item) && record(item.bundle))) throw new Error('GitHub returned invalid archive attestations');
      const bundle = path.join(scratch, `${asset.name}.attestations.jsonl`);
      writeFileSync(bundle, response.attestations.map(item => JSON.stringify(item.bundle)).join('\n') + '\n');
      run('gh', archiveAttestationArgs(archive, bundle, sourceMetadata, version, source), 5 * 60_000);
    }
    stage('Clean install and update check');
    await cleanInstall(version, source, repository, scratch, download, publicDownload);
  } finally {
    rmSync(scratch, { recursive: true, force: true });
  }

  return view;
}

const entry = process.argv[1] ? pathToFileURL(path.resolve(process.argv[1])).href : '';
if (entry === import.meta.url) {
  try {
    await main();
  } catch (error) {
    console.error(error instanceof Error ? error.message : error);
    process.exit(1);
  }
}
