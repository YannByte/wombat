import test from 'node:test';
import assert from 'node:assert/strict';
import { inspectPair, recordDifferences } from './doc-pairing.mjs';
import { findCopy, dictionaryErrors } from './check-copy.ts';
const pair = { zh: 'docs/a.md', en: 'docs/a.en.md', record: 'docs/a.i18n.json' };
const pairs = new Map([[pair.zh, pair], [pair.en, pair]]);
const zh = '# 标题\n\n中文 | [English](a.en.md)\n\n## 一\n\n说明。\n\n## 二\n\n另一个段落。\n';
const en = '# Title\n\nEnglish | [中文](a.md)\n\n## One\n\nDescription.\n\n## Two\n\nAnother paragraph.\n';
const inspect = (a = zh, b = en) => inspectPair(pair, a, b, pairs);
test('reports only the changed section and language; independent sections retain hashes', () => {
  const old = inspect(), next = inspect(zh.replace('说明。', '新说明。'));
  assert.deepEqual(old.errors, []);
  assert.deepEqual(recordDifferences(old.record, next.record), ['/title/one: zh changed since confirmation']);
  assert.deepEqual(old.record.sections['/title/two'], next.record.sections['/title/two']);
});
test('matching code edits do not invalidate translation confirmation; asymmetric code does', () => {
  const code = '\n```ts\nconst n = 1;\n```\n', next = code.replace('1', '2');
  const before = inspect(zh + code, en + code), after = inspect(zh + next, en + next);
  assert.deepEqual(after.errors, []);
  assert.deepEqual(before.record, after.record);
  assert.ok(inspect(zh + code, en + next).errors.length);
});
test('checks table shapes, nested list structure and ordered list starts', () => {
  for (const [a,b] of [
    ['\n| A | B |\n|---|---|\n| C | D |\n','\n| A |\n|---|\n| C |\n'],
    ['\n- A\n  - B\n','\n- A\n- B\n'],
    ['\n3. A\n4. B\n','\n1. A\n2. B\n'],
  ]) assert.ok(inspect(zh + a, en + b).errors.length);
});
test('reference links preserve locale and exact query/fragment suffixes', () => {
  assert.deepEqual(inspect(zh + '\n[链接][x]\n\n[x]: a.md?q=1#part\n', en + '\n[Link][x]\n\n[x]: a.en.md?q=1#part\n').errors, []);
  assert.ok(inspect(zh + '\n[链接](a.md?q=1#part)\n', en + '\n[Link](a.en.md?q=2#part)\n').errors.length);
  assert.ok(inspect(zh, en + '\n[Link](a.md)\n').errors.some(error => error.includes('other language')));
});
test('copy check excludes comments and detects literals/templates plus English label prose', () => {
  assert.deepEqual(findCopy('view.ts', '// 中文注释\nfunction title() { return t("common.title"); }'), []);
  assert.equal(findCopy('cli.ts', 'const a = "用量"; const b = `第 ${n} 页`; const c = {label: a, title:b, message:"Show usage"};').length, 3);
  assert.deepEqual(dictionaryErrors({ greeting: '你好 {name}' }, { greeting: 'Hi {user}' }), ['greeting: interpolation parameters differ']);
  assert.ok(dictionaryErrors({ key: '文本' }, {}).length);
});

test('unclosed fences and translations captured at module load fail checks', () => {
  assert.ok(inspect(zh + '\n```ts\nx\n', en + '\n```ts\nx\n').errors.some(error => error.includes('unclosed')));
  assert.ok(findCopy('view.ts', 'const title = t("common.title");').some(error => error.includes('initialization')));
});

test('the shipped pairing check includes GitHub community pages and narrow template exclusions', async t => {
  const {copyFileSync, mkdirSync, mkdtempSync, rmSync, symlinkSync, writeFileSync} = await import('node:fs');
  const {spawnSync} = await import('node:child_process');
  const path = await import('node:path');
  const os = await import('node:os');
  const {fileURLToPath} = await import('node:url');
  const root = mkdtempSync(path.join(os.tmpdir(), 'wombat-community-pairing-'));
  t.after(() => rmSync(root, {recursive: true, force: true}));
  for (const directory of ['scripts', 'docs/i18n/records', 'core', 'client', 'cli', 'ui', 'web', '.github']) mkdirSync(path.join(root, directory), {recursive: true});
  symlinkSync(fileURLToPath(new URL('../node_modules', import.meta.url)), path.join(root, 'node_modules'), process.platform === 'win32' ? 'junction' : 'dir');
  for (const file of ['check-doc-i18n.mjs', 'doc-pairing.mjs']) copyFileSync(new URL(file, import.meta.url), path.join(root, 'scripts', file));
  const community = {zh: '.github/CONTRIBUTING.zh-CN.md', en: '.github/CONTRIBUTING.md', record: 'docs/i18n/records/CONTRIBUTING.i18n.json'};
  const chinese = '# 贡献\n\n中文 | [English](CONTRIBUTING.md)\n\n使用合成数据。\n';
  const english = '# Contributing\n\nEnglish | [中文](CONTRIBUTING.zh-CN.md)\n\nUse synthetic data.\n';
  const inspected = inspectPair(community, chinese, english, new Map([[community.zh, community], [community.en, community]]));
  assert.deepEqual(inspected.errors, []);
  writeFileSync(path.join(root, community.zh), chinese); writeFileSync(path.join(root, community.en), english);
  writeFileSync(path.join(root, community.record), JSON.stringify(inspected.record, null, 2) + '\n');
  writeFileSync(path.join(root, '.github/pull_request_template.md'), 'Change / 改动\n');
  writeFileSync(path.join(root, 'scripts/doc-i18n.manifest.json'), JSON.stringify({pairs: [community], legacyUnpaired: [], excluded: ['.github/pull_request_template.md']}));
  const run = () => spawnSync(process.execPath, [path.join(root, 'scripts/check-doc-i18n.mjs')], {encoding: 'utf8', timeout: 10000, maxBuffer: 65536});
  const valid = run(); assert.ifError(valid.error); assert.equal(valid.status, 0, valid.stderr);
  writeFileSync(path.join(root, '.github/unpaired.md'), '# Missing translation\n');
  const invalid = run(); assert.equal(invalid.status, 1); assert.match(invalid.stderr, /\.github\/unpaired.md: add a complete bilingual pair/);
});
