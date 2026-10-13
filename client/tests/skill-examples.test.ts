import { test } from 'node:test';
import assert from 'node:assert/strict';
import { readFileSync } from 'node:fs';
import { validateAgentRequest } from '../src/index.js';

test('configuration Skill examples use the generated method contract instead of shared CLI flags', () => {
  const text = readFileSync(new URL('../../plugin/skills/wombat/references/configuration.md', import.meta.url), 'utf8');
  const examples = [...text.matchAll(/```json\n([\s\S]*?)\n```/g)];
  assert.ok(examples.length > 0);
  for (const [, body] of examples) {
    const request = JSON.parse(body);
    assert.equal(validateAgentRequest(request), true, JSON.stringify(validateAgentRequest.errors));
    assert.equal(request.method, 'config');
    assert.equal(validateAgentRequest({ ...request, params: { ...request.params, action: 'activity' } }), false);
  }
});
