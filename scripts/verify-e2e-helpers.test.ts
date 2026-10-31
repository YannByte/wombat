import assert from 'node:assert/strict';
import { execFileSync, spawn } from 'node:child_process';
import {once} from 'node:events';
import { existsSync, mkdtempSync, mkdirSync, readFileSync, rmSync, symlinkSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';
import { sourceIdentity } from './build-identity.ts';
import {
  acceptanceInputIdentity, assertExternalOutputDir, currentArtifactIdentity, isWithin, parseVerifyArgs, plannedStages, reusableStage, runBoundedCommand, runFingerprint, terminateTree,
} from './verify-e2e-helpers.ts';

test('verification accepts explicit native scopes and rejects removed browser options', () => {
  const outputDir = path.join(os.tmpdir(), 'wombat-e2e');
  assert.deepEqual(parseVerifyArgs(['--', '--scope', 'integration', '--output-dir', outputDir, '--resume']), {scope:'integration',outputDir,resume:true});
  assert.equal(parseVerifyArgs(['--output-dir', outputDir]).scope, 'all');
  for (const args of [[], ['--output-dir','relative'], ['--output-dir',outputDir,'--scope','browser'], ['--output-dir',outputDir,'--playwright-module','/tmp/pw'], ['--output-dir',outputDir,'--resume','--resume']]) assert.throws(()=>parseVerifyArgs(args));
  assert.deepEqual(plannedStages('integration'), ['integration']);
  assert.deepEqual(plannedStages('e2e'), ['e2e']);
  assert.deepEqual(plannedStages('all'), ['integration','e2e']);
  assert.equal(isWithin('/repo','/repo/out'), true);
  assert.equal(isWithin('/repo','/repo2/out'), false);
  assert.throws(()=>assertExternalOutputDir('/repo','/repo/out'), /outside/);
});
test('resume binds the current source, tests, artifact and native scope', () => {
  const base={sourceSha256:'a',artifactSha256:'b',acceptanceInputsSha256:'c',scope:'all' as const};
  const key=runFingerprint(base), prior={runKey:key,stages:[{name:'integration',status:'passed'},{name:'e2e',status:'failed'}]};
  assert.equal(reusableStage(prior,'integration',key),true);
  assert.equal(reusableStage(prior,'e2e',key),false);
  for(const change of [{...base,sourceSha256:'d'},{...base,artifactSha256:'d'},{...base,acceptanceInputsSha256:'d'},{...base,scope:'e2e' as const}]) assert.equal(reusableStage(prior,'integration',runFingerprint(change)),false);
});

test('acceptance fixture identity changes when a test or referenced Skill asset changes', () => {
  const temp = mkdtempSync(path.join(os.tmpdir(), 'wombat-e2e-inputs-'));
  try {
    for (const directory of ['tests/integration', 'tests/e2e', 'tests/helpers', 'plugin/skills/wombat']) mkdirSync(path.join(temp, directory), { recursive: true });
    const testFile = path.join(temp, 'tests/e2e/example.test.ts'), fixture = path.join(temp, 'plugin/skills/wombat/SKILL.md');
    writeFileSync(testFile, 'assert.equal(1, 1)'); writeFileSync(fixture, 'fixture-v1');
    const first = acceptanceInputIdentity(temp);
    writeFileSync(testFile, 'assert.equal(1, 2)');
    const changedTest = acceptanceInputIdentity(temp);
    assert.notEqual(changedTest, first);
    writeFileSync(fixture, 'fixture-v2');
    assert.notEqual(acceptanceInputIdentity(temp), changedTest);
  } finally { rmSync(temp, { recursive: true, force: true }); }
});

test('resume rejects helper edits, additions and deletions while retaining identical input reuse', () => {
  const temp = mkdtempSync(path.join(os.tmpdir(), 'wombat-e2e-helper-inputs-'));
  try {
    for (const directory of ['tests/integration', 'tests/e2e', 'tests/helpers/nested', 'plugin']) mkdirSync(path.join(temp, directory), { recursive: true });
    const helper = path.join(temp, 'tests/helpers/nested/shared.ts');
    const added = path.join(temp, 'tests/helpers/nested/added.ts');
    writeFileSync(helper, 'helper-v1');
    const key = () => runFingerprint({ sourceSha256: 'unchanged-source', artifactSha256: 'unchanged-artifact', acceptanceInputsSha256: acceptanceInputIdentity(temp), scope: 'all' });
    const prior = { runKey: key(), stages: [{ name: 'integration', status: 'passed' }, { name: 'e2e', status: 'passed' }] };
    const check = (expected: boolean) => {
      for (const stage of ['integration', 'e2e']) assert.equal(reusableStage(prior, stage, key()), expected, stage);
    };
    check(true);
    writeFileSync(helper, 'helper-v2');
    check(false);
    writeFileSync(helper, 'helper-v1');
    check(true);
    writeFileSync(added, 'new-helper');
    check(false);
    rmSync(added);
    check(true);
    rmSync(helper);
    check(false);
    writeFileSync(helper, 'helper-v1');
    check(true);
  } finally { rmSync(temp, { recursive: true, force: true }); }
});

test('source identity includes plugin assets, relocated installers and notices', () => {
  const temp = mkdtempSync(path.join(os.tmpdir(), 'wombat-plugin-source-'));
  try {
    execFileSync('git', ['init', '--quiet', temp], { timeout: 5000, maxBuffer: 4096 });
    const resources = ['plugin/skills/wombat/SKILL.md', 'plugin/package.json', 'plugin/hooks/hooks.json', 'plugin/scripts/collect.sh', 'scripts/install/install.sh', 'scripts/install/install.ps1', 'licenses/THIRD_PARTY_NOTICES.md'];
    for (const file of resources) {
      mkdirSync(path.dirname(path.join(temp, file)), { recursive: true });
      writeFileSync(path.join(temp, file), 'source-v1');
    }
    let previous = sourceIdentity(temp);
    for (const file of resources) {
      writeFileSync(path.join(temp, file), 'source-v2');
      const changed = sourceIdentity(temp);
      assert.notEqual(changed, previous, file);
      previous = changed;
    }
    mkdirSync(path.join(temp, 'dist/skill'), { recursive: true });
    writeFileSync(path.join(temp, 'dist/skill/manifest.json'), 'generated');
    assert.equal(sourceIdentity(temp), previous);
  } finally { rmSync(temp, { recursive: true, force: true }); }
});

test('workspace package build outputs participate in the artifact resume identity', () => {
  const temp = mkdtempSync(path.join(os.tmpdir(), 'wombat-e2e-artifacts-'));
  try {
    mkdirSync(path.join(temp, 'dist'), { recursive: true }); writeFileSync(path.join(temp, 'dist/wombat.js'), 'root-cli');
    for (const packageName of ['client', 'cli']) {
      mkdirSync(path.join(temp, packageName, 'dist'), { recursive: true });
      writeFileSync(path.join(temp, packageName, 'dist/index.js'), `${packageName}-v1`);
    }
    const first = currentArtifactIdentity(temp);
    const priorKey = runFingerprint({ sourceSha256: 'source', artifactSha256: first, acceptanceInputsSha256: 'tests', scope: 'e2e' });
    const prior = { runKey: priorKey, stages: [{ name: 'integration', status: 'passed' }] };
    writeFileSync(path.join(temp, 'client/dist/index.js'), 'client-v2');
    const changed = currentArtifactIdentity(temp);
    assert.notEqual(changed, first);
    assert.equal(reusableStage(prior, 'integration', runFingerprint({ sourceSha256: 'source', artifactSha256: changed, acceptanceInputsSha256: 'tests', scope: 'e2e' })), false);
    rmSync(path.join(temp, 'cli/dist'), { recursive: true, force: true });
    assert.throws(() => currentArtifactIdentity(temp));
  } finally { rmSync(temp, { recursive: true, force: true }); }
});

test('bounded command execution reports success, nonzero exit, timeout, and missing executable', async () => {
  const temp = mkdtempSync(path.join(os.tmpdir(), 'wombat-e2e-process-'));
  const invoke = (name: string, args: string[], timeoutMs = 3000) => runBoundedCommand({ command: [process.execPath, ...args], cwd: temp, env: process.env,
    logFile: path.join(temp, `${name}.log`), timeoutMs, maxBytes: 64 * 1024 });
  try {
    const success = await invoke('success', ['-e', "process.stdout.write('synthetic-ok')"]);
    assert.equal(success.exitCode, 0); assert.equal(success.timedOut, false);
    const failure = await invoke('nonzero', ['-e', 'process.exit(7)']);
    assert.equal(failure.exitCode, 7); assert.equal(failure.timedOut, false);
    const timeout = await invoke('timeout', ['-e', 'setTimeout(()=>{},5000)'], 500);
    assert.equal(timeout.timedOut, true); assert.equal(timeout.closeTimedOut, false);
    const missing = await runBoundedCommand({ command: [path.join(temp, 'does-not-exist')], cwd: temp, env: process.env,
      logFile: path.join(temp, 'missing.log'), timeoutMs: 3000, maxBytes: 64 * 1024 });
    assert.match(missing.spawnError?.message ?? '', /ENOENT/);
  } finally { rmSync(temp, { recursive: true, force: true }); }
});

test('timeout kills descendants even after the direct child exits on SIGTERM', async () => {
  if (process.platform === 'win32') return;
  const temp = mkdtempSync(path.join(os.tmpdir(), 'wombat-e2e-descendant-'));
  let descendant: number | undefined;
  const marker = path.join(temp, 'descendant-survived');
  try {
    const code = `const {spawn}=require('node:child_process');const fs=require('node:fs');const child=spawn(process.execPath,['-e',${JSON.stringify(`process.on('SIGTERM',()=>{});setTimeout(()=>{require('node:fs').writeFileSync(${JSON.stringify(marker)},'alive');process.exit(0)},3000)`)}],{stdio:'ignore'});console.log('DESCENDANT_PID='+child.pid);process.on('SIGTERM',()=>process.exit(0));setInterval(()=>{},1000);`;
    const result = await runBoundedCommand({ command: [process.execPath, '-e', code], cwd: temp, env: process.env,
      logFile: path.join(temp, 'tree.log'), timeoutMs: 600, maxBytes: 64 * 1024 });
    assert.equal(result.timedOut, true); assert.equal(result.closeTimedOut, false);
    const log = readFileSync(path.join(temp, 'tree.log'), 'utf8');
    const pid = /DESCENDANT_PID=(\d+)/.exec(log)?.[1]; assert(pid); descendant = Number(pid);
    await new Promise(resolve => setTimeout(resolve, 3200));
    assert.equal(existsSync(marker), false, 'descendant kept running after its parent exited');
  } finally {
    if (descendant && processExists(descendant)) { try { process.kill(descendant, 'SIGKILL'); } catch { /* Already exited. */ } }
    rmSync(temp, { recursive: true, force: true });
  }
});

test('cleanup handles an already closed child and still stops surviving group descendants',async()=>{
  if(process.platform==='win32')return;
  const temp=mkdtempSync(path.join(os.tmpdir(),'wombat-closed-tree-'));
  const marker=path.join(temp,'descendant-survived');let descendant:number|undefined;
  const body=`const {spawn}=require('node:child_process');const child=spawn(process.execPath,['-e',${JSON.stringify(`process.on('SIGTERM',()=>{});setTimeout(()=>{require('node:fs').writeFileSync(${JSON.stringify(marker)},'alive');},1000)`)}],{stdio:'ignore'});console.log(child.pid);child.unref();`;
  const child=spawn(process.execPath,['-e',body],{detached:true,stdio:['ignore','pipe','ignore']});
  let output='';child.stdout.on('data',chunk=>{output+=chunk.toString();});
  try {
    await once(child,'close');descendant=Number(output.trim());assert.ok(Number.isSafeInteger(descendant)&&descendant>0);
    assert.equal(await terminateTree(child),true);
    await new Promise(resolve=>setTimeout(resolve,1100));
    assert.equal(existsSync(marker),false,'Closing the parent must not skip the surviving process group');
  } finally {
    if(descendant&&processExists(descendant)){try{process.kill(descendant,'SIGKILL');}catch{}}
    rmSync(temp,{recursive:true,force:true});
  }
});

function processExists(pid: number): boolean { try { process.kill(pid, 0); return true; } catch { return false; } }

test('an output path reached through an existing symlink into the repository is rejected', () => {
  if (process.platform === 'win32') return;
  const temp = mkdtempSync(path.join(os.tmpdir(), 'wombat-e2e-output-'));
  try {
    const repo = path.join(temp, 'repo'), parent = path.join(temp, 'external');
    mkdirSync(repo); mkdirSync(parent);
    const link = path.join(parent, 'repo-link');
    writeFileSync(path.join(repo, 'marker'), 'synthetic');
    symlinkSync(repo, link, 'dir');
    assert.throws(() => assertExternalOutputDir(repo, path.join(link, 'reports')), /resolves inside the repository/);
  } finally { rmSync(temp, { recursive: true, force: true }); }
});
