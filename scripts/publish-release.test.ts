import assert from 'node:assert/strict';
import test from 'node:test';
import {spawnSync} from 'node:child_process';
import {fileURLToPath} from 'node:url';

import {
  expectedReleaseAssets,
  parsePublishArgs,
  publishedReleaseErrors,
  selectWorkflowRun,
  type WorkflowRun,
  releaseRecoveryState,
  verificationDownloads,
  installSourceArgs,
  assertHostedVerificationJobs,
  readPublicInstaller,
  archiveAttestationArgs,
} from './publish-release.ts';
import {nativeTargets} from './native-platforms.ts';

test('archive attestations retain immutable source identity through repository renames', () => {
  const source = 'a'.repeat(40);
  const metadata = {version: '1.0.0', repository: {url: 'git+https://github.com/original-owner/project.git'}};
  assert.deepEqual(archiveAttestationArgs('archive.tar.gz', 'current-repo.jsonl', metadata, '1.0.0', source), [
    'attestation', 'verify', 'archive.tar.gz', '--bundle', 'current-repo.jsonl', '--repo', 'original-owner/project',
    '--cert-identity', 'https://github.com/original-owner/project/.github/workflows/release.yml@refs/tags/v1.0.0',
    '--source-digest', source, '--source-ref', 'refs/tags/v1.0.0', '--deny-self-hosted-runners',
  ]);
  assert.ok(archiveAttestationArgs('archive', 'bundle', {...metadata, repository: 'https://github.com/current-owner/project'}, '1.0.0', source).includes('current-owner/project'));
  for (const invalid of [undefined, {...metadata, version: '2.0.0'}, {...metadata, repository: undefined},
    {...metadata, repository: 'https://untrusted.example/project'}]) {
    assert.throws(() => archiveAttestationArgs('archive', 'bundle', invalid, '1.0.0', source));
  }
  assert.throws(() => archiveAttestationArgs('archive', 'bundle', metadata, '1.0.0', 'invalid-source'));
});

test('public installers bind to the current verification source after a repository rename', async () => {
  const expected = '#!/bin/sh\nrepo="new-owner/wombat"\n';
  const old = '#!/bin/sh\nrepo="old-owner/wombat"\n';
  const fetcher: typeof fetch = async (url, options) => {
    assert.equal(url, 'https://raw.githubusercontent.com/new-owner/wombat/main/scripts/install/install.sh');
    assert.ok(options?.signal);
    return new Response(expected);
  };
  assert.equal(await readPublicInstaller('new-owner/wombat', 'install.sh', expected, fetcher), expected);
  await assert.rejects(readPublicInstaller('new-owner/wombat', 'install.sh', old, fetcher), /differs from the verification source/);
  await assert.rejects(readPublicInstaller('new-owner/wombat', 'install.sh', expected,
    async () => new Response('missing', {status: 404})), /Public installer URL failed.*404/);
  const large = 'x'.repeat(1024 * 1024 + 1);
  await assert.rejects(readPublicInstaller('new-owner/wombat', 'install.sh', large,
    async () => new Response(large)), /differs from the verification source/);
});

test('hosted recovery requires complete evidence and the dedicated public installation job to pass', () => {
  const job = {name: 'Verify public installation and update', status: 'completed', conclusion: 'success'};
  assert.doesNotThrow(() => assertHostedVerificationJobs({total_count: 1, jobs: [job]}));
  for (const value of [undefined, {total_count: 2, jobs: [job]}, {total_count: 0, jobs: []},
    {total_count: 2, jobs: [job, job]}, {total_count: 1, jobs: [{...job, conclusion: 'skipped'}]},
    {total_count: 1, jobs: [{...job, status: 'in_progress'}]}]) assert.throws(() => assertHostedVerificationJobs(value));
});

test('public verification downloads only the host archive while requiring the complete immutable release', () => {
  const view = {assets: expectedReleaseAssets().map(name => ({name, size: 1, digest: `sha256:${'a'.repeat(64)}`})),
    isDraft: false, isImmutable: true, isPrerelease: false, tagName: 'v1.0.0', url: 'https://github.com/owner/repo/releases/tag/v1.0.0'};
  for (const target of nativeTargets) {
    assert.deepEqual(verificationDownloads(view, target).map(asset => asset.name).sort(),
      ['install.sh', 'install.ps1', 'release-set.json', 'SHA256SUMS', `wombat-${target}.tar.gz`].sort());
  }
  assert.throws(() => verificationDownloads({...view, assets: view.assets.slice(1)}, nativeTargets[0]), /Release assets differ/);
  assert.throws(() => verificationDownloads({...view, isImmutable: false}, nativeTargets[0]), /not immutable/);
});

