import { test } from 'node:test';
import assert from 'node:assert/strict';
import { mkdtemp, writeFile, rm } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { connectCodex } from '../src/node/codex/rpc.js';
import { nativeVersion, runCodex } from '../src/node/codex/process.js';
import { NativeDiagnostics, isLocalAccessDenied } from '../src/node/local-access.js';
import packageMetadata from '../package.json' with { type: 'json' };

test('native version preserves prerelease and build identity without accepting damaged output', async () => {
  const dir = await mkdtemp(path.join(tmpdir(), 'wombat-native-version-'));
  const binary = path.join(dir, 'codex.cjs');
  try {
    for (const version of ['0.160.1', '0.162.0-alpha.2', '0.162.0-beta.1+build.7']) {
      await writeFile(binary, `console.log(${JSON.stringify('codex-cli ' + version)});`);
      assert.equal(await nativeVersion({}, { codexBinaryPath: binary }), version);
    }
    for (const output of [
      'other-cli 0.162.0',
      'codex-cli 0.162.0-alpha..2',
      'codex-cli 0.162.0\nprivate extra output',
      'codex-cli ' + '1'.repeat(257),
    ]) {
      await writeFile(binary, `console.log(${JSON.stringify(output)});`);
      await assert.rejects(nativeVersion({}, { codexBinaryPath: binary }), {
        code: 'NATIVE_PROTOCOL_ERROR',
      });
    }
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
});

async function fixture(body: string, run: (binary: string) => Promise<void>) {
  const dir = await mkdtemp(path.join(tmpdir(), 'wombat-native-protocol-'));
  const binary = path.join(dir, 'codex.cjs');
  try {
    await writeFile(
      binary,
      String.raw`let text=''; process.stdin.on('data',chunk=>{text+=chunk;let end;while((end=text.indexOf('\n'))>=0){const message=JSON.parse(text.slice(0,end));text=text.slice(end+1);if(message.id==null)continue;${body}}});`,
    );
    await run(binary);
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
}

test('native initialization identifies the current packaged product version', async () => {
  await fixture(
    `if(message.method==='initialize'){if(message.params.clientInfo.version!==${JSON.stringify(packageMetadata.version)}){console.log(JSON.stringify({id:message.id,error:{}}));continue;}console.log(JSON.stringify({id:message.id,result:{}}));}`,
    async (binary) => {
      const rpc = await connectCodex(false, {}, { codexBinaryPath: binary, codexTimeoutMs: 1000 });
      rpc.close();
    },
  );
});

test('native responses preserve split UTF-8 and sanitize rejected requests', async () => {
  await fixture(
    String.raw`if(message.method==='initialize'){console.log(JSON.stringify({id:message.id,result:{}}));continue;}if(message.method==='account/read'){const bytes=Buffer.from(JSON.stringify({id:message.id,result:{label:'合成🙂'}})+'\n');const split=bytes.indexOf(Buffer.from('成'))+1;process.stdout.write(bytes.subarray(0,split));setImmediate(()=>process.stdout.write(bytes.subarray(split)));}else console.log(JSON.stringify({id:message.id,error:{message:'Synthetic private native output'}}));`,
    async (binary) => {
      const rpc = await connectCodex(false, {}, { codexBinaryPath: binary, codexTimeoutMs: 1000 });
      try {
        assert.deepEqual(await rpc.request('account/read', {}), { label: '合成🙂' });
        await assert.rejects(rpc.request('account/usage/read', {}), (error) => {
          assert.equal((error as { code: string }).code, 'CODEX_REQUEST_REJECTED');
          assert.ok(!String(error).includes('Synthetic private'));
          return true;
        });
      } finally {
        rpc.close();
      }
    },
  );
});

test('native handshake cancellation and timeout stop waiting with explicit outcomes', async () => {
  await fixture('', async (binary) => {
    await assert.rejects(connectCodex(false, {}, { codexBinaryPath: binary, codexTimeoutMs: 50 }), {
      code: 'CODEX_TIMEOUT',
    });
    const controller = new AbortController();
    const pending = connectCodex(
      false,
      { signal: controller.signal },
      { codexBinaryPath: binary, codexTimeoutMs: 1000 },
    );
    setTimeout(() => controller.abort(), 30);
    await assert.rejects(pending, { code: 'CANCELLED' });
  });
});

test('malformed native responses and disconnects never look like acceptance', async () => {
  for (const [body, code] of [
    ["console.log('invalid json');", 'NATIVE_PROTOCOL_ERROR'],
    ['process.exit(0);', 'CODEX_DISCONNECTED'],
  ] as const) {
    await fixture(body, async (binary) => {
      await assert.rejects(
        connectCodex(false, {}, { codexBinaryPath: binary, codexTimeoutMs: 1000 }),
        { code },
      );
    });
  }
});

test('local access denials have a recoverable code without leaking native diagnostics', async () => {
  const dir = await mkdtemp(path.join(tmpdir(), 'wombat-native-denied-'));
  const binary = path.join(dir, 'codex.cjs');
  try {
    await writeFile(binary, String.raw`process.stderr.write('Error: listen EP');setTimeout(()=>{process.stderr.write('ERM: operation not permitted PRIVATE_SYNTHETIC_PATH\n');process.exit(1);},20);`);
    for (const run of [
      () => connectCodex(false, {}, { codexBinaryPath: binary, codexTimeoutMs: 1000 }),
      () => runCodex(['--version'], {}, { codexBinaryPath: binary, codexTimeoutMs: 1000 }),
    ]) await assert.rejects(run(), error => {
      assert.equal((error as { code: string }).code, 'LOCAL_ACCESS_DENIED');
      assert.ok(!String(error).includes('PRIVATE_SYNTHETIC_PATH'));
      return true;
    });
    await writeFile(binary, `process.stderr.write('Synthetic private output mentions EPERM and permission denied');process.exit(1);`);
    await assert.rejects(connectCodex(false, {}, { codexBinaryPath: binary, codexTimeoutMs: 1000 }), {code:'CODEX_DISCONNECTED'});
  } finally { await rm(dir, { recursive: true, force: true }); }
});

test('diagnostics preserve bounded lines and distinguish recovered warnings from host denials', () => {
  assert.equal(isLocalAccessDenied({code:'EACCES'}), true);
  assert.equal(isLocalAccessDenied({code:'EPERM'}), true);
  assert.equal(isLocalAccessDenied({code:'ECONNREFUSED'}), false);
  const diagnostics = new NativeDiagnostics();
  diagnostics.write(Buffer.from('x'.repeat(100_000)+'Error: listen EPERM: operation not permitted\n'));
  assert.equal(diagnostics.error('CODEX_DISCONNECTED','Disconnected').code,'CODEX_DISCONNECTED');
  diagnostics.write(Buffer.from('Error: failed to bind Unix socket: Permission denied (os error 13)\n'));
  assert.equal(diagnostics.error('CODEX_DISCONNECTED','Disconnected').code,'LOCAL_ACCESS_DENIED');
  diagnostics.clear();
  assert.equal(diagnostics.error('CODEX_DISCONNECTED','Disconnected').code,'CODEX_DISCONNECTED');
});
