import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, writeFile, chmod, rm, readFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { spawn } from 'node:child_process';
import { CoreError } from '@wombat/client';
import { createNodeClient } from '@wombat/client/node';
async function run(args: string[], env: NodeJS.ProcessEnv, tty = false): Promise<{
  code: number | null;
  stdout: string;
  stderr: string;
}> {
  return await new Promise((resolve, reject) => { const child = spawn(process.execPath, tty ? ['--input-type=module', '-e', "Object.defineProperty(process.stdin, 'isTTY', {value:true}); Object.defineProperty(process.stdout, 'isTTY', {value:true}); process.argv = [process.execPath, 'dist/wombat.js', ...process.argv.slice(1)]; await import('./dist/wombat.js');", '--', ...args] : ['dist/wombat.js', ...args], { cwd: process.cwd(), env: { ...process.env, WOMBAT_AUTO_PRICES: '0', WOMBAT_LANG: 'zh', ...env }, stdio: ['pipe', 'pipe', 'pipe'] }); let stdout = '', stderr = ''; child.stdout.on('data', chunk => stdout += chunk); child.stderr.on('data', chunk => stderr += chunk); child.on('error', reject); child.on('close', code => resolve({ code, stdout, stderr })); });
}
async function mockCore(body: string): Promise<{
  dir: string;
  binary: string;
}> {
  const dir = await mkdtemp(path.join(tmpdir(), 'wombat-v1-cli-'));
  const binary = path.join(dir, 'core.cjs');
  await writeFile(binary, `#!${process.execPath}\n${body}`);
  await chmod(binary, 0o700);
  return { dir, binary };
}
const body = `let input='';process.stdin.on('data',chunk=>input+=chunk);process.stdin.on('end',()=>{const request=JSON.parse(input);if(process.env.WOMBAT_TEST_ERROR){console.log(JSON.stringify({ok:false,code:'NO_SNAPSHOT',error:'请先更新记录'}));return;}const tokens={input:100,rawInput:100,cacheRead:0,cacheCreate:0,output:20,reasoning:0,total:120};const field=value=>({observedSubtotal:value,coveredRecords:1,missingRecords:0,conflictingRecords:0,invalidRecords:0,indeterminateRecords:0});const tokenAnalysis={methodVersion:1,scope:'selected_canonical_measurements',fields:Object.fromEntries(Object.entries(tokens).map(([key,value])=>[key,field(value)]))};const price={cost:'0.00055',knownCost:'0.00055',status:'priced',currency:'USD',policy:'official-standard-api-equivalent-v1',priceRevision:'synthetic',components:[],basis:[],issues:[]};console.error(JSON.stringify({stage:'保存记录'}));console.log(JSON.stringify({ok:true,value:{outputVersion:1,freshness:{status:"current",initialScan:false,checkedAt:"2026-09-30T00:00:00Z",revision:"synthetic",error:null},result:{outputVersion:5,action:request.args.query.action,snapshotRef:{snapshotId:'synthetic',createdAt:'2026-09-30T00:00:00Z'},scope:request.args.query.scope||{},availableRange:{since:'2026-09-29',until:'2026-09-30'},summary:{tokens,tokenAnalysis,price,measurementCount:1},items:[],page:{offset:0,limit:50,total:0,nextOffset:null},quality:{issueScope:'index',queryImpact:process.env.WOMBAT_TEST_PARTIAL?'unknown':'none',status:process.env.WOMBAT_TEST_PARTIAL?'partial':'complete',issues:[],sources:[]}}}}));});`;
test('foreground CLI has one JSON object and partial exit 2', async () => {
  const core = await mockCore(body);
  try {
    const success = await run(['usage', '--json'], { WOMBAT_CORE_BIN: core.binary });
    assert.equal(success.code, 0);
    assert.equal(success.stdout.trim().split('\n').length, 1);
    const value = JSON.parse(success.stdout);
    assert.equal(value.outputVersion, 5);
    assert.equal(value.action, 'usage');
    assert.equal(success.stderr, '');
    const partial = await run(['usage', '--json'], { WOMBAT_CORE_BIN: core.binary, WOMBAT_TEST_PARTIAL: '1' });
    assert.equal(partial.code, 2);
    assert.equal(JSON.parse(partial.stdout).quality.status, 'partial');
    assert.equal(partial.stderr, '');
    const plain = await run(['usage', ], { WOMBAT_CORE_BIN: core.binary });
    assert.equal(plain.code, 0);
    const terminal = await run(['usage', ], { WOMBAT_CORE_BIN: core.binary }, true);
    assert.equal(terminal.code, 0, terminal.stderr);
    assert.equal(terminal.stdout, plain.stdout);
    assert.equal(terminal.stderr, '');
    assert.match(plain.stdout, /Wombat · 用量/);
    assert.doesNotMatch(plain.stdout, /选择.*命令|额度|体检/);
  }
  finally {
    await rm(core.dir, { recursive: true, force: true, maxRetries: 20, retryDelay: 1000 });
  }
});
test('v1 CLI invalid arguments and unavailable snapshot remain structured errors', async () => {
  const core = await mockCore(body);
  try {
    const invalid = await run(['usage', '--unknown', '--json'], { WOMBAT_CORE_BIN: core.binary });
    assert.equal(invalid.code, 1);
    assert.equal(JSON.parse(invalid.stdout).error.code, 'INVALID_ARGUMENT');
    assert.equal(invalid.stderr, '');
    const missing = await run(['usage', '--json'], { WOMBAT_CORE_BIN: core.binary, WOMBAT_TEST_ERROR: '1' });
    assert.equal(missing.code, 1);
    assert.equal(JSON.parse(missing.stdout).error.code, 'NO_SNAPSHOT');
  }
  finally {
    await rm(core.dir, { recursive: true, force: true, maxRetries: 20, retryDelay: 1000 });
  }
});
test('v1 core cancellation terminates a child that ignores SIGTERM', async () => {
  const core = await mockCore("process.on('SIGTERM',()=>{});process.stdin.resume();setInterval(()=>{},1000);console.error(JSON.stringify({stage:'ready'}));");
  const previous = process.env.WOMBAT_CORE_BIN;
  process.env.WOMBAT_CORE_BIN = core.binary;
  try {
    const controller = new AbortController();
    const work = createNodeClient({ binaryPath: core.binary }).query({scope:{allTime:true}, action: 'usage' }, { signal: controller.signal, onProgress: () => controller.abort() });
    await assert.rejects(work, (error: unknown) => error instanceof CoreError && error.code === 'CANCELLED');
  }
  finally {
    if (previous === undefined)
      delete process.env.WOMBAT_CORE_BIN;
    else
      process.env.WOMBAT_CORE_BIN = previous;
    await rm(core.dir, { recursive: true, force: true, maxRetries: 20, retryDelay: 1000 });
  }
});
test('v1 version works without a core or local package paths at runtime', async () => {
  const result = await run(['--version', '--json'], { WOMBAT_CORE_BIN: '/synthetic/missing-core' });
  const expectedVersion = JSON.parse(await readFile('package.json', 'utf8')).version;
  assert.equal(result.code, 0);
  assert.deepEqual(JSON.parse(result.stdout), { outputVersion: 5, name: 'Wombat', version: expectedVersion });
});
test('built CLI SIGINT returns CANCELLED and reaps an uncooperative core', { skip: process.platform === 'win32' }, async () => {
  const core = await mockCore("require('node:fs').writeFileSync(process.env.TEST_PID_FILE,String(process.pid));process.on('SIGTERM',()=>{});process.stdin.resume();setInterval(()=>{},1000);");
  let corePid: number | undefined;
  const child = spawn(process.execPath, ['dist/wombat.js', 'system', 'call'], {
    env: { ...process.env, WOMBAT_CORE_BIN: core.binary,TEST_PID_FILE:path.join(core.dir,'pid') }, stdio: ['pipe', 'pipe', 'pipe']
  });
  child.stdin.end(JSON.stringify({method:'usage',params:{query:{action:'usage',scope:{allTime:true}},mode:'cached'}}));
  try {
    const result = await new Promise<{ code: number | null; stdout: string; stderr: string }>((resolve, reject) => {
      let stdout = '', stderr = '';
      const timer = setTimeout(() => reject(new Error('CLI cancellation timed out')), 10_000);
      child.stdout.on('data', chunk => stdout += chunk);
      child.stderr.on('data', chunk => {
        stderr += chunk;
      });
      child.once('error', error => { clearTimeout(timer); reject(error); });
      child.once('close', code => { clearTimeout(timer); resolve({ code, stdout, stderr }); });
      void (async()=>{for(let i=0;i<200;i++){try{corePid=Number(await readFile(path.join(core.dir,'pid'),'utf8'));break;}catch{await new Promise(r=>setTimeout(r,20));}}if(corePid)child.kill('SIGINT');})();
    });
    assert.equal(result.code, 130, result.stderr);
    assert.equal(result.stdout.trim().split('\n').length, 1);
    assert.equal(JSON.parse(result.stdout).error.code, 'CANCELLED');
    assert.ok(corePid);
    assert.throws(() => process.kill(corePid!, 0), { code: 'ESRCH' });
  }
  finally {
    if (child.exitCode === null) child.kill('SIGKILL');
    if (corePid) { try { process.kill(corePid, 'SIGKILL'); } catch { /* Already reaped. */ } }
    await rm(core.dir, { recursive: true, force: true, maxRetries: 20, retryDelay: 1000 });
  }
});
