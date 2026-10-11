import assert from 'node:assert/strict';
import {existsSync, mkdirSync, mkdtempSync, readFileSync, realpathSync, rmSync, symlinkSync, writeFileSync} from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import {test} from 'node:test';
import {prepareFirstInstall} from './first-install-state.ts';

test('preparation deletes historical indexes, handling records and versions before installation while retaining sources', () => {
  const root = mkdtempSync(path.join(os.tmpdir(), 'wombat-first-install-'));
  try {
    const data = path.join(root, 'data'), installation = path.join(root, 'prefix');
    mkdirSync(path.join(data, 'user-v1'), {recursive: true});
    mkdirSync(path.join(installation, 'lib/wombat/versions/old'), {recursive: true});
    writeFileSync(path.join(data, 'index.sqlite'), 'historical-index');
    writeFileSync(path.join(data, 'user-v1/reviews.sqlite3'), 'historical-decisions');
    writeFileSync(path.join(installation, 'lib/wombat/versions/old/runtime'), 'old-version');
    const source = path.join(root, 'source.jsonl'); writeFileSync(source, 'source-kept');
    assert.deepEqual(prepareFirstInstall(root, [data, installation]),
      {clearedPaths: [path.join(realpathSync(root), 'data'), path.join(realpathSync(root), 'prefix')], fresh: true});
    assert(!existsSync(data)); assert(!existsSync(installation));
    assert.equal(readFileSync(source, 'utf8'), 'source-kept');
    assert.equal(prepareFirstInstall(root, [data, installation]).fresh, true);
  } finally {rmSync(root, {recursive: true, force: true});}
});

test('invalid or redirected cleanup targets fail before any valid target is deleted', () => {
  const root = mkdtempSync(path.join(os.tmpdir(), 'wombat-first-install-'));
  const outside = mkdtempSync(path.join(os.tmpdir(), 'wombat-first-install-outside-'));
  try {
    const data = path.join(root, 'data'); mkdirSync(data);
    writeFileSync(path.join(data, 'retained'), 'retained');
    for (const target of [root, outside, path.join(root, '../escape'), 'relative']) {
      assert.throws(() => prepareFirstInstall(root, [data, target]));
      assert(existsSync(path.join(data, 'retained')));
    }
    const link = path.join(root, 'link'); symlinkSync(outside, link, process.platform === 'win32' ? 'junction' : 'dir');
    assert.throws(() => prepareFirstInstall(root, [data, path.join(link, 'data')]));
    assert(existsSync(path.join(data, 'retained')));
    const dangling = path.join(root, 'dangling');
    symlinkSync(path.join(outside, 'missing'), dangling, process.platform === 'win32' ? 'junction' : 'dir');
    assert.throws(() => prepareFirstInstall(root, [data, dangling]));
    assert(existsSync(path.join(data, 'retained')));
  } finally {rmSync(root, {recursive: true, force: true}); rmSync(outside, {recursive: true, force: true});}
});
