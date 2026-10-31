/** Assemble platform archives for GitHub Releases; never uploads or creates a release. */
import {spawnSync} from 'node:child_process';
import {chmodSync, copyFileSync, cpSync, existsSync, mkdirSync, mkdtempSync, readFileSync, readdirSync, rmSync, statSync, writeFileSync} from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {parseArgs} from 'node:util';
import {checkBuild} from './build-identity.ts';
import {releaseArchive, releaseNodeVersion, type GitHubReleaseMetadata, type GitHubReleaseSet} from './github-release.ts';
import {checkedSourceRevision, currentNativeTarget, inspectNative, nativeBinary, nativeNotices, nativeTargets, nodeRuntimeBinary, resolveNodeRuntimeLicense, sha256} from './native-platforms.ts';
import {toolCommand} from './run-tool.ts';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const output = path.join(root, 'dist', 'github');
const {values} = parseArgs({args: process.argv.slice(2).filter(arg => arg !== '--'), options: {
  'native-dir': {type: 'string'}, 'current-platform': {type: 'boolean', default: false},
  'reuse-build': {type: 'boolean', default: false},
  'runtime-license': {type: 'string'},
}});
if (Boolean(values['native-dir']) === values['current-platform'])
  throw new Error('Choose --native-dir <five-platform-artifacts> or --current-platform');

function run(program: string, args: string[], cwd = root, stream = false, timeout = 900_000): string {
  const result = spawnSync(...toolCommand(program, args), stream
    ? {cwd, stdio: 'inherit', timeout}
    : {cwd, encoding: 'utf8', timeout, maxBuffer: 16 * 1024 * 1024});
  if (result.error || result.status !== 0)
    throw new Error(`${program}: ${result.error?.message ?? result.stderr ?? result.status}`);
  return typeof result.stdout === 'string' ? result.stdout : '';
}

if (!values['reuse-build']) run('corepack', ['pnpm', 'release:check'], root, true);
checkBuild(root);
const metadata = JSON.parse(readFileSync(path.join(root, 'package.json'), 'utf8'));
const receipt = JSON.parse(readFileSync(path.join(root, 'dist', 'build.json'), 'utf8'));
const revision = run('git', ['rev-parse', 'HEAD']).trim();
const targets = values['current-platform'] ? [currentNativeTarget()] : [...nativeTargets];
const nativeDirectory = values['native-dir'] ? path.resolve(values['native-dir']) : undefined;
if (nativeDirectory) {
  const checked = checkedSourceRevision();
  if (checked !== revision) throw new Error('Source revision changed during release assembly');
  for (const target of targets) inspectNative(nativeDirectory, target, metadata.version, revision);
}

rmSync(output, {recursive: true, force: true});
mkdirSync(output, {recursive: true});
const set: GitHubReleaseSet = {
  format: 1, version: metadata.version, source: revision, sourceSha256: receipt.sourceSha256,
  candidateOnly: values['current-platform'], targets, assets: [],
};

const cliFiles = readdirSync(path.join(root, 'dist')).filter(file => /^wombat(?:-.*)?\.js$/.test(file));
if (!cliFiles.includes('wombat.js')) throw new Error('Build first: missing dist/wombat.js');

