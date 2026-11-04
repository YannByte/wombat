import {comparable} from '../helpers/comparable.js';
import {spawnProductCli} from '../../scripts/cli-invocation.js';
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { realpathSync } from 'node:fs';
import { mkdtemp, mkdir, writeFile, appendFile, readFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { spawn, spawnSync } from 'node:child_process';
import { once } from 'node:events';
import { createNodeClient } from '@wombat/client/node';
import { projectTestClient } from '../helpers/project-client.js';
import { nativeCodexFixture } from '../helpers/native-codex.js';

test('extension review joins native metadata and pinned use without changing the ledger', { timeout: 60000 }, async () => {
  const dir = realpathSync.native(await mkdtemp(path.join(tmpdir(), 'wombat-extension-review-')));
  const root = path.join(dir, 'source'), project = path.join(dir, 'project');
  const file = path.join(root, 'plugins/cache/sample/skills/review/SKILL.md');
  const saved = { WOMBAT_DATA_HOME: process.env.WOMBAT_DATA_HOME, CODEX_HOME: process.env.CODEX_HOME, WOMBAT_AUTO_PRICES: process.env.WOMBAT_AUTO_PRICES };
  await mkdir(path.dirname(file), { recursive: true }); await mkdir(path.join(root, 'sessions')); await mkdir(project);
  await writeFile(path.join(project,'AGENTS.md'),'Synthetic project rule.\n');
  const body = '---\nname: review\ndescription: Synthetic evidence\n---\n';
  await writeFile(file, body + 'Short body.');
  await writeFile(path.join(root, 'config.toml'), '[plugins."sample@synthetic"]\nenabled=true\n');
  const event = (type: string, payload: object) => ({ timestamp: '2026-10-29T01:00:00Z', type, payload });
  await writeFile(path.join(root, 'sessions/test.jsonl'), [
    event('session_meta', { id: 't', cwd: project }),
    event('turn_context', { turn_id: 'u', model: 'gpt-5.4', cwd: project }),
    event('response_item', { type: 'function_call', call_id: 'read', name: 'read_file', arguments: JSON.stringify({ path: file }) }),
    event('response_item', { type: 'function_call_output', call_id: 'read', output: 'Synthetic body' }),
    event('event_msg', { type: 'token_usage_record', thread_id: 't', turn_id: 'u', response_id: 'r', usage: { input_tokens: 100, cached_input_tokens: 20, cache_write_input_tokens: 0, output_tokens: 10, reasoning_output_tokens: 2, total_tokens: 110 } }),
  ].map(r => JSON.stringify(r)).join('\n') + '\n');
  const native = await nativeCodexFixture(dir);
  const skill = { name: 'sample:review', path: file, description: 'Synthetic evidence', scope: 'user', enabled: true };
  const mode = (skills: object[]) => writeFile(native.mode, JSON.stringify({ skills }));
  await mode([skill]);
  Object.assign(process.env, { WOMBAT_DATA_HOME: path.join(dir, 'data'), CODEX_HOME: root, WOMBAT_AUTO_PRICES: '0' });
  const binary = path.resolve('dist', process.platform === 'win32' ? 'wombat-core.exe' : 'wombat-core');

  const client = projectTestClient(createNodeClient({ binaryPath: binary, codexBinaryPath: native.binary, automaticPrices: false }), [root], [project]);
  const scope = { since: '2026-10-29', until: '2026-10-30', timezone: 'UTC', project };
  try {
    const usage = await client.live!({ query: { action: 'usage', scope }, mode: 'fresh' });
    const fullInventory=await client.config!({scope});
    const rule=fullInventory.items.find(item=>item.path===path.join(project,'AGENTS.md'));
    assert.ok(rule?.current);
    const request = { review: true, kind: 'skill' as const, scope };
    const before = await client.config!(request);
    const item = before.items.find(i => i.path === file)!;
    assert.ok(item, 'native-only plugin Skill must join the physical inventory');
    assert.equal(item.usageCount, 1); assert.equal(item.relatedTasks, 1);
    const rules=await client.config!({kind:'rule',scope});
    assert.equal(rules.items.find(item=>item.id===rule.id)?.current,true,'A Skill-only capture must not retire cached rules');
    assert.equal((await client.config!({kind:'skill',scope,cachedUsage:true})).items.find(row=>row.id===item.id)?.usageCount,1,'Committed cached reads must preserve counts');
    assert.equal(item.contextFootprint?.contexts[0].name, 'sample:review');
    assert.equal(item.contextFootprint?.contexts[0].enabled, true);
    assert.equal(item.contextFootprint?.visibility, 'unconfirmed');
    assert.equal(before.extensionReview?.enabledItems, 1);
    assert.equal(before.extensionReview?.confirmedVisibleTokens, null);
    const estimate = item.contextFootprint?.estimate?.tokens;
    assert.ok(estimate && estimate > 0);
    const calls = await readFile(native.calls, 'utf8');
    const pinned = await client.config!({ review: true, kind: 'skill', scope });
    assert.deepEqual(comparable(pinned.items), comparable(before.items));
    assert.ok((await readFile(native.calls, 'utf8')).length > calls.length, 'each request captures current discovery');
    await writeFile(file, body + 'Long body. '.repeat(1000));
    const grown = await client.config!(request);
    assert.equal(grown.items.find(i => i.path === file)!.contextFootprint!.estimate!.tokens, estimate, "Catalog metadata estimate excludes an on-demand body");
    assert.ok(grown.items.find(i => i.path === file)!.bodyTokenEstimate!.tokens > item.bodyTokenEstimate!.tokens);
    await mode([{ ...skill, enabled: false }]);
    const disabled = await client.config!(request);
    assert.equal(disabled.extensionReview?.disabledItems, 1);
    assert.equal(disabled.items.find(i => i.path === file)?.usageCount, 1);
    await mode([skill]);
    const restored = await client.config!(request);
    assert.equal(restored.extensionReview?.enabledItems, 1);
    assert.equal(restored.items.find(i => i.path === file)?.id, item.id);
    const after = await client.live!({ query: { action: 'usage', scope  }, mode: 'cached' });
    assert.deepEqual(after.result.summary, usage.result.summary);
    assert.equal(after.result.summary.tokens.total, 110);
    await mode([skill, { ...skill, enabled: false }]);
    const conflict = await client.config!(request);
    assert.ok(conflict.coverage.issues.some(i => i.code === 'nativeSkillListPartial'));
    assert.ok(!conflict.items.find(i => i.path === file)?.contextFootprint?.contexts.length);
    await writeFile(native.mode, JSON.stringify({ skillsError: true }));
    const unavailable = await client.config!(request);
    assert.ok(unavailable.coverage.issues.some(i => i.code === 'CODEX_REQUEST_REJECTED'));
    assert.equal(unavailable.extensionReview?.enabledItems, 0);
    await mode([skill]);
    const cli = spawnProductCli(path.resolve('dist/wombat.js'), {method:'config',params:{review:true,kind:'skill',roots:[root],projectRoots:[project],scope }}, { env: { ...process.env, WOMBAT_CODEX_BIN: native.binary }, encoding: 'utf8', timeout: 15000 });
    assert.ok([0, 2].includes(cli.status!), cli.stderr + cli.stdout);
    assert.equal(JSON.parse(cli.stdout).extensionReview.enabledItems, 1);
    const api = spawnSync(process.execPath, [path.resolve('dist/wombat.js'), 'system', 'call'], { env: {...process.env,WOMBAT_CODEX_BIN:native.binary}, input: JSON.stringify({ method: 'config', params: { review: true, roots: [root], projectRoots: [project], kind: 'skill', scope } }), encoding: 'utf8', timeout: 15000 });
    assert.ok([0, 2].includes(api.status!), api.stderr + api.stdout);
    assert.equal(JSON.parse(api.stdout).items[0].contextFootprint.estimate.tokens, estimate);
    const unusedFile = path.join(root, 'plugins/cache/sample/skills/unused/SKILL.md');
    await mkdir(path.dirname(unusedFile), {recursive:true});
    await writeFile(unusedFile, '---\nname: unused\ndescription: Synthetic unused skill\n---\n');
    await mode([skill, {...skill,name:'sample:unused',path:unusedFile}]);
    const args = ['check','skills','--project',project,'--root',root,'--since',scope.since,'--until',scope.until,'--timezone','UTC'];
    for (const language of ['zh','en']) {
      const invoke = (options:string[]) => {
        const result = spawnSync(process.execPath,[path.resolve('dist/wombat.js'),...args,...options,'--lang',language],{env:{...process.env,WOMBAT_CODEX_BIN:native.binary},encoding:'utf8',timeout:15000});
        assert.ifError(result.error);assert.ok([0,2].includes(result.status!),result.stderr+result.stdout);
        return result.stdout;
      };
      const brief = invoke([]);
      assert.match(brief,/所选时间内未使用|Not used in the selected period/);
      assert.match(brief,/使用了 1 次|Used 1 time/);
      assert.doesNotMatch(brief,/方法版本|Method version|不证明|does not prove/);
      assert.match(invoke(['--explain']),/方法版本|Method version/);
      const plain = JSON.parse(invoke(['--json'])), detailed = JSON.parse(invoke(['--explain','--json']));
      assert.deepEqual(comparable(detailed.items),comparable(plain.items));
      assert.deepEqual(detailed.coverage,plain.coverage);
      assert.equal(plain.items.find((i:{path:string})=>i.path===file).usageCount,1);
      assert.equal(plain.items.find((i:{path:string})=>i.path===unusedFile).usageCount,0);
      assert.equal(detailed.explanation.baseline.counted,1);
      assert.equal(detailed.explanation.records.find((r:{reason:string})=>r.reason==='counted').countDelta,1);
      const reportFile=path.join(dir,`explanation-${language}.json`);
      await writeFile(reportFile,JSON.stringify(detailed));
      const replay=JSON.parse(invoke(['--baseline',reportFile,'--json']));
      assert.equal(replay.explanation.comparison.status,'unchanged');
      assert.equal(replay.explanation.comparison.delta,0);
      const itemId=plain.items.find((i:{path:string})=>i.path===file).id;
      const selected=JSON.parse(invoke(['--item',itemId,'--json']));
      assert.equal(selected.explanation.baseline.counted,1);
      assert.ok(selected.explanation.records.every((row:{itemId:string})=>row.itemId===itemId));
      const next=JSON.parse(invoke(['--explain','--offset','1','--limit','1','--json']));
      assert.equal(next.explanation.baseline.counted,1);
    }
    await mode([skill]);
    const command=(id:string,source:string,status:string,text:string)=>event('event_msg',{type:'item_completed',thread_id:'t',turn_id:'u',item:{type:'CommandExecution',id,command:['/bin/sh','-lc',text],cwd:project,source,status,exit_code:status==='completed'?0:1,parsed_cmd:[{type:'read',path:file}],aggregated_output:'Synthetic native result'}});
    const cat=`cat '${file.replaceAll("'","'\\''")}'`;
    await appendFile(path.join(root,'sessions/test.jsonl'),[
      command('native-read','unified_exec_startup','completed',cat),
      command('conditional-read','unified_exec_startup','failed','false && '+cat),
      command('user-read','user_shell','completed',cat),
      command('declined-read','unified_exec_startup','declined',cat),
    ].map(value=>JSON.stringify(value)).join('\n')+'\n');
    await client.live!({query:{action:'usage',scope},mode:'fresh'});
    const nativeUses=await client.config!({action:'explain',kind:'skill',scope});
    assert.equal(nativeUses.items.find(i=>i.path===file)?.usageCount,2,'Verified native read must count; shell conditionals, user commands and declined work must not');
    assert.equal(nativeUses.explanation?.baseline.counted,2);
    assert.ok(nativeUses.explanation?.records.some(row=>row.reason==='dispatch_unknown'));
    assert.deepEqual((await client.live!({query:{action:'usage',scope},mode:'cached'})).result.summary,usage.result.summary);
    const outside = path.join(dir, 'outside/SKILL.md');
    await mkdir(path.dirname(outside)); await writeFile(outside, 'SYNTHETIC_OUTSIDE_BODY_MUST_NOT_BE_READ');
    await mode([skill, { ...skill, name: 'external-review', path: outside }]);
    const external = await client.config!(request);
    const externalItem = external.items.find(i => i.path === outside)!;
    assert.equal(external.extensionReview?.enabledItems, 2);
    assert.equal(externalItem.measurementStatus, 'nativeMetadataOnly');
    assert.equal(externalItem.bytes, null); assert.equal(externalItem.bodyTokenEstimate, null);
    assert.ok(externalItem.contextFootprint?.estimate);
    assert.ok(!JSON.stringify(external).includes('SYNTHETIC_OUTSIDE_BODY_MUST_NOT_BE_READ'));
    const methods = (await readFile(native.calls, 'utf8')).trim().split('\n').map(l => JSON.parse(l).method);
    assert.ok(methods.every(m => ['initialize', 'skills/list', 'hooks/list'].includes(m)),'Configuration inspection must not start model work');
  } finally {

    for (const [key, value] of Object.entries(saved)) { if (value === undefined) delete process.env[key]; else process.env[key] = value; }
    await native.waitForExit(); await rm(dir, { recursive: true, force: true });
  }
});
