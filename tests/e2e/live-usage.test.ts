import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, mkdir, writeFile, appendFile, rm, readdir, chmod } from 'node:fs/promises';
import { spawn, spawnSync } from 'node:child_process';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { DatabaseSync } from 'node:sqlite';
import { setTimeout as delay } from 'node:timers/promises';

const binary = path.resolve('dist', process.platform === 'win32' ? 'wombat-core.exe' : 'wombat-core');
const cli = path.resolve('dist/wombat.js');
const row = (r: string, input = 100) => JSON.stringify({type:'event_msg',timestamp:'2026-09-29T00:00:01Z',payload:{type:'token_usage_record',thread_id:'t',turn_id:'u',response_id:r,usage:{input_tokens:input,cached_input_tokens:60,cache_write_input_tokens:0,output_tokens:10,reasoning_output_tokens:2,total_tokens:input+10}}});

test('default source uses the user profile without CODEX_HOME', { timeout: 40_000 }, async () => {
  const dir = await mkdtemp(path.join(tmpdir(), 'wombat-profile-'));
  const profile = path.join(dir, 'profile');
  const sessions = path.join(profile, '.codex', 'sessions');
  await mkdir(sessions, { recursive: true });
  await writeFile(path.join(sessions, 'one.jsonl'), JSON.stringify({ type: 'session_meta', payload: { id: 't' } }) + '\n' + row('one') + '\n');
  const env: NodeJS.ProcessEnv = { ...process.env, USERPROFILE: profile, WOMBAT_DATA_HOME: path.join(dir, 'data'), WOMBAT_AUTO_PRICES: '0' };
  delete env.CODEX_HOME; delete env.WOMBAT_CORE_BIN;
  if (process.platform === 'win32') delete env.HOME; else env.HOME = profile;
  try {
    const result = spawnSync(process.execPath, [cli, 'usage', '--fresh', '--json'], { env, encoding: 'utf8', timeout: 15000 });
    assert.ifError(result.error);
    assert.ok([0, 2].includes(result.status!), result.stdout + result.stderr);
    assert.equal(JSON.parse(result.stdout).summary.tokens.total, 110);
  } finally { await rm(dir, { recursive: true, force: true, maxRetries: 20, retryDelay: 1000 }); }
});

