import { test } from 'node:test';
import assert from 'node:assert/strict';
import { once } from 'node:events';
import { createServer, type Socket } from 'node:net';
import { chmod, mkdtemp, readFile, rm, writeFile } from 'node:fs/promises';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { randomUUID } from 'node:crypto';
import { queryLive } from '../src/node/live.js';

test('shutdown connection races retry read-only status but never repeat a control', async () => {
  const dir=await mkdtemp(path.join(tmpdir(),'wombat-wire-shutdown-'));
  const endpoint=process.platform==='win32'?`\\\\.\\pipe\\wombat-test-${randomUUID()}`:path.join(dir,'socket');
  const binaryPath=path.join(dir,'endpoint.cjs');
  const response=JSON.stringify({ok:true,value:{protocolVersion:2,socket:endpoint}});
  await writeFile(binaryPath,`process.stdin.resume();process.stdin.on('end',()=>console.log(${JSON.stringify(response)}));`);
  await chmod(binaryPath,0o700);
  let reads=0,controls=0;
  const server=createServer(socket=>socket.once('data',bytes=>{
    const request=JSON.parse(bytes.toString());
    if(request.service.action==='status'&&++reads===2)socket.end('{"ok":true,"value":{"marker":"still-running"}}\n');
    else {if(request.service.action!=='status')controls++;socket.end();}
  }));
  try {
    server.listen(endpoint);await once(server,'listening');
    assert.deepEqual(await queryLive({service:{action:'status'}},{},{binaryPath},false),{marker:'still-running'});
    assert.equal(reads,2);
    await assert.rejects(queryLive({service:{action:'pause'}},{},{binaryPath}),{code:'SERVICE_CLOSED'});
    assert.equal(controls,1);
  } finally {
    await new Promise<void>(resolve=>server.close(()=>resolve()));
    await rm(dir,{recursive:true,force:true});
  }
});

test('explicit refresh can publish beyond the read deadline while caller timeout and cancellation still apply', async () => {
  const dir = await mkdtemp(path.join(tmpdir(), 'wombat-wire-refresh-'));
  const endpoint = process.platform === 'win32' ? `\\\\.\\pipe\\wombat-test-${randomUUID()}` : path.join(dir, 'socket');
  const connections = new Set<Socket>();
  const timers = new Set<ReturnType<typeof setTimeout>>();
  const controller = new AbortController();
  let received = 0, cancelledWaiters = 0;
  const server = createServer(socket => {
    connections.add(socket); socket.once('close', () => connections.delete(socket));
    socket.on('data', bytes => { if(bytes.includes(0)) cancelledWaiters++; });
    socket.once('data', bytes => {
      const request=JSON.parse(bytes.toString());
      assert.ok(['refresh','usage'].includes(request.query.action));
      if (++received === 1) { controller.abort(); return; }
      const timer = setTimeout(() => { timers.delete(timer); socket.end('{"ok":true,"value":{"marker":"published"}}\n'); }, 12_300);
      timers.add(timer);
    });
  });
  try {
    server.listen(endpoint); await once(server, 'listening');
    const binaryPath = path.join(dir, 'endpoint.cjs');
    const response = JSON.stringify({ ok: true, value: { protocolVersion: 2, socket: endpoint } });
    await writeFile(binaryPath, `process.stdin.resume();process.stdin.on('end',()=>console.log(${JSON.stringify(response)}));`);
    await chmod(binaryPath, 0o700);
    const request = { query: { action: 'refresh' as const } };
    await assert.rejects(queryLive(request, { signal: controller.signal }, { binaryPath }), { code: 'CANCELLED' });
    await assert.rejects(queryLive(request, {}, { binaryPath, timeoutMs: 1000 }), { code: 'TIMEOUT' });
    assert.deepEqual(await queryLive({query:{action:'usage'},mode:'fresh',waitMs:15000}, {}, { binaryPath }), { marker: 'published' });
    assert.equal(cancelledWaiters,2,'Cancellation and timeout release only their own waiters');
  } finally {
    for (const timer of timers) clearTimeout(timer);
    for (const connection of connections) connection.destroy();
    await new Promise<void>(resolve => server.close(() => resolve()));
    await rm(dir, { recursive: true, force: true });
  }
});

test('live client accepts a split response without waiting for server EOF', async () => {
  const dir = await mkdtemp(path.join(tmpdir(), 'wombat-wire-'));
  const endpoint = process.platform === 'win32' ? `\\\\.\\pipe\\wombat-test-${randomUUID()}` : path.join(dir, 'socket');
  const connections = new Set<Socket>();
  const server = createServer(socket => {
    connections.add(socket);
    socket.once('data', () => {
      socket.write('{"ok":true,"value":{"marker":"complete"}}');
      setImmediate(() => socket.write('\n')); // Deliberately keep the server end open.
    });
    socket.once('close', () => connections.delete(socket));
  });
  try {
    server.listen(endpoint); await once(server, 'listening');
    const binaryPath = path.join(dir, 'endpoint.cjs');
    const response = JSON.stringify({ ok: true, value: { protocolVersion: 2, socket: endpoint } });
    await writeFile(binaryPath, `process.stdin.resume();process.stdin.on('end',()=>console.log(${JSON.stringify(response)}));`);
    await chmod(binaryPath, 0o700);
    assert.deepEqual(await queryLive({ query: { action: 'usage' } }, {}, { binaryPath, timeoutMs: 1000 }), { marker: 'complete' });
  } finally {
    for (const connection of connections) connection.destroy();
    await new Promise<void>(resolve => server.close(() => resolve()));
    await rm(dir, { recursive: true, force: true });
  }
});

