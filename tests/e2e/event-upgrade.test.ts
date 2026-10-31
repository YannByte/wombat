import { projectTestClient } from '../helpers/project-client.js';
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { realpathSync } from 'node:fs';
import { mkdtemp, mkdir, writeFile, appendFile, readFile, rename, rm, stat } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { spawn, spawnSync, type ChildProcess } from 'node:child_process';
import { once } from 'node:events';
import { createNodeClient } from '@wombat/client/node';
import type { TimingRequest, TimingResult, TimingLocalResult, TimingShareResult, UsageClient } from '@wombat/client';
const epoch = Date.parse('2026-10-03T00:00:00Z');
const nativeThread = 'synthetic-native-thread', nativeTurn = 'synthetic-native-turn';
const body = 'SYNTHETIC_PRIVATE_MESSAGE_BODY', title = 'SYNTHETIC_PRIVATE_TITLE';
const server = 'synthetic-private-server', tool = 'synthetic-private-tool';
const row = (type: string, payload: unknown, ms = 0) => ({ type, timestamp: new Date(epoch + ms).toISOString(), payload });
const jsonl = (rows: unknown[]) => rows.map(value => JSON.stringify(value) + '\n').join('');
const usage = (response: string, ms: number) => row('event_msg', {
    type: 'token_usage_record', thread_id: nativeThread, turn_id: nativeTurn, response_id: response,
    usage: { input_tokens: 100, cached_input_tokens: 20, cache_write_input_tokens: 0, output_tokens: 10, reasoning_output_tokens: 2, total_tokens: 110 },
}, ms);
const read = (skill: string, id: string, ms: number, failed = false) => [
    row('response_item', { type: 'function_call', call_id: id, name: 'read_file', arguments: JSON.stringify({ path: skill }) }, ms),
    row('response_item', { type: 'function_call_output', call_id: id, output: { isError: failed, content: body } }, ms + 1),
];
function fixture(project: string, skill: string): unknown[] {
    const changed = (name: string) => path.join(project, 'reported-private', name);
    const changes = {
        [changed('a.rs')]: { type: 'add', content: body },
        [changed('b.rs')]: { type: 'delete', content: body },
        [changed('c.rs')]: { type: 'update', unified_diff: body, move_path: changed('d.rs') },
    };
    const command = (phase: 'item_started' | 'item_completed', ms: number) => row('event_msg', {
        type: phase, turn_id: nativeTurn,
        started_at_ms: epoch + 100, ...(phase === 'item_completed' ? { completed_at_ms: epoch + 300 } : {}),
        item: { type: 'CommandExecution', id: 'synthetic-native-command', source: 'agent', cwd: project,
            parsed_cmd: [], command: ['SYNTHETIC_PRIVATE_COMMAND'], aggregated_output: body,
            status: phase === 'item_started' ? 'in_progress' : 'completed',
            ...(phase === 'item_completed' ? { exit_code: 0, duration: { secs: 0, nanos: 200000000 } } : {}),
        },
    }, ms);
    const patch = (phase: 'item_started' | 'item_completed', ms: number) => row('event_msg', {
        type: phase, turn_id: nativeTurn,
        item: { type: 'FileChange', id: 'synthetic-native-patch', changes,
            status: phase === 'item_started' ? 'in_progress' : 'completed', stdout: body, stderr: body },
    }, ms);
    // Independent oracle: one 110-token measurement; three dispatched Skill reads,
    // one explicit MCP invocation, one command and one terminal patch = six operations.
    // The failed read remains one use; each call/output and item start/end is one operation.
    return [
        row('session_meta', { id: nativeThread, cwd: project }),
        row('turn_context', { turn_id: nativeTurn, model: 'gpt-5.4', effort: 'low' }),
        row('event_msg', { type: 'task_started', turn_id: nativeTurn }),
        row('response_item', { type: 'message', id: 'synthetic-native-message', role: 'assistant', content: [{ type: 'output_text', text: body }] }, 50),
        command('item_started', 100), command('item_completed', 300),
        ...read(skill, 'synthetic-native-read-one', 400),
        ...read(skill, 'synthetic-native-read-two', 450, true),
        ...read(skill, 'synthetic-native-read-three', 500),
        row('event_msg', { type: 'mcp_tool_call_end', call_id: 'synthetic-native-mcp', turn_id: nativeTurn,
            invocation: { server, tool, arguments: { private: body } }, duration: { secs: 0, nanos: 2000000 },
            result: { Ok: { content: [{ type: 'text', text: body }], isError: false } },
        }, 600),
        patch('item_started', 700), patch('item_completed', 800), usage('synthetic-native-response-one', 900),
        row('event_msg', { type: 'task_complete', turn_id: nativeTurn, duration_ms: 1200, time_to_first_token_ms: 0, last_agent_message: body }, 1000),
    ];
}
async function stop(child: ChildProcess | undefined): Promise<void> {
    if (!child || child.exitCode !== null || child.signalCode !== null)
        return;
    const closed = once(child, 'close');
    child.kill('SIGTERM');
    let kill: ReturnType<typeof setTimeout> | undefined, deadline: ReturnType<typeof setTimeout> | undefined;
    try {
        kill = setTimeout(() => child.kill('SIGKILL'), 2000);
        await Promise.race([closed, new Promise<never>((_, reject) => { deadline = setTimeout(() => reject(new Error('Synthetic core did not stop')), 4000); })]);
    }
    finally {
        clearTimeout(kill);
        clearTimeout(deadline);
    }
}
async function environment() {
    const dir = realpathSync.native(await mkdtemp(path.join(tmpdir(), 'wombat-event-upgrade-')));
    const root = path.join(dir, 'source'), project = path.join(dir, 'project'), data = path.join(dir, 'data');
    const skill = path.join(project, '.agents', 'skills', 'synthetic-example', 'SKILL.md');
    const file = path.join(root, 'sessions', 'synthetic.jsonl');
    const binary = path.resolve('dist', process.platform === 'win32' ? 'wombat-core.exe' : 'wombat-core');
    const keys = ['WOMBAT_DATA_HOME', 'CODEX_HOME', 'WOMBAT_AUTO_PRICES', 'WOMBAT_CODEX_BIN', 'WOMBAT_CORE_BIN'] as const;
    const saved = keys.map(key => [key, process.env[key]] as const);
    let child: ChildProcess | undefined, nativeEntry: UsageClient | undefined;
    let hidden = false;
    const close = async () => {
        try { }
        finally {
            try {
                await stop(child);
            }
            finally {
                try {
                    if (hidden)
                        await rename(path.join(root, 'sessions-hidden'), path.join(root, 'sessions'));
                }
                finally {
                    for (const [key, value] of saved) {
                        if (value === undefined)
                            delete process.env[key];
                        else
                            process.env[key] = value;
                    }
                    await rm(dir, { recursive: true, force: true, maxRetries: 10, retryDelay: 100 });
                }
            }
        }
    };
    try {
        await mkdir(path.dirname(file), { recursive: true });
        await mkdir(path.dirname(skill), { recursive: true });
        await writeFile(skill, '---\nname: synthetic-example\ndescription: Synthetic fixture\n---\nSynthetic instructions.\n');
        await writeFile(path.join(root, 'config.toml'), `[mcp_servers.${server}]\ncommand='synthetic-never-executed'\n`);
        await writeFile(path.join(root, 'session_index.jsonl'), jsonl([{ id: nativeThread, thread_name: title, updated_at: new Date(epoch).toISOString() }]));
        await writeFile(file, jsonl(fixture(project, skill)));
        Object.assign(process.env, { WOMBAT_DATA_HOME: data, CODEX_HOME: root, WOMBAT_AUTO_PRICES: '0', WOMBAT_CODEX_BIN: path.join(dir, 'absent-native-codex'), WOMBAT_CORE_BIN: binary });
        const start = async () => {
            child = spawn(binary, ['--serve-usage'], { stdio: 'ignore', env: process.env });
            await once(child, 'spawn');
        };
        await start();
        const node = createNodeClient({ binaryPath: binary, automaticPrices: false, timeoutMs: 15000 });
        nativeEntry = projectTestClient(node, [root], [project]);


        const nativeTransport = nativeEntry;
        return { dir, root, project, data, skill, file, node, nativeTransport, close, stopService: () => stop(child), startService: start,
            restart: async () => { await stop(child); await start(); },
            hideSource: async () => { await rename(path.join(root, 'sessions'), path.join(root, 'sessions-hidden')); hidden = true; },
            restoreSource: async () => { await rename(path.join(root, 'sessions-hidden'), path.join(root, 'sessions')); hidden = false; },
        };
    }
    catch (error) {
        await close();
        throw error;
    }
}
function local(result: TimingResult): TimingLocalResult {
    assert.equal(result.action, 'summary');
    assert.equal(result.profile, 'local');
    if (result.action !== 'summary' || result.profile !== 'local')
        throw new Error('Expected local timing');
    return result;
}
function share(result: TimingResult): TimingShareResult {
    assert.equal(result.action, 'summary');
    assert.equal(result.profile, 'share-v1');
    if (result.action !== 'summary' || result.profile !== 'share-v1')
        throw new Error('Expected share timing');
    return result;
}
async function timing(client: UsageClient, request: TimingRequest): Promise<TimingResult> {
    assert.ok(client.timing);
    return client.timing(request, { signal: AbortSignal.timeout(15000) });
}
type EvidenceRequest = Extract<TimingRequest, {
    action: 'evidence';
}>;
type EvidencePage = Extract<TimingResult, {
    action: 'evidence';
}>;
async function pages(client: UsageClient, request: EvidenceRequest): Promise<EvidencePage[]> {
    const result: EvidencePage[] = [], cursors = new Set<string>();
    for (let index = 0; index < 100; index++) {
        const page = await timing(client, request);
        assert.equal(page.action, 'evidence');
        if (page.action !== 'evidence')
            throw new Error('Expected evidence page');
        assert.equal(page.collection, request.collection);
        assert.equal(page.snapshotId, request.snapshotId);
        assert.equal(page.scope.threadId, request.threadId);
        assert.equal(page.scope.turnId, request.turnId);
        assert.ok(page.rows.length <= (request.limit ?? 50));
        result.push(page);
        if (!page.nextCursor)
            return result;
        assert.ok(!cursors.has(page.nextCursor.token), 'cursor must advance');
        cursors.add(page.nextCursor.token);
        request = { ...request, cursor: page.nextCursor };
    }
    throw new Error('Synthetic evidence pagination exceeded 100 pages');
}
function cli(args: string[]): unknown {
    const result = spawnSync(process.execPath, [path.resolve('dist/wombat.js'), ...args, '--json'], {
        env: process.env, encoding: 'utf8', timeout: 15000, maxBuffer: 2 * 1024 * 1024,
    });
    assert.ifError(result.error);
    assert.ok(result.status === 0 || result.status === 2, result.stderr + result.stdout);
    return JSON.parse(result.stdout);
}
function counts(value: TimingLocalResult, appended = false) {
    assert.equal(value.time.nativeWallClockMs.value, 1200);
    assert.equal(value.time.nativeWallClockMs.basis, 'native_record');
    assert.equal(value.time.derivedWallClockMs.value, 1000);
    assert.equal(value.time.derivedWallClockMs.basis, 'explicit_boundary');
    assert.equal(value.time.nativeTtftMs.value, 0);
    assert.equal(value.time.firstContentRecordDelayMs.value, 50);
    assert.equal(value.time.boundaryDiscrepancyMs.value, 200);
    assert.equal(value.time.command.unionMs.value, 200);
    assert.equal(value.time.command.sumMs.value, 200);
    assert.equal(value.work.operationCandidates.value, appended ? 7 : 6);
    assert.equal(value.work.closedOperations.value, appended ? 7 : 6);
    assert.equal(value.work.failedOperations.value, 1);
    assert.equal(value.work.fileChangeRecords.value, 1);
    assert.equal(value.work.changedFiles.value, 4);
    assert.equal(value.work.addedLines.value, null);
    assert.equal(value.work.removedLines.value, null);
    assert.equal(value.work.messageRecordCandidates.value, 1);
    assert.equal(value.work.nonemptyVisibleContentRecords.value, 1);
    assert.equal(value.work.unknownContentRecords.value, 0);
    assert.equal(value.work.missingContentTimeRecords.value, 0);
    assert.equal(value.uses.totals.methodVersion, 3);
    assert.equal(value.uses.totals.objectCount.value, 2);
    assert.equal(value.uses.totals.recordCount.value, appended ? 5 : 4);
    const skill = value.uses.objects.find(object => object.kind === 'skill'), mcp = value.uses.objects.find(object => object.kind === 'mcp');
    assert.ok(skill);
    assert.ok(mcp);
    assert.equal(skill.state, 'used');
    assert.equal(mcp.state, 'used');
    assert.equal(skill.useCount.value, appended ? 4 : 3);
    assert.equal(skill.associatedUseCount.value, appended ? 4 : 3);
    assert.equal(mcp.useCount.value, 1);
    assert.equal(mcp.server, server);
    assert.equal(value.uses.totals.unassignedSkillRecords.value, 0);
    assert.equal(value.uses.totals.unassignedMcpRecords.value, 0);
}
function whitelist(value: TimingShareResult, sensitive: string[]) {
    // The generated share schema is checked by UsageClient; this independent audit
    // rejects local navigation/identity keys recursively, including future additions.
    assert.deepEqual(Object.keys(value).sort(), ['action', 'basisCollections', 'capabilities', 'context', 'coverage', 'findings', 'freshness', 'methodVersion', 'outputVersion', 'privacy', 'profile', 'quality', 'relativeAnchors', 'scope', 'time', 'uses', 'work'].sort());
    const forbidden = new Set(['readView', 'snapshotId', 'sourceInstanceId', 'threadId', 'turnId', 'path', 'project', 'server', 'tool', 'title', 'name', 'objects', 'rows', 'cursor', 'nextCursor', 'objectRef', 'nativeId', 'callId', 'itemId', 'collectedAt', 'timestampMs']);
    const walk = (item: unknown) => {
        if (Array.isArray(item)) {
            item.forEach(walk);
            return;
        }
        if (item && typeof item === 'object')
            for (const [key, child] of Object.entries(item)) {
                assert.ok(!forbidden.has(key), `share leaked ${key}`);
                walk(child);
            }
    };
    walk(value);
    const text = JSON.stringify(value);
    for (const marker of sensitive)
        assert.ok(!text.includes(marker), `share leaked synthetic marker ${marker}`);
    assert.equal(value.relativeAnchors.startMs.value, 0);
    assert.equal(value.relativeAnchors.endMs.value, 1000);
    assert.equal(value.uses.objectCount.value, 2);
    assert.equal(value.uses.recordCount.value, 4);
    assert.equal(value.work.operationCandidates.value, 6);
}
test('fixed native events have independent token/time/work/use truth through Node and CLI', { timeout: 90000 }, async () => {
    const env = await environment();
    try {
        const first = await env.nativeTransport.live!({ query: { action: 'usage', scope: { allTime: true } }, mode: 'fresh' });
        const snapshotId = first.result.snapshotRef.snapshotId;
        assert.equal(first.result.summary.tokens.total, 110);
        assert.equal(first.result.summary.tokens.cacheRead, 20);
        assert.equal(first.result.summary.tokens.input, 80);
        assert.equal(first.result.summary.tokens.output, 10);
        const threads = await env.nativeTransport.live!({ query: { action: 'threads', snapshotId, scope: { allTime: true } }, mode: 'cached' });
        const thread = threads.result.items.find(item => item.kind === 'thread');
        assert.ok(thread);
        assert.equal(thread.title, title);
        const turns = await env.nativeTransport.live!({ query: { action: 'turns', snapshotId, threadId: thread.id, scope: { allTime: true } }, mode: 'cached' });
        const turn = turns.result.items.find(item => item.kind === 'turn');
        assert.ok(turn);
        const request = { action: 'summary', snapshotId, threadId: thread.id, turnId: turn.id, mode: 'cached', privacyProfile: 'local' } as const;
        const initial = local(await timing(env.nativeTransport, request));
        counts(initial);
        assert.equal(initial.readView.snapshotId, snapshotId);
        assert.equal(initial.scope.wholeTurn, true);
        const node = local(await timing(env.node, { ...request, roots: [env.root] }));
        counts(node);
        assert.deepEqual(node.time, initial.time);
        assert.deepEqual(node.work, initial.work);
        assert.deepEqual(node.uses, initial.uses);
        const args = ['timing', 'summary', '--snapshot', snapshotId, '--thread', thread.id, '--turn', turn.id, '--cached'];
        assert.deepEqual(cli(args), initial, 'all entries use the same fixed evidence, not separately selected latest views');
        const evidence = { action: 'evidence', snapshotId, threadId: thread.id, turnId: turn.id, limit: 2 } as const;
        const events = await pages(env.nativeTransport, { ...evidence, collection: 'turn_events' });
        const eventRows = events.flatMap(page => { assert.equal(page.collection, 'turn_events'); if (page.collection !== 'turn_events')
            throw new Error('Wrong event collection'); return page.rows; });
        assert.ok(events.length > 1);
        assert.equal(new Set(eventRows.map(item => item.reference)).size, eventRows.length);
        assert.equal(eventRows.length, initial.coverage.scopedEvents.value);
        for (const page of events)
            assert.equal(page.total.value, eventRows.length);
        assert.equal(eventRows.filter(item => item.recordKind === 'measurement').length, 1);
        assert.equal(eventRows.filter(item => item.recordKind === 'operation').length, 11, 'six read endpoints, one MCP result and four command/patch endpoints');
        assert.equal(eventRows.filter(item => item.recordKind === 'turn' && item.phase === 'started').length, 1);
        assert.equal(eventRows.filter(item => item.recordKind === 'turn' && item.phase === 'completed').length, 1);
        assert.equal(eventRows.filter(item => item.recordKind === 'command').length, 2);
        assert.equal(eventRows.filter(item => item.recordKind === 'file').length, 2);
        const objects = await pages(env.nativeTransport, { ...evidence, collection: 'use_objects', limit: 1 });
        assert.equal(objects.length, 2);
        for (const page of objects) {
            if (page.collection !== 'use_objects')
                throw new Error('Wrong object collection');
            assert.equal(page.total.value, 2);
            assert.deepEqual(page.totals, initial.uses.totals);
        }
        const records = await pages(env.nativeTransport, { ...evidence, collection: 'use_records' });
        const recordRows = records.flatMap(page => { if (page.collection !== 'use_records')
            throw new Error('Wrong record collection'); return page.rows; });
        assert.equal(recordRows.length, 4);
        assert.equal(new Set(recordRows.map(item => item.reference)).size, 4);
        assert.equal(recordRows.filter(item => item.kind === 'skill_read').length, 3);
        assert.equal(recordRows.filter(item => item.kind === 'mcp_tool').length, 1);
        assert.equal(recordRows.find(item => item.kind === 'mcp_tool')?.tool, tool);
        assert.ok(recordRows.every(item => item.identityKnown && item.replayOf == null && !item.targetConflict));
        assert.deepEqual(recordRows.map(item => item.outcome).sort(), ['completed', 'completed', 'completed', 'failed']);
        for (const page of records) {
            if (page.collection !== 'use_records')
                throw new Error('Wrong record collection');
            assert.equal(page.total.value, 4);
            assert.deepEqual(page.totals, initial.uses.totals);
        }
        const skill = initial.uses.objects.find(object => object.kind === 'skill')!;
        const filtered = await pages(env.nativeTransport, { ...evidence, collection: 'use_records', objectRef: skill.objectRef });
        const skillRows = filtered.flatMap(page => { if (page.collection !== 'use_records')
            throw new Error('Wrong record collection'); assert.equal(page.objectRef, skill.objectRef); assert.equal(page.total.value, 3); assert.equal(page.totals.recordCount.value, 4); return page.rows; });
        assert.equal(skillRows.length, 3);
        assert.ok(skillRows.every(item => item.objectRef === skill.objectRef));
        assert.equal(skillRows.filter(item => item.outcome === 'failed').length, 1);
        const cliPage = cli(['timing', 'evidence', '--snapshot', snapshotId, '--thread', thread.id, '--turn', turn.id, '--collection', 'use_records', '--object', skill.objectRef, '--limit', '2']);
        assert.deepEqual(cliPage, filtered[0]);
        const shared = share(await timing(env.nativeTransport, { ...request, privacyProfile: 'share-v1' }));
        whitelist(shared, [body, title, server, tool, env.dir, env.skill, nativeThread, nativeTurn, 'synthetic-native-command', 'synthetic-native-patch', 'synthetic-native-mcp', 'synthetic-native-read-one', 'synthetic-native-message']);
        const cliShared = cli([...args, '--share']);
        assert.ok(cliShared && typeof cliShared === 'object' && 'scope' in cliShared);
        const cliScope = cliShared.scope;
        assert.ok(cliScope && typeof cliScope === 'object' && 'taskAlias' in cliScope && typeof cliScope.taskAlias === 'string');
        const httpNonce = shared.scope.taskAlias.slice(2), cliNonce = cliScope.taskAlias.slice(2);
        assert.notEqual(cliNonce, httpNonce, 'each share export gets fresh, unlinkable aliases');
        // Only export-local random alias namespaces differ across the two entries.
        // Preserve all metric, privacy, and within-export reference relationships.
        const normalize = (value: unknown, nonce: string) => JSON.parse(JSON.stringify(value).replaceAll(nonce, 'EXPORT')) as unknown;
        assert.deepEqual(normalize(cliShared, cliNonce), normalize(shared, httpNonce));
        // A second measurement and fourth distinct read belong to the same native turn.
        // They change the new version's token/work/use facts; the original boundary
        // remains 1200 ms native and 1000 ms derived in both complete groups.
        await appendFile(env.file, jsonl([
            row('turn_context', { turn_id: nativeTurn, model: 'gpt-5.4', effort: 'low' }, 1050),
            ...read(env.skill, 'synthetic-native-read-four', 1100), usage('synthetic-native-response-two', 1150),
        ]));
        const current = await env.nativeTransport.live!({ query: { action: 'usage', scope: { allTime: true } }, mode: 'fresh' });
        assert.notEqual(current.result.snapshotRef.snapshotId, snapshotId);
        assert.equal(current.result.summary.tokens.total, 220);
        const freshRequest = { ...request, snapshotId: current.result.snapshotRef.snapshotId };
        const fresh = local(await timing(env.nativeTransport, freshRequest));
        counts(fresh, true);
        assert.equal(fresh.readView.snapshotId, current.result.snapshotRef.snapshotId);
        assert.deepEqual(fresh.scope, initial.scope);
        const frozen = local(await timing(env.nativeTransport, request));
        counts(frozen);
        assert.deepEqual(frozen.time, initial.time);
        assert.deepEqual(frozen.work, initial.work);
        assert.deepEqual(frozen.uses, initial.uses);
        assert.deepEqual(await pages(env.nativeTransport, { ...evidence, collection: 'use_records' }), records);
        const frozenUsage = await env.nativeTransport.live!({ query: { action: 'usage', snapshotId, scope: { allTime: true } }, mode: 'cached' });
        assert.equal(frozenUsage.result.summary.tokens.total, 110);
        // Moving only this synthetic sessions directory makes rescanning observable.
        await env.hideSource();
        const offline = local(await timing(env.nativeTransport, request));
        counts(offline);
        assert.deepEqual(offline.uses, initial.uses);
        const offlineUsage = await env.nativeTransport.live!({ query: { action: 'usage', snapshotId, scope: { allTime: true } }, mode: 'cached' });
        assert.equal(offlineUsage.result.summary.tokens.total, 110);
        assert.deepEqual(cli(args), offline);
        await env.restoreSource();
    }
    finally {
        await env.close();
    }
});
test('service restart and derived-index rebuild preserve decisions, reasons and the original immutable review baseline', { timeout: 60000 }, async () => {
    const env = await environment();
    try {
        const large = `---\nname: synthetic-example\ndescription: ${'字'.repeat(501)}\n---\n${'x '.repeat(4999)}x`;
        await writeFile(env.skill, large);
        const initial = await env.nativeTransport.optimize!({ action: 'list' });
        const suggestion = initial.suggestions.find(item => item.item.path === env.skill);
        assert.ok(suggestion);
        assert.deepEqual(suggestion.findings.map(finding => finding.rule), ['descriptionSize', 'bodyTokens']);
        assert.ok(suggestion.reviewBaseline);
        assert.equal(suggestion.reviewBaseline.item.contentHash, suggestion.item.contentHash);
        const kept = await env.nativeTransport.optimize!({ action: 'keep', suggestionId: suggestion.id, decisionReason: 'necessary', readView: initial.readView, decisionRevision: initial.decisionRevision });
        assert.equal(kept.pending, initial.pending - 1, 'keeping this Skill leaves unrelated findings pending');
        const history = await env.nativeTransport.optimize!({ action: 'list', group: 'history' });
        const decided = history.suggestions.find(item => item.id === suggestion.id);
        assert.ok(decided?.decision);
        assert.ok(decided.reviewBaseline);
        assert.equal(decided.decision.kind, 'keep');
        assert.equal(decided.decision.reason, 'necessary');
        assert.deepEqual(decided.reviewBaseline, suggestion.reviewBaseline);
        const originalBaseline = decided.reviewBaseline, originalDecision = decided.decision, originalRevision = history.decisionRevision;
        const userFile = path.join(env.data, 'user-v1', 'reviews.sqlite3');
        assert.ok((await stat(userFile)).size > 0);
        await env.restart();
        const restarted = await env.nativeTransport.optimize!({ action: 'list', group: 'history' });
        const restored = restarted.suggestions.find(item => item.id === suggestion.id);
        assert.ok(restored);
        assert.deepEqual(restored.decision, originalDecision);
        assert.deepEqual(restored.reviewBaseline, originalBaseline);
        assert.equal(restarted.decisionRevision, originalRevision);
        assert.equal(restarted.history, history.history);
        // Only these explicitly named, test-owned derived directories are removed.
        // The user-v1 database is never part of the deletion target or source move.
        await env.stopService();
        const userBytes = await readFile(userFile);
        for (const index of ['live-v2', 'config-v2'])
            await rm(path.join(env.data, index), { recursive: true, force: true });
        assert.deepEqual(await readFile(userFile), userBytes, 'derived-index removal must not touch user storage');
        await env.startService();
        const rebuilt = await env.nativeTransport.optimize!({ action: 'list' });
        assert.equal(rebuilt.pending, kept.pending);
        const rebuiltHistory = await env.nativeTransport.optimize!({ action: 'list', group: 'history' });
        const preserved = rebuiltHistory.suggestions.find(item => item.id === suggestion.id);
        assert.ok(preserved);
        assert.deepEqual(preserved.decision, originalDecision);
        assert.deepEqual(preserved.reviewBaseline, originalBaseline);
        assert.equal(rebuiltHistory.decisionRevision, originalRevision);
        assert.equal(rebuiltHistory.history, history.history);
        const cliHistory = cli(['optimize', 'history', '--root', env.root, '--project-root', env.project]);
        assert.ok(cliHistory && typeof cliHistory === 'object' && 'suggestions' in cliHistory && 'decisionRevision' in cliHistory);
        assert.deepEqual(cliHistory.suggestions, rebuiltHistory.suggestions);
        assert.equal(cliHistory.decisionRevision, originalRevision);
        await writeFile(env.skill, '---\nname: synthetic-example\ndescription: Fixed synthetic fixture\n---\nUse evidence.\n');
        const rechecked = await env.nativeTransport.optimize!({ action: 'recheck', group: 'history' });
        const checked = rechecked.suggestions.find(item => item.id === suggestion.id);
        assert.ok(checked);
        assert.equal(checked.status, 'verified');
        assert.ok(checked.checks.every(check => check.outcome === 'miss' && check.comparison.status === 'comparable'));
        assert.deepEqual(checked.reviewBaseline, originalBaseline);
        assert.deepEqual(checked.decision, originalDecision);
        assert.notEqual(checked.item.contentHash, originalBaseline.item.contentHash);
        await env.restart();
        const finalHistory = await env.nativeTransport.optimize!({ action: 'list', group: 'history' });
        const final = finalHistory.suggestions.find(item => item.id === suggestion.id);
        assert.ok(final);
        assert.equal(final.status, 'verified');
        assert.deepEqual(final.decision, originalDecision);
        assert.deepEqual(final.reviewBaseline, originalBaseline);
    }
    finally {
        await env.close();
    }
});
