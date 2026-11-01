import {spawnProductCli} from '../../scripts/cli-invocation.js';
import { projectTestClient } from '../helpers/project-client.js';
import type { UsageClient } from '@wombat/client';
import { test } from 'node:test';
import assert from 'node:assert/strict';
import { realpathSync } from 'node:fs';
import { mkdtemp, mkdir, writeFile, access, rm } from 'node:fs/promises';
import { createHash } from 'node:crypto';
import { spawn, spawnSync } from 'node:child_process';
import { once } from 'node:events';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { createNodeClient } from '@wombat/client/node';
test('Hook declaration measurements agree in CLI and HTTP without measuring or executing referenced scripts', { timeout: 30000 }, async () => {
    const dir = realpathSync.native(await mkdtemp(path.join(tmpdir(), 'wombat-hook-inventory-'))), root = path.join(dir, 'source'), project = path.join(dir, 'project'), marker = path.join(dir, 'must-not-exist');
    const old = { WOMBAT_DATA_HOME: process.env.WOMBAT_DATA_HOME, CODEX_HOME: process.env.CODEX_HOME, WOMBAT_AUTO_PRICES: process.env.WOMBAT_AUTO_PRICES };
    await mkdir(path.join(root, 'sessions'), { recursive: true });
    await mkdir(project);
    const declaration = JSON.stringify({ type: 'command', command: `node '${path.join(project, '工具.js')}'`, env: { SECRET: 'SYNTHETIC_NOT_EXPOSED' } });
    await writeFile(path.join(root, 'hooks.json'), `{"hooks":{"SessionStart":[{"hooks":[${declaration}]}]}}`);
    await writeFile(path.join(project, '工具.js'), `require('node:fs').writeFileSync(${JSON.stringify(marker)},'executed');\n/*${'x'.repeat(2000000)}*/`);
    Object.assign(process.env, { WOMBAT_DATA_HOME: path.join(dir, 'data'), CODEX_HOME: root, WOMBAT_AUTO_PRICES: '0' });
    const binary = path.resolve('dist', process.platform === 'win32' ? 'wombat-core.exe' : 'wombat-core');
    const service = spawn(binary, ['--serve-usage'], { stdio: 'ignore', env: process.env });
    await once(service, 'spawn');
    const nativeEntry = projectTestClient(createNodeClient({ binaryPath: binary, automaticPrices: false }), [root], [project]);
    try {

        const nativeTransport = nativeEntry;
        const result = await nativeTransport.config!({ action: 'list', kind: 'hook' });
        assert.equal(result.items.length, 1);
        const item = result.items[0];
        assert.equal(item.bytes, Buffer.byteLength(declaration));
        assert.equal(item.characters, [...declaration].length);
        assert.equal(item.estimate?.contentHash, createHash('sha256').update(declaration).digest('hex'));
        assert.equal(item.estimate?.payload, 'hookDeclaration');
        assert.ok((item.contentTokens ?? 0) > 0);
        assert.equal(item.configuredState, 'declared');
        assert.equal(item.usageCount, null);
        assert.ok(!JSON.stringify(result).includes('SYNTHETIC_NOT_EXPOSED'));
        assert.ok(!JSON.stringify(result).includes('writeFileSync'));
        const cli = spawnProductCli(path.resolve('dist/wombat.js'), {method:'config',params:{action:'list',roots:[root],projectRoots:[project],kind:'hook',readView:result.readView}}, { encoding: 'utf8', env: process.env, timeout: 15000 });
        assert.equal(cli.status, 2, cli.stderr + cli.stdout);
        const cliItem = JSON.parse(cli.stdout).items[0];
        assert.deepEqual([cliItem.bytes, cliItem.characters, cliItem.contentTokens, cliItem.estimate], [item.bytes, item.characters, item.contentTokens, item.estimate]);
        await writeFile(path.join(project, '工具.js'), 'changed script body');
        const fresh = await nativeTransport.config!({ action: 'list', kind: 'hook' });
        assert.deepEqual(fresh.items[0].estimate, item.estimate);
        assert.equal(fresh.items[0].contentHash, item.contentHash);
        await assert.rejects(access(marker));
    }
    finally {
        service.kill('SIGTERM');
        await once(service, 'exit').catch(() => { });
        for (const [k, v] of Object.entries(old)) {
            if (v === undefined)
                delete process.env[k];
            else
                process.env[k] = v;
        }
        await rm(dir, { recursive: true, force: true });
    }
});
