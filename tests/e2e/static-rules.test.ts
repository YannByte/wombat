import { projectTestClient } from '../helpers/project-client.js';
import type { UsageClient } from '@wombat/client';
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { realpathSync } from 'node:fs';
import { mkdtemp, mkdir, writeFile, readFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { spawn, spawnSync } from 'node:child_process';
import { once } from 'node:events';
import { createNodeClient } from '@wombat/client/node';
test('local resource checks retain fixed evidence, expose independent outcomes and fail closed on unknown references', { timeout: 45000 }, async () => {
    const dir = realpathSync.native(await mkdtemp(path.join(tmpdir(), 'wombat-reference-checks-'))), root = path.join(dir, 'source'), project = path.join(dir, 'project'), skill = path.join(project, '.agents/skills/resources/SKILL.md'), data = path.join(dir, 'data');
    await mkdir(path.join(root, 'sessions'), { recursive: true });
    await mkdir(path.dirname(skill), { recursive: true });
    await writeFile(path.join(root, 'AGENTS.md'), 'Synthetic source instruction.\n');
    await writeFile(path.join(project, 'AGENTS.md'), 'Synthetic project instruction.\n');
    const body = '---\nname: resources\ndescription: Synthetic resources\n---\n[Forms](references/forms.md#fields)\n';
    await writeFile(skill, body);
    const saved = { WOMBAT_DATA_HOME: process.env.WOMBAT_DATA_HOME, CODEX_HOME: process.env.CODEX_HOME, WOMBAT_AUTO_PRICES: process.env.WOMBAT_AUTO_PRICES };
    Object.assign(process.env, { WOMBAT_DATA_HOME: data, CODEX_HOME: root, WOMBAT_AUTO_PRICES: '0' });
    const binary = path.resolve('dist', process.platform === 'win32' ? 'wombat-core.exe' : 'wombat-core'), service = spawn(binary, ['--serve-usage'], { stdio: 'ignore' });
    await once(service, 'spawn');
    let nativeEntry: UsageClient | undefined;
    try {
        nativeEntry = projectTestClient(createNodeClient({ binaryPath: binary, automaticPrices: false }), [root], [project]);
        const nativeApi = nativeEntry;
        const first = await nativeApi.optimize!({ action: 'list' });
        assert.equal(first.pending, 1);
        const suggestion = first.suggestions[0];
        assert.equal(suggestion.category, 'repair');
        const reference = suggestion.findings[0].evidence!.references![0];
        assert.equal(reference.target, 'references/forms.md');
        assert.equal(reference.startLine, 5);
        assert.equal(reference.status, 'referenceTargetMissing');
        const checks = await nativeApi.optimize!({ action: 'checks', readView: first.readView, itemId: suggestion.item.id, limit: 1 });
        assert.equal(checks.page.total, 1);
        assert.equal(checks.checks?.find(c => c.rule === 'localReference')?.outcome, 'hit');
        assert.equal(checks.checks?.find(c => c.rule === 'skillFormat')?.outcome, 'miss');
        assert.equal(checks.checks?.find(c => c.rule === 'skillInactivity')?.outcome, 'unsupported');
        const cli = spawnSync(process.execPath, [path.resolve('dist/wombat.js'), 'optimize', 'checks', '--root', root, '--project-root', project, '--read-view', first.readView!, '--item', suggestion.item.id, '--json'], { env: process.env, encoding: 'utf8', timeout: 10000 });
        assert.equal(cli.status, 2, cli.stderr + cli.stdout);
        assert.deepEqual(JSON.parse(cli.stdout).checks, checks.checks);
        await nativeApi.optimize!({ action: 'recheck', readView: first.readView, decisionRevision: first.decisionRevision, suggestionId: suggestion.id });
        await mkdir(path.join(path.dirname(skill), 'references'));
        await writeFile(path.join(path.dirname(skill), 'references/forms.md'), 'Synthetic reference.\n');
        const fixed = await nativeApi.optimize!({ action: 'recheck', group: 'history' }), fixedReference = fixed.suggestions[0], fixedReferenceCheck = fixedReference.checks.find(c => c.rule === 'localReference')!;
        assert.equal(fixedReference.status, 'recheckUnavailable', 'a missing problem-location identity cannot turn a current miss into a verified repair');
        assert.equal(fixed.pending, 1);
        assert.equal(fixedReferenceCheck.outcome, 'miss');
        assert.equal(fixedReferenceCheck.comparison.status, 'unknown');
        assert.equal(fixedReferenceCheck.comparison.reason, 'problemIdentityUnavailable');
        assert.equal(fixedReference.reviewBaseline?.item.contentHash, suggestion.item.contentHash, 'rechecking keeps the original reference observation as its baseline');
        assert.equal((await nativeApi.optimize!({ action: 'checks', readView: first.readView, itemId: suggestion.item.id })).checks?.find(c => c.rule === 'localReference')?.outcome, 'hit', 'a fixed view retains its original target observation');
        await rm(path.join(path.dirname(skill), 'references/forms.md'));
        const again = await nativeApi.optimize!({ action: 'list' });
        await nativeApi.optimize!({ action: 'recheck', readView: again.readView, decisionRevision: again.decisionRevision, suggestionId: again.suggestions[0].id });
        await writeFile(skill, body.replace('references/forms.md#fields', '$RESOURCE/forms.md'));
        const unknown = await nativeApi.optimize!({ action: 'recheck', group: 'history' });
        assert.equal(unknown.suggestions[0].status, 'recheckUnavailable');
        assert.equal(unknown.pending, 1, 'unknown evidence retains the original recommendation without fabricating a new fault');
        const unknownChecks = await nativeApi.optimize!({ action: 'checks', readView: unknown.readView, itemId: suggestion.item.id });
        assert.equal(unknownChecks.checks?.find(c => c.rule === 'localReference')?.outcome, 'insufficient');
        assert.equal(unknownChecks.checks?.find(c => c.rule === 'skillFormat')?.outcome, 'miss');
        await assert.rejects(nativeApi.optimize!({ action: 'checks', readView: unknown.readView, itemId: 'foreign' }), { code: 'NOT_FOUND' });
    }
    finally {
        if (service.exitCode === null && service.signalCode === null) {
            const ended = once(service, 'exit');
            service.kill();
            await ended;
        }
        for (const [k, v] of Object.entries(saved)) {
            if (v === undefined)
                delete process.env[k];
            else
                process.env[k] = v;
        }
        await rm(dir, { recursive: true, force: true });
    }
});
test('exact blocks, explicit copies, decision fingerprints and unavailable rechecks share real CLI/HTTP results', { timeout: 45000 }, async () => {
    const dir = realpathSync.native(await mkdtemp(path.join(tmpdir(), 'wombat-static-rules-'))), root = path.join(dir, 'source'), project = path.join(dir, 'project'), data = path.join(dir, 'data');
    await mkdir(path.join(root, 'sessions'), { recursive: true });
    await mkdir(path.join(project, 'copy'), { recursive: true });
    await mkdir(path.join(project, '.wombat'), { recursive: true });
    await writeFile(path.join(root, 'AGENTS.md'), 'Synthetic source instruction.\n');
    const source = path.join(project, 'AGENTS.md'), copy = path.join(project, 'copy/AGENTS.md'), declaration = path.join(project, '.wombat/analysis.json');
    const text = '完整中文指令，保留大小写 A。\r\n\r\n完整中文指令，保留大小写 A。\r\n\r\n' + 'x '.repeat(9000) + '\n';
    await writeFile(source, text);
    await writeFile(copy, '当前副本不同。\n');
    const manifest = JSON.stringify({ version: 1, copies: [{ id: 'user-copy', source: 'AGENTS.md', copy: 'copy/AGENTS.md', transform: 'identity-v1' }] });
    await writeFile(declaration, manifest);
    const previous = { WOMBAT_DATA_HOME: process.env.WOMBAT_DATA_HOME, CODEX_HOME: process.env.CODEX_HOME, WOMBAT_AUTO_PRICES: process.env.WOMBAT_AUTO_PRICES };
    Object.assign(process.env, { WOMBAT_DATA_HOME: data, CODEX_HOME: root, WOMBAT_AUTO_PRICES: '0' });
    const binary = path.resolve('dist', process.platform === 'win32' ? 'wombat-core.exe' : 'wombat-core');
    const service = spawn(binary, ['--serve-usage'], { stdio: 'ignore', env: process.env });
    await once(service, 'spawn');
    const nativeEntry = projectTestClient(createNodeClient({ binaryPath: binary, automaticPrices: false }), [root], [project]);

    const nativeApi = nativeEntry;
    try {
        const first = await nativeApi.optimize!({ action: 'list' });
        assert.equal(first.capabilities.exactInstructionBlocks, true);
        assert.equal(first.capabilities.declaredCopyDrift, true);
        assert.equal(first.capabilities.hookSupport.status, 'no_verified_adapter');
        assert.equal(first.capabilities.hookSupport.effectiveRegistry, false);
        assert.equal(first.pending, 2);
        const duplicate = first.suggestions.find(s => s.item.path === source)!;
        assert.deepEqual(duplicate.findings.map(f => f.rule), ['fileSize', 'exactInstructionBlocks']);
        const block = duplicate.findings.find(f => f.evidence)!.evidence!;
        assert.equal(block.applicability, 'sameFileSameHeading');
        assert.equal(block.positions.length, 2);
        const original = await readFile(source);
        for (const p of block.positions) {
            assert.equal(original.subarray(p.startByte, p.endByte).toString(), '完整中文指令，保留大小写 A。\r\n');
        }
        const drift = first.suggestions.find(s => s.item.path === copy)!;
        assert.equal(drift.category, 'repair');
        assert.equal(drift.findings[0].evidence?.versions.length, 2);
        assert.equal(drift.findings[0].evidence?.direction, 'sourceToCopy');
        const cli = spawnSync(process.execPath, [path.resolve('dist/wombat.js'), 'optimize', 'list', '--root', root, '--project-root', project, '--read-view', first.readView!, '--json'], { env: process.env, encoding: 'utf8', timeout: 10000 });
        assert.equal(cli.status, 2, cli.stderr);
        assert.deepEqual(JSON.parse(cli.stdout).suggestions, first.suggestions);
        await nativeApi.optimize!({ action: 'keep', decisionReason: 'necessary', readView: first.readView, decisionRevision: first.decisionRevision, suggestionId: drift.id });
        assert.equal((await nativeApi.optimize!({ action: 'list' })).suggestions.some(s => s.id === drift.id), false);
        await writeFile(source, text + '原件新增约束。\n');
        const changed = await nativeApi.optimize!({ action: 'list' }), newDrift = changed.suggestions.find(s => s.item.path === copy)!;
        assert.notEqual(newDrift.id, drift.id, 'unchanged copy must resurface when its declared source version changes');
        await nativeApi.optimize!({ action: 'recheck', readView: changed.readView, decisionRevision: changed.decisionRevision, suggestionId: newDrift.id });
        await rm(declaration);
        const missing = await nativeApi.optimize!({ action: 'recheck', group: 'history' });
        assert.equal(missing.suggestions.find(s => s.id === newDrift.id)!.status, 'recheckUnavailable', 'absence of a declaration cannot prove a relationship repaired');
        await writeFile(declaration, manifest);
        await writeFile(copy, await readFile(source));
        const fixed = await nativeApi.optimize!({ action: 'recheck', group: 'history' });
        assert.equal(fixed.suggestions.find(s => s.id === newDrift.id)!.status, 'verified', 'the original copy-drift problem passes independently of new size and repetition problems');
        assert.ok(fixed.pending > 0, 'independent new findings remain pending');
        const shorter = '一条完整合成指令。\n';
        await writeFile(source, shorter);
        await writeFile(copy, shorter);
        const verified = await nativeApi.optimize!({ action: 'recheck', group: 'history' });
        assert.equal(verified.suggestions.find(s => s.id === newDrift.id)!.status, 'verified');
        assert.equal(await readFile(declaration, 'utf8'), manifest, 'review operations never change declarations');
        const serialized = JSON.stringify(first);
        assert.equal(serialized.includes('完整中文指令'), false, 'DTOs contain positions and fingerprints, never raw instructions');
    }
    finally {
        service.kill('SIGTERM');
        await once(service, 'exit').catch(() => { });
        for (const [k, v] of Object.entries(previous)) {
            if (v === undefined)
                delete process.env[k];
            else
                process.env[k] = v;
        }
        await rm(dir, { recursive: true, force: true });
    }
});
test('relation rechecks fail closed across heading changes and identical manifests in other projects; overlapping scopes retain membership', { timeout: 45000 }, async () => {
    const dir = realpathSync.native(await mkdtemp(path.join(tmpdir(), 'wombat-rule-review-'))), root = path.join(dir, 'source'), project = path.join(dir, 'project'), other = path.join(dir, 'other'), nested = path.join(project, 'child');
    await mkdir(path.join(root, 'sessions'), { recursive: true });
    const manifest = JSON.stringify({ version: 1, chains: [{ id: 'joint', files: ['AGENTS.md', 'child/AGENTS.md'] }] });
    for (const p of [project, other]) {
        await mkdir(path.join(p, 'child'), { recursive: true });
        await mkdir(path.join(p, '.wombat'));
        await writeFile(path.join(p, '.wombat/analysis.json'), manifest);
    }
    const body = '# Condition A\n\nExact synthetic instruction.\n';
    for (const p of ['AGENTS.md', 'child/AGENTS.md'])
        await writeFile(path.join(project, p), body);
    await writeFile(path.join(other, 'AGENTS.md'), 'Independent synthetic instruction one.\n');
    await writeFile(path.join(other, 'child/AGENTS.md'), 'Independent synthetic instruction two.\n');
    const previous = { WOMBAT_DATA_HOME: process.env.WOMBAT_DATA_HOME, CODEX_HOME: process.env.CODEX_HOME, WOMBAT_AUTO_PRICES: process.env.WOMBAT_AUTO_PRICES };
    Object.assign(process.env, { WOMBAT_DATA_HOME: path.join(dir, 'data'), CODEX_HOME: root, WOMBAT_AUTO_PRICES: '0' });
    const binary = path.resolve('dist', process.platform === 'win32' ? 'wombat-core.exe' : 'wombat-core'), service = spawn(binary, ['--serve-usage'], { stdio: 'ignore', env: process.env });
    await once(service, 'spawn');
    const nativeEntry = projectTestClient(createNodeClient({ binaryPath: binary, automaticPrices: false }), [root], [project, other, nested]);

    const nativeApi = nativeEntry;
    try {
        const scoped = await nativeApi.config!({ action: 'list', scope: { allTime: true, project: nested } }), item = scoped.items.find(i => i.path === path.join(nested, 'AGENTS.md'))!;
        assert.ok(item);
        assert.deepEqual(item.authorizedProjects, [project, nested]);
        assert.equal(item.project, nested);
        const parent = await nativeApi.config!({ action: 'list', scope: { allTime: true, project } });
        assert.ok(parent.items.some(i => i.id === item.id));
        const first = await nativeApi.optimize!({ action: 'list', project });
        const target = first.suggestions.find(s => s.item.path === path.join(project, 'AGENTS.md'))!;
        assert.equal(target.findings[0].evidence?.relation?.project, project);
        await nativeApi.optimize!({ action: 'recheck', project, suggestionId: target.id, readView: first.readView, decisionRevision: first.decisionRevision });
        await writeFile(path.join(nested, 'AGENTS.md'), '# Condition B\n\nExact synthetic instruction.\n');
        const unknown = await nativeApi.optimize!({ action: 'recheck', group: 'history', project, suggestionId: target.id });
        assert.equal(unknown.suggestions[0].status, 'recheckUnavailable');
        assert.ok(unknown.issues.some(i => i.code === 'blockApplicabilityUnknown'));
        await writeFile(path.join(nested, 'AGENTS.md'), 'Independent replacement instruction.\n');
        await rm(path.join(project, '.wombat/analysis.json'));
        const absent = await nativeApi.optimize!({ action: 'recheck', group: 'history', project, suggestionId: target.id });
        assert.equal(absent.suggestions[0].status, 'recheckUnavailable');
        await writeFile(path.join(project, '.wombat/analysis.json'), manifest);
        const fixed = await nativeApi.optimize!({ action: 'recheck', group: 'history', project, suggestionId: target.id }), fixedCheck = fixed.suggestions[0].checks.find(c => c.rule === 'exactInstructionBlocks')!;
        assert.equal(fixed.suggestions[0].status, 'recheckUnavailable', 'exact-block findings without problem-location identity remain unresolved after a current miss');
        assert.equal(fixedCheck.outcome, 'miss');
        assert.equal(fixedCheck.comparison.status, 'unknown');
        assert.equal(fixedCheck.comparison.reason, 'problemIdentityUnavailable');
        assert.equal(fixed.history, 4);
        const cli = spawnSync(process.execPath, [path.resolve('dist/wombat.js'), 'optimize', 'history', '--root', root, ...[project, other, nested].flatMap(p => ['--project-root', p]), '--project', project, '--limit', '1', '--read-view', fixed.readView!, '--json'], { env: process.env, encoding: 'utf8', timeout: 10000 });
        assert.equal(cli.status, 2, cli.stderr + cli.stdout);
        const result = JSON.parse(cli.stdout);
        assert.equal(result.history, 4);
        assert.deepEqual(result.suggestions, [fixed.suggestions[0]]);
        assert.equal(result.page.total, 4);
    }
    finally {
        if (service.exitCode === null && service.signalCode === null) {
            const ended = once(service, 'exit');
            service.kill('SIGTERM');
            await ended;
        }
        for (const [k, v] of Object.entries(previous)) {
            if (v === undefined)
                delete process.env[k];
            else
                process.env[k] = v;
        }
        await rm(dir, { recursive: true, force: true });
    }
});
test('two sources share one physical configuration and one review while keeping independent evidence and ledger identities', { timeout: 45000 }, async () => {
    const dir = realpathSync.native(await mkdtemp(path.join(tmpdir(), 'wombat-shared-object-'))), roots = [path.join(dir, 'source-one'), path.join(dir, 'source-two')], project = path.join(dir, 'project'), skill = path.join(project, '.agents/skills/shared/SKILL.md');
    await mkdir(path.dirname(skill), { recursive: true });
    const body = '---\nname: shared\ndescription: Synthetic description\n---\n\nComplete synthetic instruction.\n\nComplete synthetic instruction.\n';
    await writeFile(skill, body);
    await writeFile(path.join(project, 'AGENTS.md'), 'Synthetic project instruction.\n');
    for (const [n, root] of roots.entries()) {
        await mkdir(path.join(root, 'sessions'), { recursive: true });
        await writeFile(path.join(root, 'AGENTS.md'), 'Synthetic source instruction.\n');
        const event = (type: string, payload: object) => ({ type, payload, timestamp: '2026-10-01T00:00:00Z' });
        const rows = [event('session_meta', { id: 'same-upstream-id', cwd: project }), event('turn_context', { turn_id: 'same-turn-id', model: 'gpt-5.4', cwd: project }), event('event_msg', { type: 'task_started', turn_id: 'same-turn-id' }), event('event_msg', { type: 'token_usage_record', response_id: 'same-response-id', turn_id: 'same-turn-id', usage: { input_tokens: 100 * (n + 1), cached_input_tokens: 20 * (n + 1), output_tokens: 10 * (n + 1), total_tokens: 110 * (n + 1) } }), ...['read-one', 'read-two'].flatMap(call_id => [event('response_item', { type: 'function_call', call_id, name: 'read_file', arguments: JSON.stringify({ path: skill }) }), event('response_item', { type: 'function_call_output', call_id, output: 'Synthetic content' })])];
        await writeFile(path.join(root, 'sessions/log.jsonl'), rows.map(r => JSON.stringify(r)).join('\n') + '\n');
    }
    const saved = { WOMBAT_DATA_HOME: process.env.WOMBAT_DATA_HOME, CODEX_HOME: process.env.CODEX_HOME, WOMBAT_AUTO_PRICES: process.env.WOMBAT_AUTO_PRICES };
    Object.assign(process.env, { WOMBAT_DATA_HOME: path.join(dir, 'data'), CODEX_HOME: roots[0], WOMBAT_AUTO_PRICES: '0' });
    const binary = path.resolve('dist', process.platform === 'win32' ? 'wombat-core.exe' : 'wombat-core'), service = spawn(binary, ['--serve-usage'], { stdio: 'ignore' });
    await once(service, 'spawn');
    const nativeEntry = projectTestClient(createNodeClient({ binaryPath: binary, automaticPrices: false }), roots, [project]);
    const nativeApi = nativeEntry;
    try {
        const usage = await nativeApi.live!({ query: { action: 'usage', scope: { allTime: true } }, mode: 'fresh' });
        assert.equal(usage.result.summary.tokens.total, 330);
        const config = await nativeApi.config!({ action: 'list', scope: { allTime: true } }), shared = config.items.filter(i => i.path === skill);
        assert.equal(shared.length, 1);
        const item = shared[0];
        assert.equal(item.sourceContexts?.length, 2);
        assert.equal(item.counts.fileReads, 4);
        assert.equal(item.relatedTurns, 2);
        assert.equal(item.usage?.tokens.total, 330);
        assert.deepEqual(item.sourceContexts!.map(c => c.counts.fileReads), [2, 2]);
        const evidence = await nativeApi.config!({ action: 'evidence', readView: config.readView, itemId: item.id, scope: { allTime: true } });
        assert.equal(evidence.evidence.length, 4);
        assert.equal(new Set(evidence.evidence.map(e => e.sourceInstanceId)).size, 2);
        assert.equal(new Set(evidence.evidence.map(e => e.threadId)).size, 2);
        const first = await nativeApi.optimize!({ action: 'list' });
        assert.equal(first.pending, 1);
        assert.equal(first.suggestions[0].item.id, item.id);
        assert.equal(first.suggestions[0].findings.length, 1);
        assert.equal(first.suggestions[0].findings[0].evidence?.positions.length, 2);
        for (const c of item.sourceContexts!) {
            const filtered = await nativeApi.config!({ action: 'list', readView: config.readView, scope: { allTime: true, sourceInstanceId: c.sourceInstanceId } });
            assert.equal(filtered.items.find(i => i.id === item.id)!.counts.fileReads, 2);
            assert.equal((await nativeApi.optimize!({ action: 'list', readView: first.readView, sourceInstanceId: c.sourceInstanceId })).suggestions[0].id, first.suggestions[0].id);
        }
        await nativeApi.optimize!({ action: 'keep', decisionReason: 'necessary', suggestionId: first.suggestions[0].id, readView: first.readView, decisionRevision: first.decisionRevision });
        for (const c of item.sourceContexts!) {
            const scoped = await nativeApi.optimize!({ action: 'list', sourceInstanceId: c.sourceInstanceId }), decision = scoped.suggestions[0].decision!;
            assert.equal(scoped.pending, 1, 'a decision recorded across all source contexts does not silently transfer to a narrower source scope');
            assert.equal(decision.kind, 'keep');
            assert.equal(decision.binding.scope.sourceInstanceId, null);
            assert.deepEqual([...decision.binding.scope.sourceInstances].sort(), item.sourceContexts!.map(source => source.sourceInstanceId).sort());
        }
        const allSources = await nativeApi.optimize!({ action: 'list' }), broadHistory = await nativeApi.optimize!({ action: 'list', group: 'history' });
        assert.equal(allSources.pending, 0);
        assert.equal(allSources.history, 1);
        assert.equal(broadHistory.suggestions[0].decision?.kind, 'keep', 'the broad decision remains effective in its original scope');
        const restored = await nativeApi.optimize!({ action: 'redisplay', suggestionId: first.suggestions[0].id });
        assert.equal(restored.pending, 1);
        assert.equal(restored.history, 2);
        await nativeApi.optimize!({ action: 'recheck', suggestionId: first.suggestions[0].id });
        await writeFile(skill, '---\nname: shared\ndescription: Synthetic description\n---\n\nOne complete synthetic instruction.\n');
        const checked = await nativeApi.optimize!({ action: 'recheck', group: 'history' }), checkedSuggestion = checked.suggestions[0], checkedBlock = checkedSuggestion.checks.find(c => c.rule === 'exactInstructionBlocks')!;
        assert.equal(checkedSuggestion.status, 'recheckUnavailable', 'a current exact-block miss cannot verify a baseline that has no stable problem identity');
        assert.equal(checked.pending, 1);
        assert.equal(checkedBlock.outcome, 'miss');
        assert.equal(checkedBlock.comparison.status, 'unknown');
        assert.equal(checkedBlock.comparison.reason, 'problemIdentityUnavailable');
        assert.equal(checkedSuggestion.reviewBaseline?.item.sourceContexts?.length, 2);
        const cli = spawnSync(process.execPath, [path.resolve('dist/wombat.js'), 'optimize', 'inventory', ...roots.flatMap(p => ['--root', p]), '--project-root', project, '--all-time', '--read-view', checked.readView!, '--json'], { env: process.env, encoding: 'utf8', timeout: 10000 });
        assert.equal(cli.status, 2, cli.stderr + cli.stdout);
        assert.equal(JSON.parse(cli.stdout).items.filter((i: {
            path: string;
        }) => i.path === skill).length, 1);
    }
    finally {
        if (service.exitCode === null && service.signalCode === null) {
            const ended = once(service, 'exit');
            service.kill('SIGTERM');
            await ended;
        }
        for (const [k, v] of Object.entries(saved)) {
            if (v === undefined)
                delete process.env[k];
            else
                process.env[k] = v;
        }
        await rm(dir, { recursive: true, force: true });
    }
});