test('parses one explicit release identity and rejects ambiguous input', () => {
  assert.deepEqual(parsePublishArgs([], '0.1.0-beta.1'), {
    version: '0.1.0-beta.1', branch: 'main', repository: undefined, status: false, verifyPublished: false, hosted: false,
  });
  assert.deepEqual(parsePublishArgs(['--', '--version', '1.0.0', '--branch', 'release', '--repo', 'owner/repo', '--status'], '1.0.0'), {
    version: '1.0.0', branch: 'release', repository: 'owner/repo', status: true, verifyPublished: false, hosted: false,
  });
  assert.throws(() => parsePublishArgs(['--version', 'v1.0.0']));
  assert.throws(() => parsePublishArgs(['--version', '1.0.0', '--unknown']));
  assert.throws(() => parsePublishArgs(['--version', '1.0.1'], '1.0.0'), /differs from package.json/);
  assert.throws(() => parsePublishArgs(['--version', '1.0.0', '--version', '1.0.0'], '1.0.0'), /Duplicate/);
  assert.equal(parsePublishArgs(['--verify-published'], '1.0.0').verifyPublished, true);
  assert.throws(() => parsePublishArgs(['--status', '--verify-published']), /Choose status/);
  assert.equal(parsePublishArgs(['--verify-published', '--hosted'], '1.0.0').hosted, true);
  assert.throws(() => parsePublishArgs(['--hosted']), /requires --verify-published/);
});

test('isolated published installation preserves public version routes without registering native services', () => {
  for (const windows of [false, true]) {
    const isolated = windows ? ['-NoService'] : ['--no-service'];
    assert.deepEqual(installSourceArgs('1.0.0', 'file:///cache', true, windows), isolated);
    assert.deepEqual(installSourceArgs('1.0.0-beta.1', 'file:///cache', true, windows), windows ? ['-Version', '1.0.0-beta.1', ...isolated] : ['--version', '1.0.0-beta.1', ...isolated]);
    assert.deepEqual(installSourceArgs('1.0.0', 'file:///cache', false, windows), windows ? ['-Version', '1.0.0', '-BaseUrl', 'file:///cache', ...isolated] : ['--version', '1.0.0', '--base-url', 'file:///cache', ...isolated]);
    // Archive upgrade qualification passes the preceding public tag unchanged.
    assert.deepEqual(installSourceArgs('v0.3.6', 'file:///previous', false, windows), windows ? ['-Version', 'v0.3.6', '-BaseUrl', 'file:///previous', ...isolated] : ['--version', 'v0.3.6', '--base-url', 'file:///previous', ...isolated]);
  }
});

test('preliminary status and verification checks never publish missing releases or hide network failures', () => {
  const file = fileURLToPath(new URL('./publish-release.ts', import.meta.url));
  for (const mode of ['--status', '--verify-published']) for (const networkFailure of [false, true]) {
    const result = spawnSync(process.execPath, ['--input-type=module', '-e', `
      import childProcess from 'node:child_process';
      import {syncBuiltinESMExports} from 'node:module';
      import {pathToFileURL} from 'node:url';
      childProcess.spawnSync = (program, args) => {
        const key = program + ' ' + args.join(' ');
        const ok = stdout => ({status: 0, stdout, stderr: '', error: undefined});
        if (key === 'git symbolic-ref --quiet --short HEAD') return ok('main');
        if (key === 'git remote get-url origin') return ok('https://github.com/YannByte/wombat.git');
        if (key === 'git rev-parse HEAD') return ok('a'.repeat(40));
        if (['git diff --name-only -z', 'git diff --cached --name-only -z', 'git ls-files --others --exclude-standard -z'].includes(key)) return ok('');
        if (key === 'gh auth status') return ok('authenticated');
        if (program === 'gh' && args[0] === 'api' && /^(?:--silent )?repos\\/YannByte\\/wombat\\/(?:git\\/ref\\/tags|releases\\/tags)\\/v/.test(args.slice(1).join(' '))) {
          return {status: 1, stdout: '', stderr: ${networkFailure ? "'network unavailable'" : "'HTTP 404'"}};
        }
        throw new Error('Forbidden status operation: ' + key);
      };
      syncBuiltinESMExports();
      process.argv = [process.execPath, ${JSON.stringify(file)}, ${JSON.stringify(mode)}];
      await import(pathToFileURL(${JSON.stringify(file)}).href);
    `], {encoding: 'utf8', timeout: 10_000, maxBuffer: 1024 * 1024});
    assert.ifError(result.error);
    assert.doesNotMatch(result.stderr, /Forbidden status operation/);
    if (networkFailure) {
      assert.notEqual(result.status, 0);
      assert.match(result.stderr, /GitHub ref query failed: network unavailable/);
    } else if (mode === '--status') {
      assert.equal(result.status, 0, result.stderr);
      assert.match(result.stdout, /"recovery": "prepare"/);
    } else {
      assert.notEqual(result.status, 0);
      assert.match(result.stderr, /requires an existing tag and Release; no publication/);
    }
  }
});

