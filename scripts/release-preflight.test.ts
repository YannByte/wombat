import assert from 'node:assert/strict';
import test from 'node:test';

import { parseArgs } from './release-preflight.ts';
import { actionPinErrors, repositorySlug, rootReadmeReleaseErrors, successfulCiRun } from './release-policy.ts';

test('parses an explicit release identity and rejects ambiguous arguments', () => {
  assert.deepEqual(parseArgs(['--version', '0.1.0-dev.3', '--json']), {
    version: '0.1.0-dev.3', branch: 'main', repository: undefined, json: true,
  });
  assert.deepEqual(parseArgs(['--version', '1.0.0', '--branch', 'release', '--repo', 'owner/project']), {
    version: '1.0.0', branch: 'release', repository: 'owner/project', json: false,
  });
  assert.throws(() => parseArgs([]), /Usage:/);
  assert.throws(() => parseArgs(['--version', 'v1.0.0']), /Invalid release version/);
  assert.throws(() => parseArgs(['--version', '1.0.0', '--repo', 'project']), /Invalid GitHub repository/);
});

test('normalizes supported GitHub repository URLs', () => {
  for (const value of [
    'git+https://github.com/YannByte/wombat.git',
    'https://github.com/YannByte/wombat',
    'git@github.com:YannByte/wombat.git',
  ]) assert.equal(repositorySlug(value), 'YannByte/wombat');
  assert.throws(() => repositorySlug('https://example.com/wombat.git'), /not a GitHub repository/);
  assert.throws(() => repositorySlug('https://evilgithub.com/owner/project.git'), /not a GitHub repository/);
});

test('accepts only a successful push CI run for the exact source and branch', () => {
  const source = 'a'.repeat(40);
  const base = { headSha: source, headBranch: 'main', event: 'push', status: 'completed', conclusion: 'success', url: 'https://example.test/run' };
  assert.deepEqual(successfulCiRun([base], source, 'main'), base);
  for (const changed of [
    { ...base, headSha: 'b'.repeat(40) },
    { ...base, headBranch: 'feature' },
    { ...base, event: 'pull_request' },
    { ...base, status: 'in_progress' },
    { ...base, conclusion: 'failure' },
  ]) assert.equal(successfulCiRun([changed], source, 'main'), undefined);
});

test('rejects stale or incomplete root README release facts', () => {
  const version = '0.1.0-dev.3';
  const repository = 'owner/project';
  const release = `https://github.com/${repository}/releases/tag/v${version}`;
  const raw = `https://raw.githubusercontent.com/${repository}/main`;
  const platforms = 'macOS arm64/x64, Linux glibc arm64/x64, Windows x64';
  const installers = `curl -fsSL ${raw}/scripts/install/install.sh | sh -s -- --version ${version} --plugin\n& ([scriptblock]::Create((irm ${raw}/scripts/install/install.ps1))) -Version ${version} -Plugin`;
  const english = `Development Preview: [\`v${version}\`](${release})\n${installers}\n${platforms}\n## Update\n[Installation](docs/guides/installation.en.md)`;
  const chinese = `开发者预览版：[\`v${version}\`](${release})\n${installers}\n${platforms}\n## 更新\n[安装](docs/guides/installation.md)`;
  assert.deepEqual(rootReadmeReleaseErrors({ english, chinese }, version, repository), []);
  const errors = rootReadmeReleaseErrors({
    english: `${english}\nThe URL becomes available after the release workflow completes.`,
    chinese: chinese.replace(`${raw}/scripts/install/install.ps1`, 'missing-installer'),
  }, version, repository);
  assert.ok(errors.some(error => error.includes('stale release text')));
  assert.ok(errors.some(error => error.includes('README.zh-CN.md: missing current release fact')));
  assert.ok(rootReadmeReleaseErrors({
    english: `${english}\nThis first public preview is ready.`,
    chinese: `${chinese}\n这是首个公开预览版本。`,
  }, version, repository).some(error => error.includes('stale first-release description')));
});

