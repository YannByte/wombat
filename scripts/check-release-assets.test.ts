import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {cpSync, mkdtempSync, rmSync, writeFileSync} from 'node:fs';
import {spawnSync} from 'node:child_process';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import {verifyReleaseAssets} from './check-release-assets.ts';
import {nativeTargets} from './native-platforms.ts';

test('release helpers load in a clean checkout without development dependencies', t => {
  const directory=mkdtempSync(path.join(os.tmpdir(),'wombat-release-no-install-'));
  t.after(()=>rmSync(directory,{recursive:true,force:true}));
  cpSync(new URL('.',import.meta.url),path.join(directory,'scripts'),{recursive:true});
  writeFileSync(path.join(directory,'package.json'),JSON.stringify({type:'module',version:'1.0.0'}));
  const result=spawnSync(process.execPath,['--input-type=module','--eval',
    "await import('./scripts/check-release-assets.ts'); await import('./scripts/generate-release-notes.ts'); await import('./scripts/ci-release-candidate.ts');"],
  {cwd:directory,encoding:'utf8',timeout:10000,maxBuffer:1024*1024});
  assert.ifError(result.error);
  assert.equal(result.status,0,result.stderr+result.stdout);
});

test('reused candidates retain exact identity and archive hashes without rebuilding', t => {
  const root = mkdtempSync(path.join(os.tmpdir(), 'wombat-reused-assets-'));
  t.after(() => rmSync(root, {recursive: true, force: true}));
  const set = {format: 1, version: '1.0.0', source: 'a'.repeat(40), sourceSha256: 'b'.repeat(64), candidateOnly: false,
    targets: nativeTargets, assets: nativeTargets.map(target => ({target, archive: `wombat-${target}.tar.gz`,
      bytes: Buffer.byteLength(target), sha256: createHash('sha256').update(target).digest('hex')}))};
  for (const asset of set.assets) writeFileSync(path.join(root, asset.archive), asset.target);
  writeFileSync(path.join(root, 'release-set.json'), JSON.stringify(set));
  writeFileSync(path.join(root, 'SHA256SUMS'), set.assets.map(asset => `${asset.sha256}  ${asset.archive}`).join('\n'));
  assert.equal(verifyReleaseAssets(root, set.version, set.source).length, 5);
  assert.throws(() => verifyReleaseAssets(root, '1.0.1', set.source), /identity differs/);
  assert.throws(() => verifyReleaseAssets(root, set.version, 'c'.repeat(40)), /identity differs/);
  const assets = set.assets.map(asset => ({name: asset.archive, size: asset.bytes, digest: `sha256:${asset.sha256}`}));
  for (const asset of set.assets.slice(1)) rmSync(path.join(root, asset.archive));
  const published = {target: nativeTargets[0], assets};
  assert.equal(verifyReleaseAssets(root, set.version, set.source, published).length, 1);
  assert.throws(() => verifyReleaseAssets(root, set.version, set.source), /ENOENT/);
  for (const changes of [{size: 0}, {digest: `sha256:${'c'.repeat(64)}`}]) {
    const drifted = assets.map((asset, index) => index === 1 ? {...asset, ...changes} : asset);
    assert.throws(() => verifyReleaseAssets(root, set.version, set.source, {...published, assets: drifted}), /Published archive differs/);
  }
  assert.throws(() => verifyReleaseAssets(root, set.version, set.source, {...published, assets: assets.slice(1)}), /Published archive differs/);
  assert.throws(() => verifyReleaseAssets(root, set.version, set.source, {...published, assets: [...assets, assets[1]]}), /Published archive differs/);
  writeFileSync(path.join(root, set.assets[0].archive), 'corrupt');
  assert.throws(() => verifyReleaseAssets(root, set.version, set.source, published), /SHA-256 mismatch/);
});