test('live CLI resumes, keeps fixed views, exports only explicitly and streams append updates', { timeout: 30_000 }, async () => {
  const dir = await mkdtemp(path.join(tmpdir(), 'wombat-live-'));
  const source = path.join(dir,'source'), data = path.join(dir,'data'), log = path.join(source,'sessions','one.jsonl');
  await mkdir(path.dirname(log), {recursive:true});
  await writeFile(log, JSON.stringify({type:'session_meta',payload:{id:'t'}})+'\n'+JSON.stringify({type:'turn_context',payload:{turn_id:'u',model:'gpt-5.4',effort:'high'}})+'\n'+row('one')+'\n');
  const env: NodeJS.ProcessEnv = {...process.env, WOMBAT_AUTO_PRICES:'0', WOMBAT_DATA_HOME:data, CODEX_HOME:source}; delete env.WOMBAT_CORE_BIN;
  let service: ReturnType<typeof spawn> | undefined;
  const start = async () => { service = spawn(binary,['--serve-usage'],{env,stdio:'ignore'}); await delay(100); };
  const stop = async () => { if(service && service.exitCode === null) { const exited=new Promise(resolve=>service!.once('exit',resolve)); service.kill(); await exited; } };
  const run = (args: string[]) => {
    const p=spawnSync(process.execPath,[cli,...args,'--json'],{env,encoding:'utf8',timeout:15_000});
    assert.ifError(p.error); const value=JSON.parse(p.stdout); assert.ok([0,2].includes(p.status!),p.stdout+p.stderr); return value;
  };
  try {
    await start();
    const first=run(['usage','--fresh']);
    assert.equal(first.summary.tokens.total,110); assert.equal(first.freshness.status,'current');
    const index = new DatabaseSync(path.join(data,'live-v2','index.sqlite'),{readOnly:true});
    try {
      assert.equal(index.prepare("SELECT count(*) AS n FROM fact_rows r JOIN buckets b ON b.id=r.bucket WHERE b.scope LIKE 'projection:%' AND b.field='measurements'").get()!.n,1);
      assert.equal(index.prepare("SELECT count(*) AS n FROM fact_rows r LEFT JOIN fact_blocks f ON f.id=r.block AND f.bucket=r.bucket WHERE f.id IS NULL").get()!.n,0);
    } finally { index.close(); }
    assert.ok(!(await readdir(data)).includes('usage-v4'), 'automatic sync must not export snapshots');
    await stop();
    await appendFile(log,row('two'));
    await start();
    const cached=run(['usage','--cached']);
    assert.equal(cached.summary.tokens.total,110); assert.equal(cached.snapshotRef.snapshotId,first.snapshotRef.snapshotId);
    const partial=run(['usage','--fresh']); assert.equal(partial.summary.tokens.total,110); assert.equal(partial.quality.status,'partial');
    await appendFile(log,'\n');
    const second=run(['usage','--fresh']); assert.equal(second.summary.tokens.total,220); assert.equal(second.quality.status,'complete');
    const fixed=run(['usage','--snapshot',first.snapshotRef.snapshotId]); assert.equal(fixed.summary.tokens.total,110);
    const snapshot=run(['refresh','--verify']); assert.equal(snapshot.summary.tokens.total,220); assert.ok(!snapshot.snapshotRef.snapshotId.startsWith('live:'));
    const watch=spawn(process.execPath,[cli,'usage','--watch','--json'],{env,stdio:['ignore','pipe','pipe']});
    try {
      const results: any[]=[]; let buffer=''; let error='';
      watch.stdout!.on('data',chunk=>{buffer+=chunk; let n; while((n=buffer.indexOf('\n'))>=0){ results.push(JSON.parse(buffer.slice(0,n)));buffer=buffer.slice(n+1); }});
      watch.stderr!.on('data',chunk=>error+=chunk);
      const until=async (condition:()=>boolean)=>{const deadline=Date.now()+7000;while(!condition()&&Date.now()<deadline)await delay(30);assert.ok(condition(),error+JSON.stringify(results));};
      await until(()=>results.length>0);
      await appendFile(log,row('three')+'\n');
      assert.equal(run(['usage']).summary.tokens.total,220, 'auto reads reuse the committed view within five minutes');
      assert.equal(run(['usage','--fresh']).summary.tokens.total,330, 'explicit refresh bypasses the interval');
      await until(()=>results.some(r=>r.summary?.tokens.total===330));
      const historical=run(['usage','--snapshot',snapshot.snapshotRef.snapshotId]); assert.equal(historical.summary.tokens.total,220);
      const stable=run(['usage','--fresh']); const again=run(['usage','--fresh']); assert.equal(stable.snapshotRef.snapshotId,again.snapshotRef.snapshotId);
      const exit=new Promise(resolve=>watch.once('close',resolve)); watch.kill('SIGINT');
      const code = await exit;
      if (process.platform !== 'win32') assert.equal(code,130); // Windows kill does not deliver a console Ctrl-C event.
    } finally { if(watch.exitCode===null)watch.kill('SIGKILL'); }
  } finally { await stop(); await rm(dir,{recursive:true,force:true}); }
});


