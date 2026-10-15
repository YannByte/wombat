import test from 'node:test';
import assert from 'node:assert/strict';
import { Readable, PassThrough } from 'node:stream';
import { Ajv } from 'ajv';
import { CoreError, validateAgentRequest, agentResultSchemas } from '@wombat/client';
import { runAgentCli, agentErrorOutput } from '../src/agent-cli.js';
import { describeAgentApi } from '../src/agent-description.js';
import { local, capabilityResult } from './timing-fixtures.js';

const requests = [
  {
    method: 'usage',
    params: {
      query: {
        action: 'threads',
        scope: { project: '/synthetic', threadId: 'complete-id', allTime: true },
        limit: 3,
        compact: true,
      },
      mode: 'auto',
    },
  },
  {
    method: 'snapshot',
    params: {
      action: 'investigate',
      snapshotId: 'fixed',
      scope: { threadId: 'complete-id', turnId: 'complete-turn' },
      compact: true,
    },
  },
  { method: 'config', params: { action: 'capabilities' } },
  { method: 'optimize', params: { action: 'list', limit: 3 } },
  { method: 'timing', params: { action: 'capabilities' } },
  { method: 'setup', params: { project: '/synthetic' } },
  { method: 'account', params: { action: 'read' } },
  { method: 'directories', params: { action: 'list' } },
  { method: 'preferences', params: { action: 'get' } },
  { method: 'prices', params: { action: 'status' } },
  { method: 'monitor', params: { action: 'list' } },
  { method: 'collection', params: { action: 'status' } },
  { method: 'handoff', params: { action: 'preview' } },
  { method: 'service', params: { action: 'status' } },
  {
    method: 'conversation',
    params: {
      action: 'locate',
      snapshotId: 'fixed',
      selection: { projects: ['/synthetic'], recordKinds: ['response_snapshot'] },
      limit: 3,
    },
  },
];
function capture() {
  const out: string[] = [];
  return {
    out,
    stdout: (s: string) => {
      out.push(s);
    },
  };
}
test('local discovery exposes only reachable method schemas and current product result schemas', () => {
  const api = describeAgentApi();
  assert.equal(api.inputSchema, null);
  assert.equal(api.outputSchema, null);
  assert.deepEqual(new Set(api.methods), new Set(requests.map((r) => r.method)));
  assert.deepEqual(new Set(Object.keys(agentResultSchemas)), new Set(api.methods));
  assert.ok(Buffer.byteLength(JSON.stringify(api)) < 4096);
  for (const request of requests) {
    const selected = describeAgentApi(request.method, true);
    assert.ok(selected.inputSchema);
    assert.ok(selected.outputSchema);
    assert.ok(api.methodDescriptions[request.method].length > 15);
    const validate = new Ajv({ strict: false }).compile(selected.inputSchema);
    assert.equal(validate(request), true, JSON.stringify(validate.errors));
    assert.equal(validate({ ...request, unexpected: true }), false);
    assert.equal(validate({ ...request, method: 'arbitrary_shell' }), false);
    assert.equal(validateAgentRequest(request), true, request.method);
  }
  assert.throws(() => describeAgentApi('arbitrary_shell'));
});
test('API discovery and argument failures never construct a runtime client', async () => {
  for (const [args, expected] of [
    [[], 0],
    [['--method', 'timing', '--output-schema'], 0],
    [['--method', 'unknown'], 1],
    [['--output-schema'], 1],
    [['--json=1'], 1],
    [['--lang', 'unsupported'], 1],
  ] as const) {
    const io = capture();
    assert.equal(
      await runAgentCli('api', [...args], {
        ...io,
        createClient: () => {
          throw Error('private canary');
        },
      }),
      expected,
    );
    assert.equal(io.out.length, 1);
    assert.doesNotMatch(io.out[0], /private canary/);
    JSON.parse(io.out[0]);
  }
});
test('structured calls retain nested selection, emit unmodified product JSON and shared partial exits', async () => {
  for (const response of [local, capabilityResult]) {
    const io = capture(),
      request =
        response === local
          ? {
              method: 'timing',
              params: {
                action: 'summary',
                threadId: 'thread',
                turnId: 'turn',
                snapshotId: 'fixed',
                scope: { sourceInstanceId: 'source' },
              },
            }
          : requests[4];
    let calls = 0;
    const code = await runAgentCli('call', [], {
      ...io,
      stdin: Readable.from([JSON.stringify(request)]),
      createClient: () => ({
        timing: async (params, q) => {
          calls++;
          assert.deepEqual(params, request.params);
          assert.ok(q?.signal);
          return response;
        },
      }),
    });
    assert.equal(calls, 1);
    assert.equal(code, response === local ? 2 : 0);
    assert.equal(io.out.length, 1);
    assert.equal(io.out[0], JSON.stringify(response) + '\n');
  }
});
test('invalid, oversized, malformed UTF-8 and multiple input objects fail before dispatch', async () => {
  for (const input of [
    '{}',
    JSON.stringify({ method: 'shell', params: { command: 'private canary' } }),
    JSON.stringify({ method: 'timing', params: { action: 'capabilities', unexpected: true } }),
    '{}\n{}',
    ' '.repeat(1024 * 1024 + 1),
    Buffer.from([0xff]),
  ]) {
    const io = capture();
    let calls = 0;
    assert.equal(
      await runAgentCli('call', [], {
        ...io,
        stdin: Readable.from([input]),
        createClient: () => {
          calls++;
          return {};
        },
      }),
      1,
    );
    assert.equal(calls, 0);
    assert.equal(io.out.length, 1);
    assert.doesNotMatch(io.out[0], /private canary/);
    const error = JSON.parse(io.out[0]).error;
    assert.ok(['INVALID_ARGUMENT', 'RESOURCE_LIMIT'].includes(error.code));
  }
});
test('cancellation interrupts stdin and removes all owned listeners', async () => {
  const before = ['SIGINT', 'SIGTERM'].map((n) => process.listenerCount(n));
  const input = new PassThrough(),
    controller = new AbortController(),
    io = capture();
  const pending = runAgentCli('call', [], { ...io, stdin: input, signal: controller.signal });
  controller.abort();
  assert.equal(await pending, 130);
  assert.equal(JSON.parse(io.out[0]).error.recovery, 'none');
  for (const event of ['data', 'end', 'error']) assert.equal(input.listenerCount(event), 0);
  assert.deepEqual(
    ['SIGINT', 'SIGTERM'].map((n) => process.listenerCount(n)),
    before,
  );
});
test('unfinished stdin expires without dispatch and releases stream listeners', async (t) => {
  t.mock.timers.enable({ apis: ['setTimeout'] });
  const input = new PassThrough(),
    io = capture();
  let calls = 0;
  const pending = runAgentCli('call', [], {
    ...io,
    stdin: input,
    createClient: () => {
      calls++;
      return {};
    },
  });
  input.write('{"method":');
  t.mock.timers.tick(10_000);
  assert.equal(await pending, 1);
  assert.equal(calls, 0);
  assert.equal(JSON.parse(io.out[0]).error.code, 'TIMEOUT');
  for (const event of ['data', 'end', 'error']) assert.equal(input.listenerCount(event), 0);
});
test('oversized product output returns one recovery error without a partial JSON prefix', async () => {
  const io = capture();
  const result = {
    ...local,
    privacy: { ...local.privacy, omittedFields: ['synthetic'.repeat(300_000)] },
  };
  assert.equal(
    await runAgentCli('call', [], {
      ...io,
      stdin: Readable.from([JSON.stringify(requests[4])]),
      createClient: () => ({ timing: async () => result }),
    }),
    1,
  );
  assert.equal(io.out.length, 1);
  const error = JSON.parse(io.out[0]).error;
  assert.equal(error.code, 'OUTPUT_LIMIT');
  assert.equal(error.recovery, 'narrow_query');
  assert.ok(io.out[0].length < 1024);
});
test('errors retain stable product codes and actionable recovery without private details', async () => {
  for (const [code, recovery] of [
    ['VIEW_EXPIRED', 'reacquire_view'],
    ['SKILL_RUNTIME_MISMATCH', 'check_setup'],
    ['SYNC_PENDING', 'retry_same_scope'],
    ['RESOURCE_LIMIT', 'narrow_query'],
    ['UNSUPPORTED_VERSION', 'inspect_state'],
  ] as const) {
    const io = capture();
    assert.equal(
      await runAgentCli('call', [], {
        ...io,
        stdin: Readable.from([JSON.stringify(requests[4])]),
        createClient: () => ({
          timing: async () => {
            throw new CoreError(code, '/private/canary', { secret: 'private canary' });
          },
        }),
      }),
      1,
    );
    const error = JSON.parse(io.out[0]).error;
    assert.equal(error.code, code);
    assert.equal(error.recovery, recovery);
    assert.doesNotMatch(io.out[0], /private|canary|secret/);
  }
  assert.equal(
    agentErrorOutput(new CoreError('/private/canary', 'private canary')).error.code,
    'INTERNAL_ERROR',
  );
});
