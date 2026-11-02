import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, writeFile, chmod, rm, readFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { CoreError, type TimingRequest } from '@wombat/client';
import { createNodeClient } from '@wombat/client/node';

test('Node timing uses one foreground child for capabilities, summaries and evidence', async () => {
  const dir = await mkdtemp(path.join(tmpdir(), 'wombat-timing-route-'));
  const binaryPath = path.join(dir, 'core.cjs');
  const calls = path.join(dir, 'calls');
  const body = `let text='';process.stdin.on('data',c=>text+=c);process.stdin.on('end',()=>{const input=JSON.parse(text);require('node:fs').appendFileSync(${JSON.stringify(calls)},JSON.stringify(input)+'\\n');console.log(JSON.stringify({ok:false,code:'SYNTHETIC_ROUTE',error:'synthetic',details:input}));});`;
  await writeFile(binaryPath, body); await chmod(binaryPath, 0o700);
  try {
    const reader = createNodeClient({ binaryPath, codexBinaryPath: path.join(dir, 'must-not-run') });
    const requests:TimingRequest[]=[{action:'capabilities'},{action:'summary',threadId:'thread',turnId:'turn'},{action:'evidence',threadId:'thread',turnId:'turn'}];
    for (const request of requests) {
      await assert.rejects(reader.timing!(request), (error:unknown)=>{
        assert.ok(error instanceof CoreError);assert.equal(error.code,'SYNTHETIC_ROUTE');
        assert.deepEqual(error.details,{op:'local_query',args:{timing:request}});return true;
      });
    }
    const operations = (await readFile(calls, 'utf8')).trim().split('\n').map(line => JSON.parse(line).op);
    assert.deepEqual(operations, ['local_query','local_query','local_query']);
    const cancelled = new AbortController(); cancelled.abort();
    await assert.rejects(reader.timing!({ action: 'capabilities' }, { signal: cancelled.signal }), { code: 'CANCELLED' });
    assert.equal((await readFile(calls, 'utf8')).trim().split('\n').length, 3);
  } finally { await rm(dir, { recursive: true, force: true }); }
});
