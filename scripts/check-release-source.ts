import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { consistencyErrors } from './prepare-release.ts';
import { readReleaseNotesInput } from './generate-release-notes.ts';
import {readFileSync} from 'node:fs';
import {actionPinErrors, actionSources, repositorySlug, rootReadmeReleaseErrors} from './release-policy.ts';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
try {
  const errors = consistencyErrors(root);
  readReleaseNotesInput(root);
  const pkg = JSON.parse(readFileSync(path.join(root, 'package.json'), 'utf8')) as {version: string; repository: string | {url: string}};
  const repository = repositorySlug(typeof pkg.repository === 'string' ? pkg.repository : pkg.repository.url);
  errors.push(...rootReadmeReleaseErrors({english: readFileSync(path.join(root, 'README.md'), 'utf8'),
    chinese: readFileSync(path.join(root, 'README.zh-CN.md'), 'utf8')}, pkg.version, repository));
  errors.push(...actionPinErrors(actionSources(root)));
  if (errors.length) throw new Error(errors.join('\n'));
  console.log('Release mirrors and editorial input match the root version source.');
} catch (error) {
  console.error(error instanceof Error ? error.message : error);
  process.exit(1);
}