test('selects only the exact push workflow run', () => {
  const run = (overrides: Partial<WorkflowRun>): WorkflowRun => ({
    conclusion: '', databaseId: 1, event: 'push', headBranch: 'main', headSha: 'a'.repeat(40),
    status: 'in_progress', url: 'https://github.com/owner/repo/actions/runs/1', ...overrides,
  });
  const expected = run({ databaseId: 4 });
  assert.equal(selectWorkflowRun([
    run({ databaseId: 2, event: 'workflow_dispatch' }),
    run({ databaseId: 3, headSha: 'b'.repeat(40) }),
    expected,
  ], 'a'.repeat(40), 'main'), expected);
  const dispatched = run({databaseId: 5, event: 'workflow_dispatch'});
  assert.equal(selectWorkflowRun([expected, dispatched], 'a'.repeat(40), 'main', 'workflow_dispatch', 4), dispatched);
  assert.equal(selectWorkflowRun([expected, dispatched], 'a'.repeat(40), 'main', 'workflow_dispatch', 5), undefined);
  assert.equal(selectWorkflowRun([dispatched], 'b'.repeat(40), 'main', 'workflow_dispatch'), undefined);
});

test('requires the complete immutable release asset set', () => {
  const assets = expectedReleaseAssets().map(name => ({ name, size: 1, digest: `sha256:${'a'.repeat(64)}` }));
  const view = {
    assets, isDraft: false, isImmutable: true, isPrerelease: true,
    tagName: 'v0.1.0-beta.1', url: 'https://github.com/owner/repo/releases/tag/v0.1.0-beta.1',
  };
  assert.deepEqual(publishedReleaseErrors(view, '0.1.0-beta.1'), []);
  assert.match(publishedReleaseErrors({ ...view, isImmutable: false }, '0.1.0-beta.1').join('\n'), /not immutable/);
  assert.match(publishedReleaseErrors({ ...view, assets: assets.slice(1) }, '0.1.0-beta.1').join('\n'), /assets differ/);
});

test('resumes observed release states without replacing failed public identities', () => {
  const head = 'a'.repeat(40);
  const run: WorkflowRun = {headSha: head, headBranch: 'v1.0.0', databaseId: 1, event: 'push',
    conclusion: '', status: 'in_progress', url: 'https://example.test/run'};
  const release = {assets: [], isDraft: true, isImmutable: false, isPrerelease: false, tagName: 'v1.0.0', url: 'https://example.test/release'};
  assert.equal(releaseRecoveryState(head, undefined, undefined, undefined), 'prepare');
  assert.equal(releaseRecoveryState(head, head, undefined, undefined), 'wait');
  assert.equal(releaseRecoveryState(head, head, release, run), 'wait');
  assert.equal(releaseRecoveryState(head, head, {...release, isDraft: false}, {...run, status: 'completed', conclusion: 'success'}), 'verify');
  assert.throws(() => releaseRecoveryState(head, 'b'.repeat(40), undefined, run), /prepare a new version/);
  assert.throws(() => releaseRecoveryState(head, undefined, release, run), /without its remote tag/);
  for (const conclusion of ['failure', 'cancelled', 'timed_out']) {
    assert.throws(() => releaseRecoveryState(head, head, release, {...run, status: 'completed', conclusion}), /do not retag/);
  }
  assert.throws(() => releaseRecoveryState(head, head, undefined, {...run, status: 'completed', conclusion: 'success'}), /no complete published Release/);
});
