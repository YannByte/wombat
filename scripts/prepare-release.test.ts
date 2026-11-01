import assert from 'node:assert/strict';
import { mkdirSync, mkdtempSync, readFileSync, rmSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import {applyReleaseCopy} from './release-copy.ts';

import {
  consistencyErrors,
  packageFiles,
  prepareVersionFiles,
  releaseTextFiles,
  validateVersion,
} from './prepare-release.ts';

function fixture(): string {
  const root = mkdtempSync(path.join(os.tmpdir(), 'wombat-release-'));
  for (const file of packageFiles) {
    mkdirSync(path.dirname(path.join(root, file)), { recursive: true });
    writeFileSync(path.join(root, file), `${JSON.stringify({ name: file, version: '0.1.0-dev.1' }, null, 2)}\n`);
  }
  mkdirSync(path.join(root, 'core'), { recursive: true });
  writeFileSync(path.join(root, 'core/Cargo.toml'), '[package]\nname = "wombat-core"\nversion = "0.1.0-dev.1"\n');
  writeFileSync(path.join(root, 'core/Cargo.lock'), '[[package]]\nname = "another-package"\nversion = "0.1.0-dev.1"\n\n[[package]]\nname = "wombat-core"\nversion = "0.1.0-dev.1"\n');
  for (const file of releaseTextFiles) {
    mkdirSync(path.dirname(path.join(root, file)), { recursive: true });
    const chinese = file === 'README.zh-CN.md';
    writeFileSync(path.join(root, file), file.startsWith('README')
      ? `**${chinese ? '开发者预览版：' : 'Development Preview: '}[\`v0.1.0-dev.1\`](https://github.com/YannByte/wombat/releases/tag/v0.1.0-dev.1)${chinese ? '。' : '.'}**\n\ncurl -fsSL https://raw.githubusercontent.com/YannByte/wombat/main/install.sh | sh -s -- --version 0.1.0-dev.1\n& ([scriptblock]::Create((irm https://raw.githubusercontent.com/YannByte/wombat/main/install.ps1))) -Version 0.1.0-dev.1\n\n### ${chinese ? '更新 Wombat' : 'Update Wombat'}\n\nold\n\n## Uses\n`
      : 'Wombat `v0.1.0-dev.1` supports these platforms. Node 26.4.0 remains independent.\n');
  }
  return root;
}

test('prepares every release version surface and passes consistency checks', t => {
  const root = fixture();
  t.after(() => rmSync(root, { recursive: true, force: true }));
  assert.equal(prepareVersionFiles(root, '0.1.0-dev.2'), '0.1.0-dev.1');
  assert.deepEqual(consistencyErrors(root), []);
  for (const file of [...packageFiles, ...releaseTextFiles, 'core/Cargo.toml']) {
    assert.doesNotMatch(readFileSync(path.join(root, file), 'utf8'), /0\.1\.0-dev\.1/);
  }
  const lock = readFileSync(path.join(root, 'core/Cargo.lock'), 'utf8');
  assert.match(lock, /name = "another-package"\nversion = "0\.1\.0-dev\.1"/);
  assert.match(lock, /name = "wombat-core"\nversion = "0\.1\.0-dev\.2"/);
});

test('rejects malformed versions and incomplete release surfaces before writing', t => {
  for (const version of ['v0.1.0', '01.0.0', '0.1', '0.1.0+build']) {
    assert.throws(() => validateVersion(version), /Invalid release version/);
  }
  const root = fixture();
  t.after(() => rmSync(root, { recursive: true, force: true }));
  writeFileSync(path.join(root, 'README.md'), 'missing current version\n');
  assert.throws(() => prepareVersionFiles(root, '0.1.0-dev.2'), /README\.md/);
  assert.equal(JSON.parse(readFileSync(path.join(root, 'package.json'), 'utf8')).version, '0.1.0-dev.1');

  const driftedRoot = fixture();
  t.after(() => rmSync(driftedRoot, { recursive: true, force: true }));
  writeFileSync(path.join(driftedRoot, 'core/Cargo.toml'), '[package]\nname = "wombat-core"\n');
  assert.throws(() => prepareVersionFiles(driftedRoot, '0.1.0-dev.2'), /core\/Cargo\.toml: cannot locate/);
  assert.equal(JSON.parse(readFileSync(path.join(driftedRoot, 'package.json'), 'utf8')).version, '0.1.0-dev.1');
});

test('the root manifest repairs partially synchronized mirrors and preparation is idempotent', t => {
  const root = fixture();
  t.after(() => rmSync(root, {recursive: true, force: true}));
  writeFileSync(path.join(root, 'package.json'), JSON.stringify({name: 'wombat', version: '0.2.0-beta.1'}));
  writeFileSync(path.join(root, 'cli/package.json'), JSON.stringify({name: '@wombat/cli', version: '0.1.1'}));
  writeFileSync(path.join(root, 'core/Cargo.toml'), '[package]\nname = "wombat-core"\nversion = "0.1.0-dev.0"\n');
  assert.ok(consistencyErrors(root).length);
  prepareVersionFiles(root);
  assert.deepEqual(consistencyErrors(root), []);
  const files = [...packageFiles, ...releaseTextFiles, 'core/Cargo.toml', 'core/Cargo.lock'];
  const before = files.map(file => readFileSync(path.join(root, file), 'utf8'));
  prepareVersionFiles(root);
  assert.deepEqual(files.map(file => readFileSync(path.join(root, file), 'utf8')), before);
  const readme = readFileSync(path.join(root, 'README.md'), 'utf8');
  assert.match(readme, /\*\*Beta:/);
  assert.match(readme, /--version 0\.2\.0-beta\.1 --plugin/);
  assert.match(readme, /-Version 0\.2\.0-beta\.1 -Plugin/);
  assert.doesNotMatch(readme, /wombat system update/);
  assert.match(readme, /runtime-only updates/);
  assert.match(readFileSync(path.join(root, 'docs/guides/installation.en.md'), 'utf8'), /Node 26\.4\.0/);
});

test('release preparation leaves proposal and historical acceptance text unchanged', t => {
  const root = fixture();
  t.after(() => rmSync(root, { recursive: true, force: true }));
  const proposal = path.join(root, 'docs/decisions/proposed/product/acceptance.md');
  const evidence = path.join(root, 'docs/benchmarks/acceptance.json');
  const retiredStatus = path.join(root, 'docs/project/status.md');
  const retained = 'Acceptance evidence for 0.1.0-dev.1 must not become evidence for a new release.\n';
  for (const file of [proposal, evidence, retiredStatus]) {
    mkdirSync(path.dirname(file), { recursive: true });
    writeFileSync(file, retained);
  }
  prepareVersionFiles(root, '0.1.0-dev.2');
  assert.deepEqual(consistencyErrors(root), []);
  for (const file of [proposal, evidence, retiredStatus]) {
    assert.equal(readFileSync(file, 'utf8'), retained);
  }
});

test('promotes beta user copy to stable installation and update instructions', t => {
  const root = fixture();
  t.after(() => rmSync(root, { recursive: true, force: true }));
  writeFileSync(path.join(root, 'README.md'), `## Get started\n\n**Beta: [\`v0.1.0-dev.1\`](https://github.com/YannByte/wombat/releases/tag/v0.1.0-dev.1).** Beta.\n\ncurl -fsSL https://raw.githubusercontent.com/YannByte/wombat/main/install.sh | sh -s -- --version 0.1.0-dev.1\n& ([scriptblock]::Create((irm https://raw.githubusercontent.com/YannByte/wombat/main/install.ps1))) -Version 0.1.0-dev.1\n\n### Update a pre-release\n\nold\n\n## Uses\n`);
  writeFileSync(path.join(root, 'README.zh-CN.md'), `## 开始使用\n\n**Beta 测试版：[\`v0.1.0-dev.1\`](https://github.com/YannByte/wombat/releases/tag/v0.1.0-dev.1)。** Beta。\n\ncurl -fsSL https://raw.githubusercontent.com/YannByte/wombat/main/install.sh | sh -s -- --version 0.1.0-dev.1\n& ([scriptblock]::Create((irm https://raw.githubusercontent.com/YannByte/wombat/main/install.ps1))) -Version 0.1.0-dev.1\n\n### 更新预发行版本\n\n旧文案\n\n## 用途\n`);
  prepareVersionFiles(root, '0.1.0');
  const english = readFileSync(path.join(root, 'README.md'), 'utf8');
  const chinese = readFileSync(path.join(root, 'README.zh-CN.md'), 'utf8');
  assert.match(english, /\*\*Stable:/); assert.match(chinese, /\*\*正式版：/);
  assert.match(english, /install\.sh \| sh -s -- --plugin\n/); assert.doesNotMatch(english, /--version 0\.1\.0/);
  assert.match(english, /install\.ps1\)\)\) -Plugin/); assert.match(chinese, /install\.ps1\)\)\) -Plugin/);
  assert.doesNotMatch(english, /wombat system update/);
  assert.doesNotMatch(chinese, /wombat system update/);
  assert.match(english, /runtime-only updates/);
  assert.match(chinese, /仅更新运行时/);
  assert.doesNotMatch(english, /Update a pre-release|\nold\n/);
  assert.doesNotMatch(chinese, /更新预发行版本|旧文案/);
  prepareVersionFiles(root, '0.1.0');
  assert.equal(readFileSync(path.join(root, 'README.md'), 'utf8'), english);
  assert.equal(readFileSync(path.join(root, 'README.zh-CN.md'), 'utf8'), chinese);
});

