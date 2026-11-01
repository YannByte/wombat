import test from 'node:test';
import assert from 'node:assert/strict';
import { parseUsageArgs, runUsageCli, usageHelp } from '../src/usage-app-cli.js';

test('default entry is the same usage query as the explicit command', () => {
  assert.deepEqual(parseUsageArgs([]), parseUsageArgs(['usage']));
  assert.deepEqual(parseUsageArgs(['--json']), parseUsageArgs(['usage', '--json']));
  assert.doesNotMatch(usageHelp(), /wombat web/);
});

test('help discovers every supported command without duplicates or removed hosts', async () => {
  const chunks: string[] = [], write = process.stdout.write;
  process.stdout.write = ((text: string) => { chunks.push(String(text)); return true; }) as typeof process.stdout.write;
  try {
    assert.equal(await runUsageCli(['--lang', 'en', '--help', '--json']), 0);
    const result = JSON.parse(chunks.join(''));
    const expected = ['usage','check','system','api','call'];
    assert.deepEqual([...result.commands].sort(), expected.sort());
    assert.match(result.help, /^Wombat\n\nUsage/);
    for (const command of expected) assert.match(result.help, new RegExp('wombat '+command+'\\b'));
  } finally { process.stdout.write = write; }
});

test('removed and unknown commands use the ordinary invalid-argument envelope', async () => {
  const write = process.stdout.write;
  try {
    for (const command of ['web', 'skill', '__proto__']) {
      const chunks: string[] = [];
      process.stdout.write = ((text: string) => { chunks.push(String(text)); return true; }) as typeof process.stdout.write;
      assert.equal(await runUsageCli([command, '--json']), 1);
      const result = JSON.parse(chunks.join(''));
      assert.equal(result.outputVersion, 3);
      assert.equal(result.error.code, 'INVALID_ARGUMENT');
    }
  } finally { process.stdout.write = write; }
});
