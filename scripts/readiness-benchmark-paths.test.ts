import assert from 'node:assert/strict';
import { test } from 'node:test';
import { mkdtempSync, mkdirSync, rmSync, symlinkSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { validateReadinessPaths, readinessCliEnvironment } from './readiness-benchmark-paths.js';
test('installed CLI uses normal discovery only for its selected current core', () => {
  const root = mkdtempSync(path.join(os.tmpdir(), 'wombat-readiness-entry-'));
  try {
    const lib = path.join(root, 'versions/0.3.6-test/lib');
    mkdirSync(lib,{recursive:true});
    const cli = path.join(lib, 'wombat.js'), core = path.join(lib, process.platform === 'win32' ? 'wombat-core.exe' : 'wombat-core');
    for (const file of [core, cli]) writeFileSync(file,'synthetic');
    const env = {WOMBAT_CORE_BIN:core,CODEX_HOME:'/synthetic/source',WOMBAT_ENDPOINT_FILE:'/synthetic/endpoint'};
    assert.deepEqual(readinessCliEnvironment(core,cli,env),env);
    writeFileSync(path.join(root,'.managed-by-wombat'),'managed GitHub installation\n');
    writeFileSync(path.join(root,'current.txt'),'0.3.6-test\n');
    const normal = readinessCliEnvironment(core,cli,env);
    assert.equal(normal.WOMBAT_CORE_BIN,undefined);
    assert.equal(normal.WOMBAT_INSTALL_ROOT,root);
    assert.equal(normal.CODEX_HOME,env.CODEX_HOME);
    assert.equal(normal.WOMBAT_ENDPOINT_FILE,env.WOMBAT_ENDPOINT_FILE);
    const other = path.join(root,'other-core');writeFileSync(other,'synthetic');
    assert.deepEqual(readinessCliEnvironment(other,cli,env),env);
    writeFileSync(path.join(root,'current.txt'),'../untrusted');
    assert.deepEqual(readinessCliEnvironment(core,cli,env),env);
  } finally {rmSync(root,{recursive:true,force:true});}
});
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