test('release copy keeps relocated installer URLs through preview and stable preparation', t => {
  const root = fixture();t.after(()=>rmSync(root,{recursive:true,force:true}));
  for(const file of ['README.md','README.zh-CN.md'])writeFileSync(path.join(root,file),readFileSync(path.join(root,file),'utf8').replaceAll('/main/install.','/main/scripts/install/install.'));
  prepareVersionFiles(root,'0.2.0-beta.1');
  const preview=readFileSync(path.join(root,'README.md'),'utf8');
  assert.match(preview,/main\/scripts\/install\/install\.sh \| sh -s -- --version 0\.2\.0-beta\.1 --plugin/);
  prepareVersionFiles(root,'0.2.0');
  assert.match(readFileSync(path.join(root,'README.md'),'utf8'),/main\/scripts\/install\/install\.sh \| sh -s -- --plugin/);
});

test('release copy preserves installer fences and plugin update guidance in the question-led README', () => {
  for(const file of ['README.md','README.zh-CN.md']){
    const chinese=file.endsWith('zh-CN.md');
    const original=`**${chinese?'正式版：':'Stable: '}[\`v0.3.0\`](https://github.com/YannByte/wombat/releases/tag/v0.3.0)${chinese?'。':'.'}**\n\n\`\`\`sh\ncurl -fsSL https://raw.githubusercontent.com/YannByte/wombat/main/scripts/install/install.sh | sh -s -- --plugin\n\`\`\`\n\n\`\`\`powershell\n& ([scriptblock]::Create((irm https://raw.githubusercontent.com/YannByte/wombat/main/scripts/install/install.ps1))) -Plugin\n\`\`\`\n\n## ${chinese?'更新':'Update'}\n\nOld update copy\n\n## ${chinese?'数据与使用范围':'Data and scope'}\n\nRetained scope.\n`;
    const preview=applyReleaseCopy(file,original,'0.4.0-beta.1');
    assert.match(preview,/```sh\ncurl[^\n]+--version 0\.4\.0-beta\.1 --plugin\n```/);
    assert.match(preview,/```powershell\n& [^\n]+-Version 0\.4\.0-beta\.1 -Plugin\n```/);
    assert.doesNotMatch(preview,/wombat system update/);
    assert.match(preview,chinese?/更新选项和仅更新运行时/:/update options and runtime-only updates/);
    const stable=applyReleaseCopy(file,preview,'0.4.0');
    assert.doesNotMatch(stable,/--version|-Version|Old update copy/);
    assert.match(stable,chinese?/仅更新运行时/:/runtime-only updates/);
    assert.match(stable,chinese?/可以更新 Wombat 与 Codex 插件/:/update Wombat and its Codex plugin/);
    assert.ok(stable.endsWith('Retained scope.\n'));
    assert.equal(applyReleaseCopy(file,stable,'0.4.0'),stable);
    assert.throws(()=>applyReleaseCopy(file,original.replace(/^## (更新|Update)$/m,'Update text'),'0.4.0'),/cannot locate release copy section/);
  }
});
