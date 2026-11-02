import {spawnProductCli,type CliInput} from './cli-invocation.ts';
/** Real installed runtime, native plugin manager and optional native model journey. */
import assert from 'node:assert/strict';
import {spawn, type ChildProcess} from 'node:child_process';
import {existsSync, mkdirSync, readFileSync, writeFileSync, appendFileSync, realpathSync, rmSync, readdirSync} from 'node:fs';
import path from 'node:path';
import {fileURLToPath, pathToFileURL} from 'node:url';
import {setTimeout as delay} from 'node:timers/promises';
import {onboardingOptions, commandMentionsExecutable} from './verify-onboarding-options.ts';
import {assertExternalOutputDir, runBoundedCommand, stdoutFromLog, terminateTree} from './verify-e2e-helpers.ts';
import {checkBuild, sourceIdentity} from './build-identity.ts';
import {agentMetrics} from './verify-agent-query.ts';
import {nativeSessionId} from './verify-skill-conversation-helpers.ts';
import {cleanupOnboardingCores} from './onboarding-cleanup.ts';

const repo = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const options = onboardingOptions(process.argv.slice(2));
if (process.platform === 'win32') throw new Error('This POSIX installer journey requires macOS or Linux; Windows acceptance is separate');
checkBuild(repo);
const output = assertExternalOutputDir(repo, options.output);
if (existsSync(output) && readdirSync(output).length) throw new Error('Choose a fresh empty --output-dir; previous evidence is preserved');
mkdirSync(output, {recursive: true, mode: 0o700});
const work = path.join(output, 'fixture'), prefix = path.join(work, "安装 & runtime '"), home = path.join(work, 'codex'), data = path.join(work, 'data'), project = path.join(work, "项目 with ' quote");
mkdirSync(path.join(home, 'sessions'), {recursive: true}); mkdirSync(project, {recursive: true});
const source = realpathSync(home), cwd = realpathSync(project), agentEnv = {...process.env};
const env = {...process.env, PATH: '/usr/bin:/bin', CODEX_HOME: source, WOMBAT_DATA_HOME: data,
  WOMBAT_CODEX_BIN: options.codex, WOMBAT_CORE_BIN: '', WOMBAT_AUTO_PRICES: '0', WOMBAT_LANG: 'en'};
const launcher = path.join(prefix, 'bin/wombat'), installer = path.join(repo, 'scripts/install/install.sh');
const report: {status: string; sourceSha256: string; platform: string; checks: unknown[]; boundaries: string[]; failure?: string; completedAt?: string} = {
  status: 'running', sourceSha256: sourceIdentity(repo), platform: process.platform+'-'+process.arch, checks: [],
  boundaries: ['Real local candidate archive and native plugin installation; only independent synthetic usage is queried.',
    'Optional model sessions retain the current authentication profile; runtime queries are restricted to synthetic source and data. No credentials are read or copied.',
    'CLI acceptance uses installed production assets and two languages. No Hook trust changes or publication. Other platforms and large-history performance remain separate.']};
const save = () => writeFileSync(path.join(output, 'onboarding-report.json'), JSON.stringify(report, null, 2)+'\n', {mode: 0o600});
const pass = (name: string, evidence: unknown = {}) => {report.checks.push({name, status: 'passed', evidence}); save(); console.log(JSON.stringify({stage: name, status: 'passed'}));};
const cancel = new AbortController(), onCancel = () => cancel.abort();
process.once('SIGINT', onCancel); process.once('SIGTERM', onCancel);
const deadline = setTimeout(onCancel, 12*60_000);

