import assert from 'node:assert/strict';
import path from 'node:path';
import test from 'node:test';
import {onboardingOptions, commandMentionsExecutable} from './verify-onboarding-options.ts';
const base = ['output-dir', 'release-dir', 'codex-bin'].flatMap(k => ['--'+k, path.resolve(k)]);
test('onboarding accepts complete native paths and an optional installed plugin conversation', () => {
  assert.equal(onboardingOptions(['--', ...base]).agentBin, undefined);
  assert.throws(()=>onboardingOptions([...base, '--agent-bin', path.resolve('codex'), '--agent-skill', 'wombat-collection:wombat']));
});
test('onboarding rejects missing, relative, duplicate, unknown and incomplete agent options', () => {
  for (const args of [[], base.slice(0, -2), [...base, '--unknown', 'x'], [...base, '--codex-bin', path.resolve('other')],
    ['--output-dir', 'relative', ...base.slice(2)], [...base, '--agent-bin', path.resolve('codex')],
    [...base, '--agent-bin', path.resolve('codex'), '--agent-skill', 'other:skill']]) assert.throws(() => onboardingOptions(args));
});
test('native command evidence recognizes quoted Unicode paths and JSON presentation without accepting another installation', () => {
  const executable = path.resolve("安装 & runtime '/bin/wombat"), quoted = "'"+executable.replaceAll("'", "'\\''")+"'";
  for (const value of [executable, quoted, JSON.stringify(quoted).slice(1, -1)])
    assert.equal(commandMentionsExecutable('shell '+value+' usage --json', executable), true);
  assert.equal(commandMentionsExecutable('shell /another/bin/wombat usage --json', executable), false);
  assert.equal(commandMentionsExecutable('cat SKILL.md', executable), false);
});