for (const target of targets) {
  const stage = mkdtempSync(path.join(output, `.stage-${target}-`));
  const bundle = path.join(stage, 'wombat');
  const runtime = path.join(bundle, 'lib');
  const licenses = path.join(runtime, 'licenses');
  try {
    mkdirSync(path.join(bundle, 'bin'), {recursive: true});
    mkdirSync(path.join(bundle, 'runtime'), {recursive: true});
    mkdirSync(licenses, {recursive: true});
    for (const file of cliFiles) copyFileSync(path.join(root, 'dist', file), path.join(runtime, file));
    cpSync(path.join(root, 'dist', 'skill'), path.join(runtime, 'skill'), {recursive: true});
    copyFileSync(path.join(root, 'LICENSE'), path.join(bundle, 'LICENSE'));
    const notices = readFileSync(path.join(root, 'licenses/THIRD_PARTY_NOTICES.md'), 'utf8');
    writeFileSync(path.join(bundle, 'THIRD_PARTY_NOTICES.md'), notices
      .replace('(../LICENSE)', '(LICENSE)')
      .replace('(node-dependencies.txt)', '(lib/licenses/node-dependencies.txt)')
      .replace('(rust-dependencies.txt)', '(lib/licenses/rust-dependencies.txt)')
      .replace('(../docs/dependency-licenses.json)', '(lib/licenses/inventory.json)'));
    copyFileSync(path.join(root, 'README.md'), path.join(bundle, 'README.md'));
    copyFileSync(path.join(root, 'README.zh-CN.md'), path.join(bundle, 'README.zh-CN.md'));
    copyFileSync(path.join(root, 'LICENSE'), path.join(licenses, 'wombat-MIT.txt'));
    writeFileSync(path.join(licenses, 'third-party-notices.md'), notices
      .replace('(../LICENSE)', '(wombat-MIT.txt)')
      .replace('(../docs/dependency-licenses.json)', '(inventory.json)'));

    const binary = nativeBinary(target);
    const runtimeBinary = nodeRuntimeBinary(target);
    const nativeRoot = nativeDirectory ? path.join(nativeDirectory, target) : path.join(root, 'dist');
    copyFileSync(path.join(nativeRoot, binary), path.join(runtime, binary));
    if (!target.startsWith('win32-')) chmodSync(path.join(runtime, binary), 0o755);
    if (nativeDirectory) {
      copyFileSync(path.join(nativeRoot, 'runtime', runtimeBinary), path.join(bundle, 'runtime', runtimeBinary));
      for (const file of nativeNotices)
        copyFileSync(path.join(nativeRoot, 'licenses', file), path.join(licenses, file));
    } else {
      if (process.version !== `v${releaseNodeVersion}`) throw new Error(`Release runtime must be Node ${releaseNodeVersion}; found ${process.version}`);
      copyFileSync(process.execPath, path.join(bundle, 'runtime', runtimeBinary));
      copyFileSync(path.join(root, 'licenses', 'node-dependencies.txt'), path.join(licenses, 'node-dependencies.txt'));
      copyFileSync(path.join(root, 'licenses', 'rust-dependencies.txt'), path.join(licenses, 'rust-dependencies.txt'));
      copyFileSync(path.join(root, 'docs', 'dependency-licenses.json'), path.join(licenses, 'inventory.json'));
      copyFileSync(resolveNodeRuntimeLicense(values['runtime-license']), path.join(licenses, 'node-runtime.txt'));
    }
    if (!target.startsWith('win32-')) chmodSync(path.join(bundle, 'runtime', runtimeBinary), 0o755);

    const launcher = '#!/bin/sh\nset -eu\ncase "$0" in */*) WOMBAT_SCRIPT_DIR=${0%/*} ;; *) WOMBAT_SCRIPT_DIR=. ;; esac\nWOMBAT_DIR=$(CDPATH= cd -- "$WOMBAT_SCRIPT_DIR/.." && pwd)\nexec "$WOMBAT_DIR/runtime/node" "$WOMBAT_DIR/lib/wombat.js" "$@"\n';
    writeFileSync(path.join(bundle, 'bin', 'wombat'), launcher, {mode: 0o755});
    writeFileSync(path.join(bundle, 'bin', 'wombat.cmd'), '@echo off\r\n"%~dp0..\\runtime\\node.exe" "%~dp0..\\lib\\wombat.js" %*\r\n');
    const release: GitHubReleaseMetadata = {
      format: 1, version: metadata.version, source: revision, sourceSha256: receipt.sourceSha256,
      target, runtime: {name: 'node', version: releaseNodeVersion},
    };
    writeFileSync(path.join(bundle, 'release.json'), JSON.stringify(release, null, 2) + '\n');

    const archive = releaseArchive(target);
    run('tar', ['-czf', archive, '-C', path.basename(stage), 'wombat'], output);
    set.assets.push({target, archive, sha256: sha256(path.join(output, archive)), bytes: statSync(path.join(output, archive)).size});
  } finally {
    rmSync(stage, {recursive: true, force: true});
  }
}

set.assets.sort((a, b) => a.target.localeCompare(b.target));
writeFileSync(path.join(output, 'release-set.json'), JSON.stringify(set, null, 2) + '\n');
writeFileSync(path.join(output, 'SHA256SUMS'), set.assets.map(asset => `${asset.sha256}  ${asset.archive}`).join('\n') + '\n');
run(process.execPath, [path.join(root, 'scripts', 'verify-github-release.ts'), '--set', path.join(output, 'release-set.json')], root, true, 1_800_000);
checkBuild(root);
console.log(`GitHub Release candidate: ${output}\n${set.assets.length} platform archive(s), checksums, and clean extraction verified. No upload performed.`);
