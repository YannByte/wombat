/** Synthetic storage workload, not an adapter or a public product contract. */
import { createHash } from 'node:crypto';
import { constants, zstdCompressSync, zstdDecompressSync } from 'node:zlib';
import assert from 'node:assert/strict';

export interface StorageFixture {
  events: number;
  tasks: number;
  unique: boolean;
  blockRows: number;
}
export interface StorageRow {
  seq: number;
  task: number;
  turn: number;
  project: number;
  model: number;
  time: number;
  kind: number;
  tokens: number | null;
  status: number;
  duration: number;
  id: string;
  event: string;
  fact: string | null;
}
export const hash = (s: string | Buffer) => createHash('sha256').update(s).digest('hex');
export function fixtureRow(seq: number, spec: StorageFixture): StorageRow {
  const perTask = Math.ceil(spec.events / spec.tasks);
  const task = Math.floor(seq / perTask),
    local = seq % perTask;
  const turn = Math.floor(local / 60),
    project = task % 24,
    model = (turn + task) % 6;
  const cycle = local % 60;
  const kind =
    cycle % 15 === 0 ? 0 : [1, 13, 25, 37, 49].includes(cycle) ? 1 : cycle % 3 === 0 ? 2 : 3;
  const time = 1790812800 + (task % 30) * 86400 + local;
  const timestamp = new Date(time * 1000).toISOString();
  const threadId = hash(`task:${task}`),
    turnId = hash(`turn:${task}:${turn}`);
  const position = {
    sourceInstanceId: hash('synthetic-source'),
    fileId: `synthetic-file:${task}`,
    generation: hash(`generation:${task}`),
    byteOffset: local * 768,
    ordinal: 0,
  };
  const id = hash(JSON.stringify(position));
  const input = 100 + (seq % 1000),
    output = 10 + (seq % 100);
  const tokens = kind === 0 && seq % 97 !== 0 ? input + output : null;
  const status = seq % 7 === 0 ? 1 : 0,
    duration = seq % 120_000;
  const evidence = [
    {
      file: `/synthetic/project-${project}/sessions/${hash(`file:${task}`)}.jsonl`,
      line: local + 1,
    },
  ];
  let payload: Record<string, unknown>,
    fact: Record<string, unknown> | null = null;
  if (kind === 0) {
    fact = {
      id: hash(`measurement:${seq}`),
      agentKind: 'codex',
      sourceInstanceId: position.sourceInstanceId,
      threadId,
      turnId,
      responseId: `response-${seq}`,
      timestamp,
      intervalEnd: null,
      grain: 'response',
      timePrecision: 'millisecond',
      model: {
        raw: `synthetic-model-${model}`,
        provider: 'synthetic',
        apiProvider: 'synthetic',
        pricingModel: `synthetic-model-${model}`,
      },
      reasoningEffort: 'high',
      tokens: {
        input,
        cacheRead: 40,
        cacheCreate: 0,
        output,
        reasoning: 2,
        total: tokens,
        rawInput: input + 40,
      },
      tokenUnavailableReasons: {
        input: null,
        cacheRead: null,
        cacheCreate: null,
        output: null,
        reasoning: null,
        total: tokens === null ? 'missing' : null,
        rawInput: null,
      },
      pricingContextConflict: false,
      requestScoped: true,
      reportedCost: null,
      serviceTier: null,
      sequence: seq,
      evidence,
    };
    payload = {
      kind: 'measurement',
      value: fact,
      contextConflicts: [],
      direct: true,
      cumulative: null,
      interval_start: null,
      fingerprint: hash(`fingerprint:${seq}`),
    };
  } else if (kind === 1) {
    fact = {
      id: hash(`operation:${seq}`),
      threadId,
      turnId,
      itemId: null,
      callId: `call-${seq}`,
      responseId: null,
      kind: 'tool',
      name: 'read_file',
      sequence: seq,
      timestamp,
      timePrecision: 'millisecond',
      status: status ? 'failed' : 'completed',
      exitCode: status,
      outcomeConflict: false,
      durationMs: duration,
      path: `/synthetic/project-${project}/file-${spec.unique ? hash(`path:${seq}`) : seq % 32}.ts`,
      work: null,
      matching: null,
      server: null,
      tool: null,
      evidence,
      textResult: { methodVersion: 1, hash: hash(`result:${seq}`) },
    };
    payload = { kind: 'operation', value: fact, phase: status ? 'failed' : 'completed' };
  } else if (kind === 2) {
    payload = {
      kind: 'lifecycle',
      lifecycle: 'model_request',
      phase: cycle % 2 ? 'completed' : 'started',
      native_id: `request-${spec.unique ? hash(`native:${seq}`) : `${task}:${turn}`}`,
      duration_ms: null,
      first_token_ms: null,
    };
  } else {
    payload = {
      kind: 'message',
      origin: cycle % 2 ? 'assistant_visible' : 'reasoning',
      presence: 'non_empty',
      native_id: spec.unique ? hash(`message:${seq}`) : null,
      record_kind: 'response_snapshot',
      record_phase: 'completed',
      content_phase: 'commentary',
    };
  }
  const event = JSON.stringify({
    version: 4,
    id,
    position,
    threadId,
    turnId,
    time: { timestamp, precision: 'millisecond' },
    collectedAt: '2026-10-11T00:00:00.000Z',
    gaps: seq % 101 === 0 ? ['source_partial'] : [],
    payload,
  });
  return {
    seq,
    task,
    turn,
    project,
    model,
    time,
    kind,
    tokens,
    status,
    duration,
    id,
    event,
    fact: fact && JSON.stringify(fact),
  };
}
export function pack(
  rows: StorageRow[],
  level: number,
): { bytes: Buffer; digest: string; decodedBytes: number } {
  const raw = Buffer.from('[' + rows.map((row) => row.event).join(',') + ']');
  return {
    bytes: zstdCompressSync(raw, { params: { [constants.ZSTD_c_compressionLevel]: level } }),
    digest: hash(raw),
    decodedBytes: raw.length,
  };
}
export function unpack(bytes: Uint8Array, size: number, digest: string): unknown[] {
  assert(
    Number.isSafeInteger(size) && size > 0 && size <= 4 * 1024 * 1024,
    'Invalid decoded block size',
  );
  const raw = zstdDecompressSync(bytes, { maxOutputLength: size });
  assert.equal(raw.length, size, 'Decoded size differs');
  assert.equal(hash(raw), digest, 'Block digest differs');
  const events: unknown = JSON.parse(raw.toString());
  assert(Array.isArray(events), 'Expected block array');
  return events;
}
export function metric(values: number[]) {
  assert(values.length > 0);
  const sorted = [...values].sort((a, b) => a - b);
  const q = (p: number) => sorted[Math.ceil((sorted.length - 1) * p)];
  return {
    samples: values.length,
    p50Ms: q(0.5),
    p95Ms: q(0.95),
    p99Ms: q(0.99),
    maxMs: sorted.at(-1),
    over100ms: values.filter((n) => n > 100).length,
  };
}
