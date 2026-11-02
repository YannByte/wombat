import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, writeFile, chmod, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { CoreError } from '@wombat/client';
import { createNodeClient } from '@wombat/client/node';

async function withCore(body: string, run: (binaryPath: string) => Promise<void>): Promise<void> {
  const directory = await mkdtemp(path.join(tmpdir(), 'wombat-client-'));
  const binaryPath = path.join(directory, 'core.cjs');
  try {
    await writeFile(binaryPath, `#!${process.execPath}\n${body}`);
    await chmod(binaryPath, 0o700);
    await run(binaryPath);
  } finally { await rm(directory, { recursive: true, force: true }); }
}

test('node client sends only the fixed product protocol and preserves structured failures', async () => {
  await withCore(`let text='';process.stdin.on('data',chunk=>text+=chunk);process.stdin.on('end',()=>{const input=JSON.parse(text);console.log(JSON.stringify({ok:false,code:'NO_SNAPSHOT',error:'请先更新记录',details:input}));});`, async binaryPath => {
    await assert.rejects(createNodeClient({ binaryPath }).query({ action: 'usage' }), (error: unknown) => {
      assert.ok(error instanceof CoreError);
      assert.equal(error.code, 'NO_SNAPSHOT');
      assert.deepEqual(error.details, { op: 'local_query', args: { query: {action:'usage'}, mode:'auto' } });
      return true;
    });
  });
});

test('node client rejects malformed envelopes and output overflow', async () => {
  await withCore(`process.stdin.resume();process.stdin.on('end',()=>console.log('invalid'));`, async binaryPath => {
    await assert.rejects(createNodeClient({ binaryPath }).query({ action: 'usage' }), (error: unknown) => error instanceof CoreError && error.code === 'PROTOCOL_ERROR');
  });
  await withCore(`process.stdin.resume();process.stdin.on('end',()=>process.stdout.write('x'.repeat(5000)));`, async binaryPath => {
    await assert.rejects(createNodeClient({ binaryPath, maxResponseBytes: 1000 }).query({ action: 'usage' }), (error: unknown) => error instanceof CoreError && error.code === 'OUTPUT_LIMIT');
  });
});

test('node client times out and removes process signal listeners', async () => {
  const before = ['SIGINT', 'SIGTERM', 'exit'].map(signal => process.listenerCount(signal));
  await withCore(`process.stdin.resume();setInterval(()=>{},1000);`, async binaryPath => {
    await assert.rejects(createNodeClient({ binaryPath, timeoutMs: 100 }).query({ action: 'usage' }), (error: unknown) => error instanceof CoreError && error.code === 'TIMEOUT');
  });
  assert.deepEqual(['SIGINT', 'SIGTERM', 'exit'].map(signal => process.listenerCount(signal)), before);
});
