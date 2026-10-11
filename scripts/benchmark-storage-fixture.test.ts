import test from 'node:test';
import assert from 'node:assert/strict';
import { fixtureRow, pack, unpack, metric } from './benchmark-storage-fixture.ts';
import { validateComparison } from './benchmark-storage-report.ts';

test('storage comparisons reject mismatched scope and failed or missing evidence', () => {
  const base = {
    engine: 'sqlite',
    spec: { events: 60 },
    oracle: { events: 60, tokens: 140 },
    settled: { bytes: 4096, allocated: 4096 },
  };
  assert.doesNotThrow(() => validateComparison([base, { ...base, engine: 'duckdb' }]));
  assert.throws(
    () => validateComparison([base, { ...base, spec: { events: 120 } }]),
    /specifications differ/,
  );
  assert.throws(
    () => validateComparison([base, { ...base, oracle: { events: 60, tokens: 0 } }]),
    /truth differs/,
  );
  assert.throws(() => validateComparison([base, { ...base, error: 'timeout' }]), /Incomplete/);
});

test('storage fixture keeps events distinct and preserves full evidence through compression', () => {
  const spec = { events: 120, tasks: 2, unique: false, blockRows: 60 };
  const rows = Array.from({ length: 60 }, (_, i) => fixtureRow(i, spec));
  assert.equal(new Set(rows.map((r) => r.id)).size, 60);
  assert.equal(rows.filter((r) => r.kind === 0).length, 4);
  assert.equal(rows.filter((r) => r.kind === 1).length, 5);
  assert.equal(rows[0].tokens, null);
  assert.equal(rows[15].tokens, 140);
  const p = pack(rows, 3);
  assert.deepEqual(
    unpack(p.bytes, p.decodedBytes, p.digest),
    rows.map((r) => JSON.parse(r.event)),
  );
  assert.throws(() => unpack(p.bytes, p.decodedBytes, '0'.repeat(64)), /digest/);
  assert.throws(() => unpack(p.bytes, 0, p.digest), /size/);
  assert.throws(() => unpack(p.bytes, p.decodedBytes - 1, p.digest));
});
test('storage latency gate retains maximum and counts every 100 ms violation', () => {
  assert.deepEqual(metric([1, 2, 100, 101]), {
    samples: 4,
    p50Ms: 100,
    p95Ms: 101,
    p99Ms: 101,
    maxMs: 101,
    over100ms: 1,
  });
  assert.throws(() => metric([]));
});
