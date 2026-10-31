import { spawnSync } from 'node:child_process';
import { readFileSync, readdirSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

import { toolCommand } from './run-tool.ts';
import { actionPinErrors, repositorySlug, rootReadmeReleaseErrors } from './release-policy.ts';
import { applyReleaseCopy } from './release-copy.ts';
import { releaseVersion, validateVersion } from './release-version.ts';
import { readReleaseNotesInput } from './generate-release-notes.ts';
export { validateVersion } from './release-version.ts';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');

export const packageFiles = [
  'package.json',
  'client/package.json',
  'cli/package.json',
] as const;

export const releaseTextFiles = [
  'README.md',
  'README.zh-CN.md',
  'docs/guides/installation.md',
  'docs/guides/installation.en.md',
] as const;

const translatedDocs = [
  'README.zh-CN.md',
  'docs/guides/installation.md',
] as const;

function read(projectRoot: string, file: string): string {
  return readFileSync(path.join(projectRoot, file), 'utf8');
}

function packageVersion(projectRoot: string, file: string): string {
  const parsed = JSON.parse(read(projectRoot, file));
  if (typeof parsed.version !== 'string') throw new Error(`${file}: missing string version`);
  return parsed.version;
}

function cargoVersionPattern(file: string): RegExp {
  return file === 'core/Cargo.toml'
    ? /(\[package\][\s\S]*?\nversion = ")([^"]+)("\n)/
    : /(\[\[package\]\]\nname = "wombat-core"\nversion = ")([^"]+)("\n)/;
}

function cargoVersion(content: string, file: string): string {
  const match = content.match(cargoVersionPattern(file));
  if (!match) throw new Error(`${file}: cannot locate wombat-core version`);
  return match[2];
}

function replaceCargoVersion(content: string, version: string, file: string): string {
  return content.replace(cargoVersionPattern(file), `$1${version}$3`);
}

function versionedText(file: string, content: string, version: string): string {
  if (file.startsWith('README')) return applyReleaseCopy(file, content, version);
  const pattern = /^(Wombat `v)([^`]+)(`)/m;
  const match = content.match(pattern);
  if (!match) throw new Error(`${file}: cannot locate current Wombat version`);
  validateVersion(match[2]);
  return content.replace(pattern, `$1${version}$3`);
}

export function prepareVersionFiles(projectRoot: string, version = releaseVersion(projectRoot)): string {
  validateVersion(version);
  const current = packageVersion(projectRoot, 'package.json');
  validateVersion(current);

  const updates = new Map<string, string>();
  for (const file of packageFiles) {
    const parsed = JSON.parse(read(projectRoot, file));
    if (typeof parsed.version !== 'string') throw new Error(`${file}: missing string version`);
    validateVersion(parsed.version);
    parsed.version = version;
    updates.set(file, `${JSON.stringify(parsed, null, 2)}\n`);
  }

  for (const file of ['core/Cargo.toml', 'core/Cargo.lock']) {
    const content = read(projectRoot, file);
    const found = cargoVersion(content, file);
    validateVersion(found);
    updates.set(file, replaceCargoVersion(content, version, file));
  }

  for (const file of releaseTextFiles) {
    const content = read(projectRoot, file);
    updates.set(file, versionedText(file, content, version));
  }

  for (const [file, content] of updates) writeFileSync(path.join(projectRoot, file), content);
  return current;
}

export function consistencyErrors(projectRoot: string): string[] {
  const errors: string[] = [];
  let version = '';
  try {
    version = packageVersion(projectRoot, 'package.json');
    validateVersion(version);
  } catch (error) {
    return [error instanceof Error ? error.message : String(error)];
  }

  for (const file of packageFiles.slice(1)) {
    try {
      const found = packageVersion(projectRoot, file);
      if (found !== version) errors.push(`${file}: expected ${version}, found ${found}`);
    } catch (error) {
      errors.push(error instanceof Error ? error.message : String(error));
    }
  }
  for (const file of ['core/Cargo.toml', 'core/Cargo.lock']) {
    try {
      const found = cargoVersion(read(projectRoot, file), file);
      if (found !== version) errors.push(`${file}: expected ${version}, found ${found}`);
    } catch (error) {
      errors.push(error instanceof Error ? error.message : String(error));
    }
  }
  for (const file of releaseTextFiles) {
    try {
      const content = read(projectRoot, file);
      if (versionedText(file, content, version) !== content) errors.push(`${file}: generated release copy differs from package.json ${version}; run release:prepare`);
    } catch (error) {
      errors.push(error instanceof Error ? error.message : String(error));
    }
  }
  return errors;
}

function run(program: string, args: string[], timeout = 120_000): void {
  const result = spawnSync(...toolCommand(program, args), { cwd: root, stdio: 'inherit', timeout });
  if (result.error || result.status !== 0) {
    throw new Error(`${program} ${args.join(' ')} failed: ${result.error?.message ?? `exit ${result.status}`}`);
  }
}

function checkRepository(): void {
  readReleaseNotesInput(root);
  const errors = consistencyErrors(root);
  const pkg: unknown = JSON.parse(read(root, 'package.json'));
  const packageRecord = typeof pkg === 'object' && pkg !== null && !Array.isArray(pkg) ? pkg : undefined;
  const repositoryValue = packageRecord && 'repository' in packageRecord ? packageRecord.repository : undefined;
  const repositoryUrl = typeof repositoryValue === 'string' ? repositoryValue
    : typeof repositoryValue === 'object' && repositoryValue !== null && 'url' in repositoryValue
      && typeof repositoryValue.url === 'string' ? repositoryValue.url : undefined;
  const version = packageRecord && 'version' in packageRecord && typeof packageRecord.version === 'string'
    ? packageRecord.version : undefined;
  if (!repositoryUrl) errors.push('package.json repository URL is missing');
  if (!version) errors.push('package.json version is missing');
  if (repositoryUrl && version) {
    try {
      const repository = repositorySlug(repositoryUrl);
      errors.push(...rootReadmeReleaseErrors({
        english: read(root, 'README.md'), chinese: read(root, 'README.zh-CN.md'),
      }, version, repository));
    } catch (error) {
      errors.push(error instanceof Error ? error.message : String(error));
    }
  }
  const workflowRoot = path.join(root, '.github/workflows');
  errors.push(...actionPinErrors(Object.fromEntries(
    readdirSync(workflowRoot).filter(file => /\.ya?ml$/.test(file))
      .map(file => [`.github/workflows/${file}`, readFileSync(path.join(workflowRoot, file), 'utf8')]),
  )));
  if (errors.length) throw new Error(errors.join('\n'));
  run('corepack', ['pnpm', 'docs:i18n:check']);
  run('corepack', ['pnpm', 'licenses:check']);
  run('git', ['diff', '--check']);
  console.log(`Release preparation is consistent for ${packageVersion(root, 'package.json')}. No publication performed.`);
}

function usage(): never {
  console.error('Usage: corepack pnpm release:prepare [-- --version <semver> | --check]');
  process.exit(2);
}

function main(): void {
  const args = process.argv.slice(2).filter(arg => arg !== '--');
  if (args.length === 1 && args[0] === '--check') {
    checkRepository();
    return;
  }
  if (args.length && (args.length !== 2 || args[0] !== '--version')) usage();

  const version = args[1] ?? releaseVersion(root);
  const previous = prepareVersionFiles(root, version);
  if (previous === version) {
    console.log(`Release files already use ${version}; refreshing generated records.`);
  } else {
    console.log(`Prepared release version ${previous} -> ${version}.`);
  }
  run('corepack', ['pnpm', 'docs:i18n:record', '--', ...translatedDocs]);
  run('corepack', ['pnpm', 'licenses:generate']);
  checkRepository();
}

const entry = process.argv[1] ? pathToFileURL(path.resolve(process.argv[1])).href : '';
if (entry === import.meta.url) {
  try {
    main();
  } catch (error) {
    console.error(error instanceof Error ? error.message : error);
    process.exit(1);
  }
}
