import assert from 'node:assert/strict';
import { execFileSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { mkdirSync, mkdtempSync, readFileSync, rmSync, statSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { pathToFileURL } from 'node:url';
import { test } from 'node:test';
import { parseUpdateArgs, updateInstalled } from '../src/update-cli.js';

test('update arguments are narrow and deterministic', () => {
  assert.deepEqual(parseUpdateArgs(['--check', '--version', '0.3.0', '--json']), {
    version: '0.3.0',
    check: true,
    json: true,
    help: false,
  });
  for (const args of [['--version'], ['--version', '../bad'], ['--check', '--check'], ['other']])
    assert.throws(() => parseUpdateArgs(args));
});

test('managed installation verifies and atomically selects a downloaded release', async () => {
  const root = mkdtempSync(path.join(os.tmpdir(), 'wombat-update-test-'));
  try {
    const source = 'b'.repeat(40),
      sourceSha256 = 'c'.repeat(64),
      oldSha256 = 'd'.repeat(64),
      target = `${process.platform}-${process.arch}`;
    const install = path.join(root, 'install'),
      versions = path.join(install, 'versions'),
      oldId = `0.3.0-${source.slice(0, 12)}-${oldSha256.slice(0, 12)}`;
    const old = path.join(versions, oldId),
      entry = path.join(old, 'lib', 'wombat.js');
    mkdirSync(path.dirname(entry), { recursive: true });
    writeFileSync(entry, 'old');
    writeFileSync(
      path.join(old, 'release.json'),
      JSON.stringify({
        format: 1,
        version: '0.3.0',
        source,
        sourceSha256: oldSha256,
        target,
        runtime: { name: 'node', version: process.versions.node },
      }),
    );
    writeFileSync(path.join(install, 'current.txt'), oldId + '\n');

    const stage = path.join(root, 'stage', 'wombat');
    mkdirSync(path.join(stage, 'runtime'), { recursive: true });
    mkdirSync(path.join(stage, 'lib'));
    writeFileSync(
      path.join(stage, 'runtime', process.platform === 'win32' ? 'node.exe' : 'node'),
      'runtime',
    );
    writeFileSync(path.join(stage, 'lib', 'wombat.js'), 'new');
    writeFileSync(
      path.join(stage, 'release.json'),
      JSON.stringify({
        format: 1,
        version: '0.3.0',
        source,
        sourceSha256,
        target,
        runtime: { name: 'node', version: process.versions.node },
      }),
    );
    const releases = path.join(root, 'release');
    mkdirSync(releases);
    const archiveName = `wombat-${target}.tar.gz`,
      archive = path.join(releases, archiveName);
    const tar =
      process.platform === 'win32'
        ? path.join(process.env.SystemRoot ?? 'C:\\Windows', 'System32', 'tar.exe')
        : 'tar';
    execFileSync(tar, ['-czf', path.join('release', archiveName), '-C', 'stage', 'wombat'], {
      cwd: root,
    });
    const body = readFileSync(archive),
      sha256 = createHash('sha256').update(body).digest('hex'),
      bytes = statSync(archive).size;
    writeFileSync(
      path.join(releases, 'release-set.json'),
      JSON.stringify({
        format: 1,
        version: '0.3.0',
        source,
        sourceSha256,
        assets: [{ target, archive: archiveName, sha256, bytes }],
      }),
    );
    writeFileSync(path.join(releases, 'SHA256SUMS'), `${sha256}  ${archiveName}\n`);
    const baseUrl = pathToFileURL(releases).href.replace(/\/$/, '');

    const checked = await updateInstalled({
      baseUrl,
      entryFile: entry,
      check: true,
    });
    assert.equal(checked.updated, false);
    assert.equal(checked.updateAvailable, true);
    assert.equal(readFileSync(path.join(install, 'current.txt'), 'utf8').trim(), oldId);
    const wrong = 'e'.repeat(64);
    writeFileSync(
      path.join(releases, 'release-set.json'),
      JSON.stringify({
        format: 1,
        version: '0.3.0',
        source,
        sourceSha256,
        assets: [{ target, archive: archiveName, sha256: wrong, bytes }],
      }),
    );
    writeFileSync(path.join(releases, 'SHA256SUMS'), `${wrong}  ${archiveName}\n`);
    await assert.rejects(updateInstalled({ baseUrl, entryFile: entry }), /checksum|校验和/i);
    assert.equal(readFileSync(path.join(install, 'current.txt'), 'utf8').trim(), oldId);
    writeFileSync(
      path.join(releases, 'release-set.json'),
      JSON.stringify({
        format: 1,
        version: '0.3.0',
        source,
        sourceSha256,
        assets: [{ target, archive: archiveName, sha256, bytes }],
      }),
    );
    writeFileSync(path.join(releases, 'SHA256SUMS'), `${sha256}  ${archiveName}\n`);
    writeFileSync(
      path.join(install, 'service-registration.json'),
      JSON.stringify({ version: 99, enabled: true }),
    );
    const updated = await updateInstalled({ baseUrl, entryFile: entry });
    const nextId = `0.3.0-${source.slice(0, 12)}-${sourceSha256.slice(0, 12)}`;
    assert.equal(updated.updated, true);
    assert.equal(updated.serviceRestart, 'unconfirmed');
    assert.equal(
      JSON.parse(readFileSync(path.join(install, 'service-registration.json'), 'utf8')).version,
      99,
    );
    assert.equal(readFileSync(path.join(install, 'current.txt'), 'utf8').trim(), nextId);
    assert.equal(readFileSync(path.join(versions, nextId, 'lib', 'wombat.js'), 'utf8'), 'new');
    const current = await updateInstalled({
      baseUrl,
      entryFile: path.join(versions, nextId, 'lib', 'wombat.js'),
      check: true,
    });
    assert.equal(current.updateAvailable, false);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test('metadata checks cancel oversized streams before buffering their complete bodies', async () => {
  const root = mkdtempSync(path.join(os.tmpdir(), 'wombat-update-metadata-'));
  const originalFetch = globalThis.fetch;
  try {
    const source = 'b'.repeat(40),
      sourceSha256 = 'c'.repeat(64),
      target = `${process.platform}-${process.arch}`;
    const id = `0.3.0-${source.slice(0, 12)}-${sourceSha256.slice(0, 12)}`,
      install = path.join(root, 'install'),
      old = path.join(install, 'versions', id);
    const entry = path.join(old, 'lib/wombat.js');
    mkdirSync(path.dirname(entry), { recursive: true });
    writeFileSync(entry, 'synthetic');
    writeFileSync(
      path.join(old, 'release.json'),
      JSON.stringify({
        format: 1,
        version: '0.3.0',
        source,
        sourceSha256,
        target,
        runtime: { name: 'node', version: process.versions.node },
      }),
    );
    writeFileSync(path.join(install, 'current.txt'), id + '\n');
    const metadata = JSON.stringify({
      format: 1,
      version: '0.3.0',
      source,
      sourceSha256,
      assets: [
        {
          target,
          archive: `wombat-${target}.tar.gz`,
          sha256: 'a'.repeat(64),
          bytes: 100,
        },
      ],
    });
    const limit = 4 * 1024 * 1024,
      baseUrl = 'https://synthetic.invalid';
    for (const size of [metadata.length, limit]) {
      globalThis.fetch = async () => new Response(metadata.padEnd(size, ' '));
      assert.equal(
        (await updateInstalled({ entryFile: entry, baseUrl, check: true })).updateAvailable,
        false,
      );
    }
    for (const contentLength of [undefined, '1']) {
      let produced = 0,
        cancelled = false;
      globalThis.fetch = async () =>
        new Response(
          new ReadableStream<Uint8Array>({
            pull(controller) {
              if (produced === 2 * limit) {
                controller.close();
                return;
              }
              produced += 65536;
              controller.enqueue(new Uint8Array(65536));
            },
            cancel() {
              cancelled = true;
            },
          }),
          {
            headers: contentLength ? { 'Content-Length': contentLength } : undefined,
          },
        );
      await assert.rejects(
        updateInstalled({ entryFile: entry, baseUrl, check: true }),
        /上限|limit/i,
      );
      assert.ok(produced < 2 * limit, 'The complete oversized response must not be consumed');
      assert.equal(cancelled, true);
    }
    let cancelled = false;
    globalThis.fetch = async () =>
      new Response(
        new ReadableStream({
          start(controller) {
            controller.enqueue(new Uint8Array(limit + 1));
          },
          cancel() {
            cancelled = true;
          },
        }),
      );
    await assert.rejects(
      updateInstalled({ entryFile: entry, baseUrl, check: true }),
      /上限|limit/i,
    );
    assert.equal(cancelled, true);
    const releases = path.join(root, 'release');
    mkdirSync(releases);
    writeFileSync(path.join(releases, 'release-set.json'), metadata.padEnd(limit + 1, ' '));
    await assert.rejects(
      updateInstalled({
        entryFile: entry,
        baseUrl: pathToFileURL(releases).href,
        check: true,
      }),
      /上限|limit/i,
    );
    assert.equal(readFileSync(path.join(install, 'current.txt'), 'utf8').trim(), id);
  } finally {
    globalThis.fetch = originalFetch;
    rmSync(root, { recursive: true, force: true });
  }
});