test('failed source retains facts while healthy source advances, survives restart and recovers', {
  timeout: 40_000, skip: process.platform === 'win32' || process.getuid?.() === 0,
}, async () => {
  const dir = await mkdtemp(path.join(tmpdir(), 'wombat-isolation-'));
  const roots = [path.join(dir, 'good'), path.join(dir, 'bad')];
  const logs = roots.map(root => path.join(root, 'sessions', 'one.jsonl'));
  for (const [i, log] of logs.entries()) {
    await mkdir(path.dirname(log), { recursive: true });
    await writeFile(log, JSON.stringify({ type: 'session_meta', payload: { id: 't' } }) + '\n' + row('first-' + i) + '\n');
  }
  const env = { ...process.env, WOMBAT_AUTO_PRICES: '0', WOMBAT_DATA_HOME: path.join(dir, 'data') };
  let service: ReturnType<typeof spawn>;
  const start = async () => { service = spawn(binary, ['--serve-usage'], { env, stdio: 'ignore' }); await delay(100); };
  const stop = async () => { if (service && service.exitCode === null) { const closed = new Promise(resolve => service.once('close', resolve)); service.kill(); await closed; } };
  const run = (mode: string, snapshot?: string) => {
    const args = snapshot ? ['--snapshot', snapshot] : [mode, ...roots.flatMap(root => ['--root', root])];
    const result = spawnSync(process.execPath, [cli, 'usage', ...args, '--since', '2026-09-29', '--until', '2026-09-30', '--timezone', 'UTC', '--json'], { env, encoding: 'utf8', timeout: 15000 });
    assert.ifError(result.error);
    return { status: result.status, value: JSON.parse(result.stdout) };
  };
  try {
    await start();
    const first = run('--fresh').value;
    assert.equal(first.summary.tokens.total, 220);
    for (const [i, log] of logs.entries()) await appendFile(log, row('second-' + i) + '\n');
    await chmod(logs[1]!, 0);
    const partial = run('--fresh');
    assert.equal(partial.status, 2);
    assert.equal(partial.value.summary.tokens.total, 330);
    assert.equal(partial.value.quality.status, 'partial');
    assert.ok(JSON.stringify(partial.value.quality).includes('sourceSyncFailed'));
    assert.equal(run('--fresh').value.snapshotRef.snapshotId, partial.value.snapshotRef.snapshotId);
    assert.equal(run('', first.snapshotRef.snapshotId).value.summary.tokens.total, 220);
    await stop(); await start();
    const restored = run('--cached').value;
    assert.equal(restored.summary.tokens.total, 330);
    assert.equal(restored.quality.status, 'partial');
    await chmod(logs[0]!, 0);
    const failed = run('--fresh');
    assert.ok(failed.status !== 0);
    assert.equal(run('--cached').value.summary.tokens.total, 330);
    await chmod(logs[0]!, 0o600); await chmod(logs[1]!, 0o600);
    const recovered = run('--fresh').value;
    assert.equal(recovered.summary.tokens.total, 440);
    assert.equal(recovered.quality.status, 'complete');
    assert.notEqual(recovered.snapshotRef.snapshotId, partial.value.snapshotRef.snapshotId);
  } finally {
    for (const log of logs) await chmod(log, 0o600);
    await stop(); await rm(dir, { recursive: true, force: true });
  }
});


test('active and archived task metadata is discoverable without inventing measurements', {timeout:20_000}, async()=>{
 const dir=await mkdtemp(path.join(tmpdir(),'wombat-unmetered-')),root=path.join(dir,'source'),data=path.join(dir,'data');
 for(const bucket of ['sessions','archived_sessions']){await mkdir(path.join(root,bucket),{recursive:true});await writeFile(path.join(root,bucket,'one.jsonl'),JSON.stringify({type:'session_meta',timestamp:'2020-01-01T00:00:00Z',payload:{id:bucket,cwd:'/synthetic/project'}})+'\n');}
 const env={...process.env,WOMBAT_AUTO_PRICES:'0',WOMBAT_DATA_HOME:data,CODEX_HOME:root};
 const service=spawn(binary,['--serve-usage'],{env,stdio:'ignore'});
 try {
  const run=(args:string[])=>{const p=spawnSync(process.execPath,[cli,...args,'--json'],{env,encoding:'utf8',timeout:10000});assert.ifError(p.error);assert.ok([0,2].includes(p.status!),p.stdout+p.stderr);return JSON.parse(p.stdout);};
  const empty=run(['usage','--fresh','--since','2026-09-01','--until','2026-10-01']);
  assert.equal(empty.summary.measurementCount,0);assert.equal(empty.facets.discoveredThreadCount,2);
  const tasks=run(['threads','--snapshot',empty.snapshotRef.snapshotId]);
  assert.equal(tasks.items.length,2);assert.equal(tasks.summary.measurementCount,0);assert.equal(tasks.items.every((t:any)=>t.matchedUsage.measurementCount===0),true);
 }finally{const exit=new Promise(resolve=>service.once('close',resolve));service.kill('SIGTERM');await exit;await rm(dir,{recursive:true,force:true});}
});
