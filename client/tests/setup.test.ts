import assert from 'node:assert/strict';
import {createHash} from 'node:crypto';
import {mkdtemp, mkdir, writeFile, readFile, rm} from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import {checkSetup} from '../src/node/setup.js';

test('a slow Hook query cannot erase completed discovery and runtime verification', {timeout: 15000}, async t => {
  const root = await mkdtemp(path.join(os.tmpdir(), 'wombat-setup-deadline-'));
  t.after(() => rm(root, {recursive: true, force: true}));
  const skill = path.join(root, 'plugin/skills/wombat'), native = path.join(root, 'native.cjs'), core = path.join(root, 'core.cjs');
  await mkdir(skill, {recursive: true});
  const content = Buffer.from('Synthetic Skill\n'), hash = (value: string | Buffer) => createHash('sha256').update(value).digest('hex');
  await writeFile(path.join(skill, 'SKILL.md'), content, {mode: 0o600});
  const contentHash = hash(JSON.stringify([{path: 'SKILL.md', size: content.length, sha256: hash(content), executable: false}]));
  const skills=[{name:'wombat',contentHash}], skillContentHash=hash(JSON.stringify(skills));
  await writeFile(path.join(root, 'plugin/wombat-runtime.json'), JSON.stringify({format: 1, version: '1.2.3', requiredCapabilities: [], skills, skillContentHash}));
  await writeFile(native, `
if(process.argv.includes('--version')){console.log('codex-cli 0.163.0');process.exit(0);}
let buffer='';process.stdin.on('data',chunk=>{buffer+=chunk;let end;while((end=buffer.indexOf('\\n'))>=0){const message=JSON.parse(buffer.slice(0,end));buffer=buffer.slice(end+1);if(message.id==null)continue;console.log(JSON.stringify({id:message.id,result:message.method==='skills/list'?{data:[{cwd:message.params.cwds[0],errors:[],skills:[{name:'wombat:wombat',path:${JSON.stringify(path.join(skill, 'SKILL.md'))},enabled:true}]}]}:{}}));}});
`);
  // An unrelated configuration probe remains pending beyond setup's deadline.
  await writeFile(core, 'process.stdin.resume();setInterval(()=>{},1000);', {mode: 0o700});
  const basic = await checkSetup({project: root}, {}, {codexBinaryPath: native, binaryPath: path.join(root,'MUST_NOT_START_CORE')});
  assert.equal(basic.hooks,null);
  assert.equal(basic.discovery.status,'available');
  assert.ok(!basic.errorCodes.includes('SETUP_TIMEOUT'));
  const parseNative=path.join(root,'parse-native.cjs');
  await writeFile(parseNative,(await readFile(native,'utf8')).replace('errors:[]','errors:[{path:"/synthetic/other/SKILL.md",message:"PRIVATE_PARSE_MESSAGE"}]'));
  const partial=await checkSetup({project:root},{},{codexBinaryPath:parseNative,binaryPath:path.join(root,'MUST_NOT_START_CORE')});
  assert.equal(partial.discovery.status,'unavailable');
  assert.deepEqual(partial.discovery.parseErrorPaths,['/synthetic/other/SKILL.md']);
  assert.equal(partial.runtimeChecks[0].status,'compatible');
  assert.ok(partial.errorCodes.includes('SKILL_PARSE_ERROR'));
  assert.ok(!JSON.stringify(partial).includes('PRIVATE_PARSE_MESSAGE'));
  const result = await checkSetup({project: root, includeHooks: true}, {}, {codexBinaryPath: native, binaryPath: core});
  assert.equal(result.discovery.status, 'available');
  assert.equal(result.runtimeChecks.length, 1);
  assert.equal(result.runtimeChecks[0].status, 'compatible');
  assert.equal(result.runtimeChecks[0].skillContentHash, skillContentHash);
  assert.ok(result.errorCodes.includes('SETUP_TIMEOUT'));
  assert.ok(!result.errorCodes.includes('CANCELLED'), 'an internal deadline is not user cancellation');
  const controller = new AbortController();
  const cancelled = checkSetup({project: root, includeHooks: true}, {signal: controller.signal}, {codexBinaryPath: native, binaryPath: core});
  const timer = setTimeout(() => controller.abort(), 100);
  try {await assert.rejects(cancelled, {code: 'CANCELLED'});} finally {clearTimeout(timer);}
});
