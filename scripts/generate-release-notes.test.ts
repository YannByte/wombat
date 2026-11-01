import assert from 'node:assert/strict';
import test from 'node:test';
import {nativeTargets} from './native-platforms.ts';
import {parseReleaseNotesArgs, renderReleaseNotes, validateReleaseSet, validateReleaseNotesInput} from './generate-release-notes.ts';

const set = {
  format: 1 as const, version: '0.1.0', source: 'a'.repeat(40), sourceSha256: 'b'.repeat(64), candidateOnly: false,
  targets: [...nativeTargets], assets: nativeTargets.map(target => ({target, archive: `wombat-${target}.tar.gz`, sha256: 'c'.repeat(64), bytes: 10})),
};
test('release notes accept package-script argument forwarding and reject missing or unknown options', () => {
  const args = ['--set', 'release-set.json', '--output', 'notes.md'];
  for (const argv of [args, ['--', ...args]]) {
    assert.deepEqual(parseReleaseNotesArgs(argv), {set: 'release-set.json', output: 'notes.md'});
  }
  assert.throws(() => parseReleaseNotesArgs(['--', '--set', 'release-set.json']), /Usage:/);
  assert.throws(() => parseReleaseNotesArgs(['--', ...args, '--unknown']), /Unknown option/);
});
test('renders complete English stable release notes from verified release facts', () => {
  const notes = renderReleaseNotes({summary: 'Stable summary.', highlights: ['One'], knownLimitations: ['Limit']}, set, 'owner/repo', 'v0.1.0-beta.1');
  assert.match(notes, /# Wombat v0\.1\.0/); assert.match(notes, /install\.sh \| sh\n/);
  assert.match(notes, /wombat system update --check\n+wombat system update/); assert.match(notes, /compare\/v0\.1\.0-beta\.1\.\.\.v0\.1\.0/);
  assert.doesNotMatch(notes, /--version 0\.1\.0/);
});
test('rejects incomplete target sets, version copies and malformed editorial input', () => {
  assert.throws(() => validateReleaseSet({...set, assets: set.assets.slice(1)}), /one archive/);
  const invalidTarget = JSON.parse(JSON.stringify(set)) as typeof set;
  Object.assign(invalidTarget.assets[0], {target: '../escape', archive: 'wombat-../escape.tar.gz'});
  assert.throws(() => validateReleaseSet(invalidTarget), /Release asset is invalid/);
  for (const input of [{version: '0.1.1', summary: 'x', highlights: ['x'], knownLimitations: ['x']},
    {summary: 'x', highlights: [null], knownLimitations: ['x']},
    {summary: 'TODO', highlights: ['x'], knownLimitations: ['x']}, null]) {
    assert.throws(() => validateReleaseNotesInput(input), /Release notes/);
  }
});