save();
const run = async (name: string, command: string[], commandEnv: NodeJS.ProcessEnv = env, timeoutMs = 120_000) => {
  const log = path.join(output, name+'.log');
  const result = await runBoundedCommand({command, cwd, env: commandEnv, logFile: log, timeoutMs, maxBytes: 4*1024*1024, signal: cancel.signal});
  assert.equal(result.exitCode, 0, `${name}: ${JSON.stringify(result)}`);
  assert.ok(!result.timedOut && !result.outputLimit && !result.interrupted && !result.closeTimedOut && !result.spawnError && !result.logError, name);
  return stdoutFromLog(readFileSync(log, 'utf8')).trim();
};
const fixtureFile = path.join(source, 'sessions/synthetic.jsonl');
const timestamp = new Date().toISOString();
const record = (response: string, tokens: number) => JSON.stringify({timestamp, type: 'event_msg', payload: {
  type: 'token_usage_record', thread_id: 'onboarding', turn_id: 'turn', response_id: response,
  usage: {input_tokens: tokens-10, cached_input_tokens: 0, output_tokens: 10, total_tokens: tokens}}})+'\n';
const sourceBody = [JSON.stringify({timestamp, type: 'session_meta', payload: {id: 'onboarding', cwd}}),
  JSON.stringify({timestamp, type: 'turn_context', payload: {turn_id: 'turn', model: 'gpt-5.4'}})].join('\n')+'\n'+record('one', 110);
