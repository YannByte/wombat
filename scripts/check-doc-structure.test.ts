import assert from 'node:assert/strict';
import test from 'node:test';

import { audienceBoundaryErrors, readmePairErrors } from './check-doc-structure.ts';

test('requires English default READMEs without changing other document pairs', () => {
  assert.deepEqual(readmePairErrors([
    { en: 'README.md', zh: 'README.zh-CN.md' },
    { en: 'docs/README.md', zh: 'docs/README.zh-CN.md' },
    { en: 'docs/guide.en.md', zh: 'docs/guide.md' },
  ]), []);
  for (const pair of [
    { en: 'ui/README.en.md', zh: 'ui/README.md' },
    { en: 'ui/README.md', zh: 'ui/README.md' },
    { en: 'ui/README.md', zh: 'web/README.zh-CN.md' },
  ]) assert.match(readmePairErrors([pair])[0], /use English README.md and Chinese README.zh-CN.md in the same directory/);
});

test('keeps internal release operations out of user documentation', () => {
  assert.deepEqual(audienceBoundaryErrors({
    'README.md': '# Wombat\n\nInstall and open Wombat.\n',
    'docs/guides/installation.en.md': '# Install\n\nRun the installer.\n',
    'docs/development/workflow.en.md': '# Workflow\n\ncorepack pnpm release:publish\n',
  }), []);

  const errors = audienceBoundaryErrors({
    'README.md': '# Wombat\n\nSee .agents/skills/wombat-release for release-set.json.\n',
    'docs/guides/installation.en.md': '# Install\n\nRun corepack pnpm github:pack.\n',
  });
  assert.equal(errors.length, 3);
  assert.ok(errors.every(error => error.includes('internal release material')));
});