test('live client retries when the first service process loses the startup race', async () => {
  const dir = await mkdtemp(path.join(tmpdir(), 'wombat-wire-retry-'));
  const endpoint = process.platform === 'win32' ? `\\\\.\\pipe\\wombat-test-${randomUUID()}` : path.join(dir, 'socket');
  const counter = path.join(dir, 'starts');
  const binaryPath = path.join(dir, 'endpoint.cjs');
  const response = JSON.stringify({ ok: true, value: { protocolVersion: 2, socket: endpoint } });
  const success = JSON.stringify({ ok: true, value: { marker: 'retried' } }) + '\n';
  await writeFile(binaryPath, `
const fs = require('node:fs');
const net = require('node:net');
if (process.argv.includes('--serve-usage')) {
  const starts = fs.existsSync(${JSON.stringify(counter)}) ? Number(fs.readFileSync(${JSON.stringify(counter)}, 'utf8')) + 1 : 1;
  fs.writeFileSync(${JSON.stringify(counter)}, String(starts));
  if (starts === 1) process.exit(0);
  const server = net.createServer(socket => socket.once('data', () => socket.end(${JSON.stringify(success)}, () => server.close())));
  server.listen(${JSON.stringify(endpoint)});
} else {
  process.stdin.resume();
  process.stdin.on('end', () => console.log(${JSON.stringify(response)}));
}
`);
  await chmod(binaryPath, 0o700);
  try {
    assert.deepEqual(await queryLive({ query: { action: 'usage' } }, {}, { binaryPath, timeoutMs: 1000 }), { marker: 'retried' });
    assert.equal(await readFile(counter, 'utf8'), '2');
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
});

test('live client rejects the previous protocol before connecting or starting a service', async () => {
  const dir = await mkdtemp(path.join(tmpdir(), 'wombat-wire-version-'));
  const binaryPath = path.join(dir, 'endpoint.cjs');
  const starts = path.join(dir, 'starts');
  const response = JSON.stringify({ ok: true, value: { protocolVersion: 1, socket: path.join(dir, 'absent') } });
  await writeFile(binaryPath, `if(process.argv.includes('--serve-usage')){require('node:fs').writeFileSync(${JSON.stringify(starts)},'started');}else{process.stdin.resume();process.stdin.on('end',()=>console.log(${JSON.stringify(response)}));}`);
  await chmod(binaryPath, 0o700);
  try {
    await assert.rejects(queryLive({ timing: { action: 'capabilities' } }, {}, { binaryPath }), { code: 'PROTOCOL_ERROR' });
    await assert.rejects(readFile(starts), { code: 'ENOENT' });
  } finally { await rm(dir, { recursive: true, force: true }); }
});

test('cancelling one timing connection preserves another reader and sends no hook inputs', async () => {
  const dir = await mkdtemp(path.join(tmpdir(), 'wombat-wire-cancel-'));
  const endpoint = process.platform === 'win32' ? `\\\\.\\pipe\\wombat-test-${randomUUID()}` : path.join(dir, 'socket');
  const binaryPath = path.join(dir, 'endpoint.cjs');
  const response = JSON.stringify({ ok: true, value: { protocolVersion: 2, socket: endpoint } });
  await writeFile(binaryPath, `process.stdin.resume();process.stdin.on('end',()=>console.log(${JSON.stringify(response)}));`);
  await chmod(binaryPath, 0o700);
  const connections = new Set<Socket>();
  const received: { timing: { threadId: string } }[] = [];
  let release!: () => void;
  const bothReceived = new Promise<void>(resolve => { release = resolve; });
  const server = createServer(socket => {
    connections.add(socket);
    socket.once('close', () => connections.delete(socket));
    socket.once('data', bytes => {
      const request = JSON.parse(bytes.toString()); received.push(request);
      if (received.length === 2) release();
      if (request.timing.threadId === 'other') setTimeout(() => socket.end('{"ok":true,"value":{"marker":"other"}}\n'), 100);
    });
  });
  try {
    server.listen(endpoint); await once(server, 'listening');
    const cancelled = new AbortController();
    const firstRequest = { timing: { action: 'summary' as const, threadId: 'cancelled', turnId: 'turn' } };
    const secondRequest = { timing: { action: 'summary' as const, threadId: 'other', turnId: 'turn' } };
    const first = queryLive(firstRequest, { signal: cancelled.signal }, { binaryPath, timeoutMs: 2000, codexBinaryPath: path.join(dir, 'must-not-run') });
    const rejected = assert.rejects(first, { code: 'CANCELLED' });
    const second = queryLive(secondRequest, {}, { binaryPath, timeoutMs: 2000, codexBinaryPath: path.join(dir, 'must-not-run') });
    await bothReceived;
    cancelled.abort(); await rejected;
    assert.deepEqual(await second, { marker: 'other' });
    assert.deepEqual(received.sort((a, b) => a.timing.threadId.localeCompare(b.timing.threadId)), [firstRequest, secondRequest]);
    assert.equal(server.listening, true);
  } finally {
    for (const connection of connections) connection.destroy();
    await new Promise<void>(resolve => server.close(() => resolve()));
    await rm(dir, { recursive: true, force: true });
  }
});