try {
  await run('install', ['/bin/sh', installer, '--prefix', prefix, '--base-url', pathToFileURL(options.release).href, '--no-modify-path', '--plugin']);
  const id = readFileSync(path.join(prefix, 'lib/wombat/current.txt'), 'utf8').trim(), destination = path.join(prefix, 'lib/wombat/versions', id);
  const release = JSON.parse(readFileSync(path.join(destination, 'release.json'), 'utf8'));
  assert.equal(release.sourceSha256, report.sourceSha256);
  const version = JSON.parse(await run('bundled-no-system-runtime', ['/usr/bin/env', 'PATH=', launcher, '--version', '--json']));
  assert.equal(version.version, release.version);
  const setup = JSON.parse(await run('native-discovery', [launcher, 'system','doctor', '--project', cwd, '--json']));
  assert.equal(setup.discovery.status, 'available'); assert.equal(setup.discovery.instances[0].name, 'wombat:wombat');
  assert.equal(setup.runtimeChecks[0].status, 'compatible');
  pass('fresh-install-plugin', {release, invocation:'wombat:wombat', noSystemNodeOrRust:true});
  const query = async (name:string,args:CliInput)=>{if(Array.isArray(args))return JSON.parse(await run(name,[launcher,...args,'--json']));const r=spawnProductCli(path.join(destination,'lib/wombat.js'),args,{cwd,env,encoding:'utf8',timeout:60000,maxBuffer:2*1024*1024},[],path.join(destination,'runtime/node'));writeFileSync(path.join(output,name+'.log'),r.stdout+r.stderr);assert.ifError(r.error);assert.equal(r.status,0,r.stdout+r.stderr);return JSON.parse(r.stdout);};
  const empty = await query('empty', ['usage','--all-time','--fresh']);
  assert.equal(empty.summary.measurementCount,0);
  writeFileSync(fixtureFile,sourceBody);
  const first = await query('first',['usage','--all-time','--fresh']);
  assert.equal(first.summary.tokens.total,110);
  for (const language of ['zh','en']) {
    const tasks = await query('tasks-'+language, {method:'usage',params:{query:{action:'threads',scope:{allTime:true}},mode:'cached'}});
    assert.equal(tasks.summary.tokens.total,110);
    assert.ok(tasks.items.some((v: {kind:string})=>v.kind==='thread'));
  }
  appendFileSync(fixtureFile,record('two',220));
  const updated = await query('updated',['usage','--all-time','--fresh']);
  assert.equal(updated.summary.tokens.total,330);
  const fixed = await query('fixed',['usage','--cached','--all-time']);
  assert.equal(fixed.summary.tokens.total,330);
  pass('append-and-cached-read',{previous:110,current:330});
  await run('plugin-only',['/bin/sh',installer,'--prefix',prefix,'--plugin-only']);
  assert.equal(readFileSync(path.join(prefix,'lib/wombat/current.txt'),'utf8').trim(),id);
  pass('plugin-only-retains-runtime');
  if (options.agentBin) {
    const modelData = path.join(work, 'cold-model-data'), answer = path.join(work, 'answer.json'), schema = path.join(work, 'answer-schema.json');
    writeFileSync(schema, JSON.stringify({type: 'object', additionalProperties: false, required: ['tokens', 'taskId', 'explanation'], properties: {tokens: {type: 'integer'}, taskId: {type: 'string'}, explanation: {type: 'string'}}}));
    const quote = (s: string) => "'"+s.replaceAll("'", "'\\''")+"'";
    const runtime = `CODEX_HOME=${quote(source)} WOMBAT_DATA_HOME=${quote(modelData)} WOMBAT_AUTO_PRICES=0 WOMBAT_CODEX_BIN=${quote(options.codex)} ${quote(launcher)}`;
    const boundary = ` This is an isolated installed-plugin onboarding test. Use the installed $${options.agentSkill} Skill and selected executable ${launcher}. Every runtime call must start with ${runtime}; query only synthetic source ${source} and project ${cwd}. Do not read raw logs, databases, credentials, other projects, or other plugins. No MCP, network tools or delegation. Only return the required JSON with a Chinese explanation. Do not change source files or Hook trust.`;
    const events = await run('installed-plugin-first-question', [options.agentBin, 'exec', '--config', 'mcp_servers={}', '--sandbox', 'workspace-write', '--skip-git-repo-check', '--cd', cwd, '--json', '--output-schema', schema, '--output-last-message', answer,
      `$${options.agentSkill} 我第一次用，请告诉我这个项目全部日期用了多少 Token，哪项任务用量最多？`+boundary], agentEnv, 360_000);
    const firstAnswer = JSON.parse(readFileSync(answer, 'utf8')); assert.equal(firstAnswer.tokens, 330); assert.match(firstAnswer.taskId, /^[0-9a-f]{64}$/); assert.match(firstAnswer.explanation, /[\u3400-\u9fff]/);
    const metrics = agentMetrics(events); assert.equal(metrics.unsupportedToolCalls, 0); assert.ok(metrics.commands.some(c => commandMentionsExecutable(c, launcher)));
    appendFileSync(fixtureFile, record('three', 440));
    const followEvents = await run('installed-plugin-followup', [options.agentBin, 'exec', 'resume', nativeSessionId(events), '--config', 'mcp_servers={}', '--skip-git-repo-check', '--json', '--output-schema', schema, '--output-last-message', answer,
      '刚才这个任务又有了新的用量记录，请更新一下全部日期用量，继续查看同一项任务。'+boundary], agentEnv, 360_000);
    const next = JSON.parse(readFileSync(answer, 'utf8')); assert.equal(next.tokens, 770); assert.equal(next.taskId, firstAnswer.taskId); assert.match(next.explanation, /[\u3400-\u9fff]/);
    const followMetrics = agentMetrics(followEvents); assert.equal(followMetrics.unsupportedToolCalls, 0); assert.ok(followMetrics.commands.some(c => commandMentionsExecutable(c, launcher)));
    assert.equal(readFileSync(fixtureFile, 'utf8'), sourceBody+record('two', 220)+record('three', 440));
    pass('installed-plugin-cold-first-question-and-resume', {first: firstAnswer, followup: next, metrics, followMetrics});
  }
  report.status = 'passed';
} catch (error) {report.status = 'failed'; report.failure = error instanceof Error ? error.stack : String(error); process.exitCode = 1;}
finally {
  try {
    await cleanupOnboardingCores(prefix,[data,path.join(work,'cold-model-data')]);
    pass('owned-process-cleanup');
    rmSync(work, {recursive: true, force: true, maxRetries: 5, retryDelay: 100});
  } catch (error) {report.status = 'failed'; report.failure = (report.failure ?? '')+'\nCleanup: '+String(error); process.exitCode = 1;}
  clearTimeout(deadline); process.off('SIGINT', onCancel); process.off('SIGTERM', onCancel);
  report.completedAt = new Date().toISOString(); save(); console.log(JSON.stringify({status: report.status, report: path.join(output, 'onboarding-report.json'), failure: report.failure}));
}
