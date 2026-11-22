import assert from 'node:assert/strict';
import { mkdir, mkdtemp, readFile, readdir, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { test } from 'node:test';
import { runHarnessTask } from '../src/node/codex/task-trial.js';
import { systemTrialProxy } from '../src/node/codex/trial-network.js';

test('task trial runs four versions against the same inputs and keeps independent checks outside the runner', async () => {
  const root = await mkdtemp(path.join(tmpdir(), 'wombat-task-flow-'));
  const selected = path.join(root, 'selected');
  const output = path.join(root, 'output');
  await mkdir(selected);
  await mkdir(output);
  const manifest = path.join(selected, 'task.json');
  await writeFile(path.join(selected, 'material.txt'), 'Synthetic task material');
  await writeFile(path.join(selected, 'excluded.txt'), 'Excluded source');
  await writeFile(manifest, JSON.stringify({
    task: 'Summarize selected material.', instructions: 'Base instructions.', files: ['material.txt'],
    checks: [{ id: 'required', contains: 'REQUIRED_FRAGMENT' }, { id: 'preserved', contains: 'PRESERVED_FRAGMENT' }],
  }));
  const signal = new AbortController().signal;
  const stages: string[] = [];
  try {
    const result = await runHarnessTask(manifest, output, { model: 'fixture-model', signal }, async (session, _query, _config, invocation, prompt) => {
      const stage = path.basename(path.dirname(session.workspace));
      stages.push(stage);
      assert.equal(invocation.cwd, session.workspace);
      const inputs = path.resolve(session.workspace, '..', 'inputs');
      assert.equal(await readFile(path.join(inputs, 'material.txt'), 'utf8'), 'Synthetic task material');
      assert.ok(!(await readdir(inputs)).includes('excluded.txt'));
      assert.ok(!prompt.includes('REQUIRED_FRAGMENT'));
      if (stage === 'propose') {
        assert.equal(await readFile(path.join(inputs, '_baseline.md'), 'utf8'), 'PRESERVED_FRAGMENT');
        assert.deepEqual(session.outputFields, ['general', 'domain']);
        return { items: [], truncated: false, finalText: JSON.stringify({ general: 'General edit.', domain: 'Domain edit.' }) };
      } else if (stage === 'compose') {
        const compared = await readFile(path.join(inputs, '_candidate-evidence.json'), 'utf8');
        assert.match(compared, /"stage":"general"/);
        assert.match(compared, /"stage":"domain"/);
        assert.doesNotMatch(compared, /REQUIRED_FRAGMENT/);
        assert.deepEqual(session.outputFields, ['instructions', 'rationale']);
        return { items: [], truncated: false, finalText: JSON.stringify({ instructions: 'Composed edit.', rationale: 'Use observed checks.' }) };
      } else {
        if (stage === 'composed') assert.equal(session.developerInstructions, 'Composed edit.');
        const content = stage === 'baseline' ? 'PRESERVED_FRAGMENT' : 'PRESERVED_FRAGMENT REQUIRED_FRAGMENT';
        await writeFile(path.join(session.workspace, 'deliverable.md'), content);
      }
      return { items: [], truncated: false };
    });
    assert.deepEqual(stages, ['baseline', 'propose', 'general', 'domain', 'compose', 'composed']);
    assert.deepEqual(result.results.map(stage => stage.stage), ['baseline', 'general', 'domain', 'composed']);
    assert.equal(result.recommendation, 'review_composed');
    assert.equal(await readFile(path.join(path.dirname(result.report), 'composed.md'), 'utf8'), 'PRESERVED_FRAGMENT REQUIRED_FRAGMENT');
    assert.equal(await readFile(path.join(selected, 'material.txt'), 'utf8'), 'Synthetic task material');
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});

test('model host uses enabled manual proxy settings without routing loopback through the proxy', () => {
  const settings = [
    '  HTTPEnable : 0',
    '  HTTPProxy : unused.example',
    '  HTTPPort : 8080',
    '  HTTPSEnable : 1',
    '  HTTPSProxy : ::1',
    '  HTTPSPort : 7897',
    '  ExceptionsList : <array> {',
    '    0 : *.local',
    '  }',
  ].join('\n');
  assert.deepEqual(systemTrialProxy(settings), {
    HTTPS_PROXY: 'http://[::1]:7897', NO_PROXY: 'localhost,127.0.0.1,::1,*.local',
  });
  assert.deepEqual(systemTrialProxy(settings.replace('7897', '70000')), {});
});
