import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync, readdirSync } from 'node:fs';
import { validateAgentRequest } from '../src/index.js';

test('Skill Agent examples use generated contracts and reject CLI-only params', () => {
  const directory = new URL('../../plugin/skills/wombat/references/', import.meta.url);
  let checked = 0;
  for (const file of readdirSync(directory).filter(file => file.endsWith('.md'))) {
    const text = readFileSync(new URL(file, directory), 'utf8');
    for (const [, body] of text.matchAll(/```json\n([\s\S]*?)\n```/g)) {
      const request = JSON.parse(body);
      if (!('method' in request)) continue; // Web contexts have a separate generated contract.
      assert.equal(validateAgentRequest(request), true, file+': '+JSON.stringify(validateAgentRequest.errors));
      assert.equal(validateAgentRequest({ ...request, params: { ...request.params, '--json': true } }), false);
      if (request.method === 'config') assert.equal(validateAgentRequest({ ...request, params: { ...request.params, action: 'activity' } }), false);
      checked++;
    }
  }
  assert.ok(checked > 0, 'No executable Agent examples were checked');
});
