import assert from 'node:assert/strict';
import { test } from 'node:test';
import { mkdtempSync, mkdirSync, rmSync, symlinkSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { validateReadinessPaths } from './readiness-benchmark-paths.js';
test('benchmark preparation excludes repositories, source logs and path aliases', () => {
  const root = mkdtempSync(path.join(os.tmpdir(), 'wombat-benchmark-guard-'));
  try {
    const repo = path.join(root, 'repo'),
      source = path.join(root, 'source'),
      output = path.join(root, 'output');
    mkdirSync(repo);
    mkdirSync(source);
    validateReadinessPaths(output, repo, source);
    for (const target of [repo, path.join(repo, 'data'), root, source, path.join(source, 'report')])
      assert.throws(() => validateReadinessPaths(target, repo, source));
    assert.throws(() =>
      validateReadinessPaths(output, repo, path.join(output, 'data', 'sessions')),
    );
    if (process.platform !== 'win32') {
      const alias = path.join(root, 'source-alias');
      symlinkSync(source, alias, 'dir');
      assert.throws(() => validateReadinessPaths(path.join(alias, 'report'), repo, source));
    }
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});
