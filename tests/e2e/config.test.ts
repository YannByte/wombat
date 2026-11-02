import {queryInvocation,spawnProductCli, type CliInput} from '../../scripts/cli-invocation.js';
import { projectTestClient } from '../helpers/project-client.js';
import type { UsageClient } from '@wombat/client';
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { realpathSync } from 'node:fs';
import { mkdtemp, mkdir, writeFile, appendFile, readFile, readdir, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { spawn, spawnSync } from 'node:child_process';
import { once } from 'node:events';
import { createNodeClient } from '@wombat/client/node';
import type { ConfigRequest } from '@wombat/client';
test('configuration evidence shares the ledger, pins versions and enforces host read scope', { timeout: 40000 }, async () => {
    const dir = realpathSync.native(await mkdtemp(path.join(tmpdir(), 'wombat-config-e2e-')));
    const root = path.join(dir, 'source'), project = path.join(dir, 'project'), data = path.join(dir, 'data');
    const skill = path.join(project, '.agents/skills/review/SKILL.md');
    await mkdir(path.join(root, 'sessions'), { recursive: true });
    await mkdir(path.dirname(skill), { recursive: true });
    await writeFile(skill, 'Synthetic review instruction');
    const failedSkill = path.join(root, 'skills/failed/SKILL.md');
    await mkdir(path.dirname(failedSkill), { recursive: true });
    await writeFile(failedSkill, 'Synthetic unread instruction');
    await writeFile(path.join(project, 'AGENTS.md'), 'Synthetic project rule');
    await writeFile(path.join(root, 'config.toml'), '[mcp_servers.docs]\ncommand="DO_NOT_EXECUTE"\nenv={SECRET="SYNTHETIC_SECRET_NEVER_EXPOSE"}\n');
    const event = (type: string, payload: object) => ({ timestamp: '2026-09-29T00:00:01Z', type, payload });
    const rows = [
        event('session_meta', { id: 't', cwd: project }),
        event('response_item', { type: 'message', id: 'instructions', role: 'user', content: [{ type: 'input_text', text: `# AGENTS.md instructions for ${project}\n\n<INSTRUCTIONS>SYNTHETIC_RULE_BODY_PRIVATE</INSTRUCTIONS>` }], internal_chat_message_metadata_passthrough: { turn_id: 'u', content_item_kinds: ['agents_md.instructions'] } }),
        event('response_item', { type: 'message', id: 'ordinary-user-text', role: 'user', content: [{ type: 'input_text', text: `# AGENTS.md instructions for ${root}` }], internal_chat_message_metadata_passthrough: { turn_id: 'u', content_item_kinds: ['user_prompt'] } }),
        event('response_item', { type: 'message', id: 'skills', role: 'developer', content: [{ type: 'input_text', text: `<skills_instructions>\n### Skill roots\n- \`r0\` = \`${path.join(project, '.agents/skills')}\`\n### Available skills\n- review: Synthetic private description. (file: r0/review/SKILL.md)\n</skills_instructions>` }], internal_chat_message_metadata_passthrough: { turn_id: 'u', content_item_kinds: ['host_skills.instructions'] } }),
        event('turn_context', { turn_id: 'u', model: 'gpt-5.4', cwd: project }),
        event('event_msg', { type: 'task_started', turn_id: 'u' }),
        event('response_item', { type: 'message', id: 'skill-declaration', role: 'assistant', content: [{ type: 'output_text', text: '我会使用 review Skill。' }], internal_chat_message_metadata_passthrough: { turn_id: 'u', content_item_kinds: ['unknown'] } }),
        event('event_msg', { type: 'token_usage_record', thread_id: 't', turn_id: 'u', response_id: 'r', usage: { input_tokens: 100, cached_input_tokens: 20, cache_write_input_tokens: 0, output_tokens: 10, reasoning_output_tokens: 2, total_tokens: 110 } }),
        ...['read1', 'read2'].flatMap(call_id => [
            event('response_item', { type: 'function_call', call_id, name: 'read_file', arguments: JSON.stringify({ path: skill }) }),
            event('response_item', { type: 'function_call_output', call_id, output: 'Synthetic content' }),
        ]),
        event('event_msg', { type: 'item_completed', thread_id: 't', turn_id: 'u', item: { type: 'mcpToolCall', id: 'm1', server: 'docs', tool: 'search', status: 'completed' } }),
        ...['m1', 'failed', 'retry'].map(id => event('event_msg', { type: 'item_completed', thread_id: 't', turn_id: 'u', item: { type: 'mcpToolCall', id, server: 'docs', tool: 'search', status: id === 'failed' ? 'failed' : 'completed' } })),
        event('response_item', { type: 'function_call', call_id: 'ambiguous', name: 'mcp__docs__search', arguments: '{}' }),
        event('response_item', { type: 'function_call', call_id: 'failed-read', name: 'read_file', status: 'failed', arguments: JSON.stringify({ path: failedSkill }) }),
        event('event_msg', { type: 'task_complete', turn_id: 'u' }),
    ];
    await writeFile(path.join(root, 'sessions/one.jsonl'), rows.map(r => JSON.stringify(r)).join('\n') + '\n');
    const saved = { WOMBAT_DATA_HOME: process.env.WOMBAT_DATA_HOME, CODEX_HOME: process.env.CODEX_HOME, WOMBAT_AUTO_PRICES: process.env.WOMBAT_AUTO_PRICES };
    Object.assign(process.env, { WOMBAT_DATA_HOME: data, CODEX_HOME: root, WOMBAT_AUTO_PRICES: '0' });
    const binary = path.resolve('dist', process.platform === 'win32' ? 'wombat-core.exe' : 'wombat-core');


    const client = createNodeClient({ binaryPath: binary });
    const nativeEntry = projectTestClient(client, [root], [project]);

    const nativeApi = nativeEntry;
    const scope = { since: '2026-09-29', until: '2026-09-30', timezone: 'UTC' };
    try {
        const usage = await nativeApi.live!({ query: { action: 'usage', scope }, mode: 'fresh' });
        const result = await nativeApi.config!({ action: 'list', scope });
        assert.equal(result.items.length, 5);
        assert.equal(result.summary.currentItems, 4, 'missing paths are disclosed without being counted as present configuration');
        assert.equal(result.items.find(i => i.path === path.join(root, 'AGENTS.md'))!.measurementStatus, 'missing');
        const item = result.items.find(i => i.name === 'review')!;
        const combined = spawnSync(process.execPath, [path.resolve('dist/wombat.js'), 'system', 'call'], { env: process.env, input: JSON.stringify({ method: 'config', params: { action: 'list', roots: [root], projectRoots: [project], scope, kinds: ['rule', 'skill', 'mcp', 'hook'], limit: 20 } }), encoding: 'utf8', timeout: 10000 });
        assert.ok([0, 2].includes(combined.status!), combined.stderr + combined.stdout);
        const inventory = JSON.parse(combined.stdout);
        assert.deepEqual(inventory.items.map((i: {
            id: string;
        }) => i.id).sort(), result.items.map(i => i.id).sort(), 'One multi-kind view preserves the complete requested inventory');
        const webInventory = await nativeApi.config!({ action: 'list', kinds: ['rule', 'skill', 'mcp', 'hook'], scope });
        assert.deepEqual(webInventory.items.map(i => i.id).sort(), inventory.items.map((i: {
            id: string;
        }) => i.id).sort());
        const nodeRetained = await client.config!({ action: 'list', roots: [root], projectRoots: [project], kind: 'skill', scope });
        assert.equal('readView' in nodeRetained,false);
        const retained = await nativeApi.config!({ action: 'list', kind: 'skill', scope });
        assert.equal('readView' in retained,false);
        assert.ok(retained.items.every(i => i.kind === 'skill'));
        await assert.rejects(nativeApi.config!({ action: 'list', scope,readView:'removed' } as never), { code: 'INVALID_ARGUMENT' });
        const failedItem = result.items.find(i => i.name === 'failed')!;
        assert.equal(failedItem.counts.failed, 1);
        assert.equal(failedItem.observation, 'used', 'a dispatched read is a use even when its outcome is failed');
        assert.equal(failedItem.usageCount, 1);
        assert.equal(failedItem.counts.succeeded, 0, 'use does not establish successful loading');
        assert.equal(failedItem.useBasis?.status, 'observed');
        assert.equal(item.counts.fileReads, 2);
        assert.equal(item.counts.toolCalls, 0);
        // Each canonical dispatched read counts; declarations and catalogs do not.
        assert.equal(item.configuredState, 'enabled');
        assert.equal(item.observation, 'used');
        assert.equal(item.usageCount, 2);
        assert.equal(item.useBasis?.status, 'observed');
        assert.deepEqual(item.useBasis?.coverage, { dispatchGaps: 0, identityGaps: 0, targetGaps: 0, timeGaps: 0, turnGaps: 0 });
        assert.equal(item.counts.outcomeUnknown, 2);
        assert.equal(item.counts.succeeded, 0);
        assert.equal(item.relatedTurns, 1);
        assert.equal(item.relatedTasks, 1);
        assert.equal(item.usage?.tokens.total, 110);
        assert.equal(result.items.find(i => i.kind === 'mcp')!.counts.toolCalls, 3, 'duplicate evidence counts once; failed attempts and distinct retries each count');
        const mcp = result.items.find(i => i.kind === 'mcp')!;
        assert.equal(mcp.usageCount, 3, 'a prefix-only function name does not establish an MCP operation');
        assert.equal(mcp.useBasis?.status, 'observed');
        assert.deepEqual(mcp.useBasis?.coverage, { dispatchGaps: 0, identityGaps: 0, targetGaps: 0, timeGaps: 0, turnGaps: 0 });
        assert.equal(result.items.find(i => i.kind === 'mcp')!.counts.failed, 1);
        assert.equal(result.summary.usage?.tokens.total, 110, 'shared turns must be counted once');
        const rule = result.items.find(i => i.kind === 'rule' && i.current)!;
        assert.equal(rule.observation, 'loaded_only');
        assert.equal(rule.counts.fileReads, 0, 'instruction loading is evidence of loading, not a dispatched file read');
        assert.equal(rule.usage?.tokens.total, 110);
        const ruleEvidence = await nativeApi.config!({ action: 'evidence', itemId: rule.id, scope });
        assert.equal(ruleEvidence.evidence.length, 1);
        assert.equal(ruleEvidence.evidence[0].eventType, 'instruction_load');
        assert.equal(ruleEvidence.evidence[0].outcome, 'completed');
        assert.ok(!JSON.stringify(result).includes('SYNTHETIC_RULE_BODY_PRIVATE'));
        assert.equal(result.coverage.absenceObservable, false);
        assert.ok(!JSON.stringify(result).includes('SYNTHETIC_SECRET'));
        const request: ConfigRequest = { action: 'evidence', itemId: item.id, scope, limit: 1 };
        // Each call collects current facts without retaining a process lease.
        const first = await nativeApi.config!(request);
        assert.equal(first.page.total, 3);
        assert.equal(first.page.nextOffset, 1);
        const allEvidence = await nativeApi.config!({ ...request, limit: 10 });
        assert.deepEqual(allEvidence.evidence.map(row => row.eventType).sort(), ['file_read', 'file_read', 'skill_available'], 'availability is retained separately; a declared use is not a dispatched read');
        const next = await nativeApi.config!({ ...request, offset: 1 });
        assert.notEqual(first.evidence[0].id, next.evidence[0].id);
        const threadId = first.evidence[0].threadId, turnId = first.evidence[0].turnId!;
        for (let n = 0; n < 9; n++) {
            await appendFile(path.join(root, 'sessions/one.jsonl'), JSON.stringify(event('event_msg', { type: 'token_usage_record', thread_id: 't', turn_id: 'u', response_id: `added-${n}`, usage: { input_tokens: 100, cached_input_tokens: 20, cache_write_input_tokens: 0, output_tokens: 10, reasoning_output_tokens: 2, total_tokens: 110 } })) + '\n');
            await nativeApi.live!({ query: { action: 'usage', scope }, mode: 'fresh' });
        }
        const turns = await nativeApi.live!({ query: { action: 'turns', threadId, locateTurnId: turnId }, mode: 'cached' });
        assert.equal(turns.result.items[0].kind, 'turn');
        assert.equal(turns.result.items[0].id, turnId);
        assert.equal(turns.result.summary.tokens.total, 1100, 'current configuration uses the latest committed usage');
        const cliTurn = spawnProductCli(path.resolve('dist/wombat.js'), queryInvocation({"action":"turns","threadId":threadId}, {mode:"auto"}), { encoding: 'utf8', env: process.env, timeout: 10000 });
        assert.equal(cliTurn.status, 0, cliTurn.stderr + cliTurn.stdout);
        assert.equal(JSON.parse(cliTurn.stdout).summary.tokens.total, 1100, 'CLI reads the latest committed usage');
        const reverse = await nativeApi.config!({ action: 'list', scope: { ...scope, threadId } });
        assert.equal(reverse.items.length, 4);
        assert.ok((await nativeApi.config!({action:'list'})).items.length > 0);
        await assert.rejects(nativeApi.config!({action:'list',readView:'removed'} as unknown as ConfigRequest),{code:'INVALID_ARGUMENT'});
        const cli = spawnProductCli(path.resolve('dist/wombat.js'), {method:'config',params:{action:'list',roots:[root],projectRoots:[project],scope:{...scope,timezone:'UTC'}}}, { encoding: 'utf8', env: process.env, timeout: 10000 });
        assert.equal(cli.status, 2, cli.stderr + cli.stdout);
        assert.equal(JSON.parse(cli.stdout).summary.currentItems,result.summary.currentItems);
        assert.equal(JSON.parse(cli.stdout).summary.usage.tokens.total,1100);
        await writeFile(skill, 'Changed synthetic review instruction');
        const changed = await nativeApi.config!({ action: 'list', scope });
        assert.notEqual(changed.configRevision, result.configRevision);
        assert.notEqual((await nativeApi.config!({ ...request, action: 'detail' })).items[0].contentHash, item.contentHash);
        await writeFile(path.join(root, 'config.toml'), '[malformed');
        const failed = await nativeApi.config!({ action: 'list', scope });
        assert.equal(failed.items.find(i => i.kind === 'mcp')!.stale, true);
        assert.ok(failed.coverage.issues.some(i => i.code === 'configInvalid'));
        await rm(skill);
        const removed = await nativeApi.config!({ action: 'list', scope });
        assert.equal(removed.items.find(i => i.id === item.id)!.current, false);
        const unselected=path.join(dir,'unselected');
        await mkdir(unselected);
        const unselectedRule=path.join(unselected,'AGENTS.md');
        await writeFile(unselectedRule,'Unselected synthetic rule.');
        const outside=await nativeApi.config!({action:'list',scope:{...scope,project:unselected}});
        assert.ok(!outside.items.some(item=>item.path===unselectedRule),'A filter must not add a project root');
        const outsideDate = await nativeApi.config!({ action: 'list', scope: { since: '2026-09-30', until: '2026-10-01', timezone: 'UTC' } });
        assert.equal(outsideDate.summary.usage, null);
        for (const filename of await readdir(path.join(data, 'config-v2')))
            assert.ok(!(await readFile(path.join(data, 'config-v2', filename), 'utf8')).includes('SYNTHETIC_SECRET'));
    }
    finally {

        for (const [key, value] of Object.entries(saved))
            if (value === undefined)
                delete process.env[key];
            else
                process.env[key] = value;
        await rm(dir, { recursive: true, force: true, maxRetries: 20, retryDelay: 100 });
    }
});
