import assert from 'node:assert/strict';
import {chmodSync, mkdirSync, mkdtempSync, rmSync, writeFileSync} from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import {collectDoctor, parseDoctorArgs} from '../src/doctor-cli.js';
import packageMetadata from '../package.json' with {type: 'json'};

test('doctor validates a managed installation without scanning sources or using the network', t => {
  const prefix = mkdtempSync(path.join(os.tmpdir(), 'wombat-doctor-'));
  t.after(() => rmSync(prefix, {recursive: true, force: true}));
  const version = packageMetadata.version;
  const id = `${version}-a-b`, payload = path.join(prefix, 'lib', 'wombat', 'versions', id);
  const entry = path.join(payload, 'lib', 'wombat.js'), bin = path.join(prefix, 'bin'), codex = path.join(prefix, 'codex');
  mkdirSync(path.dirname(entry), {recursive: true}); mkdirSync(bin); mkdirSync(codex);
  writeFileSync(entry, ''); writeFileSync(path.join(payload, 'release.json'), JSON.stringify({version}));
  writeFileSync(path.join(prefix, 'lib', 'wombat', 'current.txt'), id);
  const launcher = path.join(bin, process.platform === 'win32' ? 'wombat.cmd' : 'wombat');
  writeFileSync(launcher, '');
  if (process.platform !== 'win32') chmodSync(launcher, 0o755);
  const result = collectDoctor({entryFile: entry, env: {PATH: bin, CODEX_HOME: codex}, resolveCore: () => '/synthetic/core',
    runCore: () => ({status: 0, stdout: `wombat-core ${version}\n`, stderr: ''})});
  assert.equal(result.version, version);
  assert.equal(result.status, 'pass'); assert.equal(result.scannedSources, false); assert.equal(result.networkUsed, false);
  assert.deepEqual(result.checks.map(check => [check.id, check.status]), [['installation','pass'],['path','pass'],['runtime','pass'],['core','pass'],['codex-source','pass']]);
});

test('doctor reports repairable source, PATH, core and Codex warnings or failures', t => {
  const root = mkdtempSync(path.join(os.tmpdir(), 'wombat-doctor-'));
  t.after(() => rmSync(root, {recursive: true, force: true}));
  const result = collectDoctor({entryFile: path.join(root, 'dist', 'wombat.js'), env: {PATH: '', CODEX_HOME: path.join(root, 'missing')},
    resolveCore: () => { throw new Error('missing core'); }});
  assert.equal(result.status, 'fail'); assert.ok(result.checks.every(check => check.message && (check.status === 'pass' || check.repair)));
  assert.equal(parseDoctorArgs(['--json']).json,true);
  assert.equal(parseDoctorArgs(['--project','/synthetic']).request.project,path.resolve('/synthetic'));
  assert.throws(()=>parseDoctorArgs(['--plugin']));
  assert.throws(() => parseDoctorArgs(['--scan']));
});
