import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { copyFileSync, mkdirSync, mkdtempSync, rmSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { fileURLToPath } from 'node:url';

test('accepts the bilingual decision index without exempting misplaced notes', t => {
  const root = mkdtempSync(path.join(os.tmpdir(), 'wombat-decisions-'));
  t.after(() => rmSync(root, { recursive: true, force: true }));
  mkdirSync(path.join(root, 'scripts'));
  mkdirSync(path.join(root, 'docs/decisions/implemented/process'), { recursive: true });
  copyFileSync(fileURLToPath(new URL('./check-decisions.mjs', import.meta.url)), path.join(root, 'scripts/check-decisions.mjs'));
  writeFileSync(path.join(root, 'docs/decisions/README.md'), '# Decision notes\n');
  writeFileSync(path.join(root, 'docs/decisions/README.zh-CN.md'), '# 决策记录\n');
  writeFileSync(path.join(root, 'docs/decisions/implemented/process/2026-10-11-example.md'),
    '# 决策记录：示例\n\nStatus: implemented\n\n## 问题\n\n## 决定\n\n## 考虑过的方案\n\n## 影响与验证\n');
  const run = () => {
    const result = spawnSync(process.execPath, [path.join(root, 'scripts/check-decisions.mjs')], {
      encoding: 'utf8', timeout: 10_000, maxBuffer: 64 * 1024,
    });
    assert.ifError(result.error);
    return result;
  };
  const valid = run();
  assert.equal(valid.status, 0, valid.stderr);
  assert.match(valid.stdout, /1 active note/);
  writeFileSync(path.join(root, 'docs/decisions/implemented/process/README.zh-CN.md'), '# Misplaced index\n');
  const invalid = run();
  assert.equal(invalid.status, 1);
  assert.match(invalid.stderr, /implemented\/process\/README.zh-CN.md: invalid lifecycle, class, or dated filename/);
});
