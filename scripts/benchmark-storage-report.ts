import assert from 'node:assert/strict';
import { readFileSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { option, sha256 } from './benchmark-common.ts';

interface Report {
  engine: string;
  spec: Record<string, unknown>;
  oracle: Record<string, unknown>;
  settled: { bytes: number; allocated: number };
  error?: unknown;
}
export function validateComparison(reports: Report[]) {
  assert(reports.length > 1, 'Need comparable complete reports');
  for (const r of reports) {
    assert(
      !r.error && r.oracle && Number.isSafeInteger(r.settled?.bytes) && r.settled.bytes > 0,
      'Incomplete experiment report',
    );
    assert.deepEqual(r.spec, reports[0].spec, 'Fixture specifications differ');
    assert.deepEqual(r.oracle, reports[0].oracle, 'Fixture truth differs');
  }
}
export function collectStorageReport() {
  const read = (directory: string, name: string): Report =>
    JSON.parse(readFileSync(path.join(directory, name + '.json'), 'utf8'));
  const comparison = ['baseline', 'sqlite', 'duckdb', 'tiered'].map((n) =>
    read(option('--comparison'), n),
  );
  const scale = ['sqlite', 'duckdb', 'tiered'].map((n) => read(option('--scale'), n));
  const variation = ['baseline', 'sqlite'].map((n) => read(option('--variation'), n));
  for (const group of [comparison, scale, variation]) validateComparison(group);
  const scriptDir = path.dirname(fileURLToPath(import.meta.url));
  const sources = Object.fromEntries(
    [
      'benchmark-storage.ts',
      'benchmark-storage-fixture.ts',
      'benchmark-storage-engine.ts',
      'benchmark-storage-report.ts',
    ].map((n) => [n, sha256(readFileSync(path.join(scriptDir, n)))]),
  );
  const ratios = comparison
    .slice(1)
    .map((r) => ({
      engine: r.engine,
      baselineBytes: comparison[0].settled.bytes,
      bytes: r.settled.bytes,
      ratio: comparison[0].settled.bytes / r.settled.bytes,
      sameOfflineRetention: r.engine !== 'tiered',
    }));
  const timeout = JSON.parse(
    readFileSync(path.join(option('--scale'), 'baseline-timeout.json'), 'utf8'),
  );
  assert.equal(timeout.status, 'timeout');
  const value = {
    formatVersion: 1,
    date: '2026-10-11',
    scope: 'Preliminary synthetic layout research, not release or product acceptance',
    sources,
    ratios,
    comparison,
    scale,
    variation,
    excludedIncompleteBaseline: timeout,
    limits: [
      'All runs use schema-shaped synthetic rows, not the Rust parser or installed product. No private conversations are included.',
      'Only six sampled queries and four fixed aggregate families are modeled. Missing product fields and all other query families remain unmeasured.',
      'Tiered data is a lower-bound index experiment against a generated safe-event source, not a Codex raw-log parser. Its generated source bytes are reported separately; it has different offline guarantees.',
      'Timing includes storage reads and JavaScript serialization only; no CLI startup, IPC, HTTP or concurrency-4 acceptance. OS cache is uncontrolled.',
      'Aggregates are built and queried before correction probes; incremental aggregate maintenance, accounting reconciliation, durable revision views and crash recovery remain unimplemented.',
      'RSS includes generator, oracle and verification allocations. File peaks are checkpoint samples, not continuous temporary-space peaks.',
      'The 3000000-event legacy-layout run timed out. Its incomplete database is excluded from ratios; the complete comparison uses 300000 events.',
    ],
  };
  writeFileSync(path.resolve(option('--output')), JSON.stringify(value, null, 2) + '\n');
  console.log(JSON.stringify({ status: 'collected', ratios }));
}
