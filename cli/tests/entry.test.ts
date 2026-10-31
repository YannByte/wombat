import test from 'node:test';
import assert from 'node:assert/strict';
import { parseUsageArgs, usageHelp } from '../src/usage-app-cli.js';

test('default entry is the same usage query as the explicit command', () => {
  assert.deepEqual(parseUsageArgs([]), parseUsageArgs(['usage']));
  assert.deepEqual(parseUsageArgs(['--json']), parseUsageArgs(['usage', '--json']));
  assert.doesNotMatch(usageHelp(), /wombat web/);
});
