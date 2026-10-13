import { test } from 'node:test';
import assert from 'node:assert/strict';
import { writeFile } from 'node:fs/promises';
import { createNodeClient } from '@wombat/client/node';
import { createHttpClient } from '@wombat/client/http';
import { startWebHost } from '@wombat/web';
import path from 'node:path';
import { nativeCodexFixture } from '../helpers/native-codex.js';
import { withLocalProduct } from '../helpers/local-product.js';

test('local native access denial remains explicit in CLI/Web and recovers without a usage scan', { timeout: 30000 }, async () => {
  await withLocalProduct(async f => {
    const native = await nativeCodexFixture(f.dir);
    const previous = process.env.WOMBAT_CODEX_BIN;
    process.env.WOMBAT_CODEX_BIN = native.binary;
    const client = createNodeClient({ binaryPath: path.resolve('dist', process.platform === 'win32' ? 'wombat-core.exe' : 'wombat-core'), codexBinaryPath: native.binary, automaticPrices: false });
    const host = await startWebHost({ client, assets: path.resolve('dist/web'), roots: [f.root], projectRoots: [f.project], automaticPrices: false });
    try {
      const token = new URLSearchParams(new URL(host.url).hash.slice(1)).get('token')!;
      const browser = createHttpClient({ origin: host.origin, token, fetch: (input, init) => fetch(input, { ...init, headers: { ...init?.headers, Origin: host.origin } }) });
      await writeFile(native.mode, JSON.stringify({ kind: 'available', permissionDenied: true }));
      const cli = JSON.parse(f.cli(['account', '--json'], '', 2));
      const web = await browser.account!({action:'read'});
      for (const result of [cli, web]) {
        for (const section of [result.account, result.allowance, result.activity]) {
          assert.equal(section.status, 'unavailable');
          assert.equal(section.errorCode, 'LOCAL_ACCESS_DENIED');
        }
        assert.deepEqual(result.windows, []);
        assert.equal(result.identity, null);
        assert.ok(!JSON.stringify(result).includes('SYNTHETIC_PRIVATE_NATIVE_PATH'));
      }
      const setup = JSON.parse(f.cli(['setup', '--project', f.project, '--json'], '', 2));
      assert.equal(setup.discovery.status, 'unavailable');
      assert.equal(setup.discovery.errorCode, 'LOCAL_ACCESS_DENIED');
      await writeFile(native.mode, JSON.stringify({ kind: 'available' }));
      const recovered = await browser.account!({action:'refresh'});
      assert.equal(recovered.allowance.status, 'available');
      assert.equal(recovered.windows[0].usedPercent, 20);
      assert.equal(recovered.windows[0].durationMinutes, 17);
      assert.equal(recovered.allowance.errorCode, null);
      const recoveredCli = JSON.parse(f.cli(['account', '--json'], '', 2));
      assert.equal(recoveredCli.windows[0].usedPercent, 20);
      assert.equal(recoveredCli.allowance.status, 'available');
    } finally {
      await host.close();
      if (previous === undefined) delete process.env.WOMBAT_CODEX_BIN; else process.env.WOMBAT_CODEX_BIN = previous;
      await native.waitForExit();
    }
  });
});