test('requires the README stage to match beta versions', () => {
  const repository = 'owner/repo';
  const version = '0.1.0-beta.1';
  const url = `https://github.com/${repository}/releases/tag/v${version}`;
  const raw = `https://raw.githubusercontent.com/${repository}/main`;
  const common = `[\`v${version}\`](${url})\ncurl -fsSL ${raw}/scripts/install/install.sh | sh -s -- --version ${version} --plugin\n& ([scriptblock]::Create((irm ${raw}/scripts/install/install.ps1))) -Version ${version} -Plugin\nmacOS arm64/x64 Linux glibc arm64/x64 Windows x64`;
  const english = `Beta: ${common}\n## Update\n[Installation](docs/guides/installation.en.md)`;
  const chinese = `Beta 测试版：${common}\n## 更新\n[安装](docs/guides/installation.md)`;
  assert.deepEqual(rootReadmeReleaseErrors({
    english,
    chinese,
  }, version, repository), []);
  assert.match(rootReadmeReleaseErrors({
    english: english.replace('Beta:', 'Development Preview:'),
    chinese: chinese.replace('Beta 测试版：', '开发者预览版：'),
  }, version, repository).join('\n'), /missing Beta/);
});

test('requires stable README installation and update facts without preview flags', () => {
  const repository = 'owner/repo', version = '0.1.0';
  const url = `https://github.com/${repository}/releases/tag/v${version}`;
  const raw = `https://raw.githubusercontent.com/${repository}/main`;
  const common = `[\`v${version}\`](${url})\ncurl -fsSL ${raw}/scripts/install/install.sh | sh -s -- --plugin\n& ([scriptblock]::Create((irm ${raw}/scripts/install/install.ps1))) -Plugin\nmacOS arm64/x64 Linux glibc arm64/x64 Windows x64`;
  const english = `Stable: ${common}\n## Update\n[Installation](docs/guides/installation.en.md)`;
  const chinese = `正式版：${common}\n## 更新\n[安装](docs/guides/installation.md)`;
  assert.deepEqual(rootReadmeReleaseErrors({english, chinese}, version, repository), []);
  assert.match(rootReadmeReleaseErrors({english: english.replace('Stable:', 'Beta:'), chinese: chinese.replace('正式版：', 'Beta 测试版：')}, version, repository).join('\n'), /missing Stable/);
  for (const [good, bad] of [
    [' --plugin', ''], [' -Plugin', ''],
    ['| sh -s -- --plugin', `| sh -s -- --version ${version}-beta.1 --plugin`],
    ['## Update', 'Update text'],
    ['(docs/guides/installation.en.md)', '(missing-guide)'],
  ]) assert.ok(rootReadmeReleaseErrors({english: english.replace(good, bad), chinese}, version, repository).length > 0, good);
  assert.ok(rootReadmeReleaseErrors({english, chinese: chinese.replace('(docs/guides/installation.md)', '(missing-guide)')}, version, repository).length > 0);
});

test('requires immutable external Action references', () => {
  const sha = '1'.repeat(40);
  assert.deepEqual(actionPinErrors({
    '.github/workflows/ci.yml': `steps:\n  - uses: actions/checkout@${sha} # v4\n  - uses: ./.github/actions/local\n`,
  }), []);
  const errors = actionPinErrors({
    '.github/workflows/ci.yml': 'steps:\n  - uses: actions/checkout@v4\n  - uses: owner/action\n',
  });
  assert.equal(errors.length, 2);
  assert.match(errors[0], /full commit SHA/);
  assert.deepEqual(actionPinErrors({
    '.github/workflows/container.yml': `steps:\n  - uses: docker://alpine@sha256:${'a'.repeat(64)}\n`,
  }), []);
  assert.match(actionPinErrors({
    '.github/workflows/container.yml': 'steps:\n  - uses: docker://alpine:latest\n',
  })[0], /SHA-256 digest/);
});
