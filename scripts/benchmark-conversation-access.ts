/** Synthetic-only experiment. Measures a candidate index, not the Rust service,
 * CLI, Web, a production source adapter, or operating-system cold-cache latency.
 */
import assert from 'node:assert/strict';
import { spawnSync } from 'node:child_process';
import { closeSync, fstatSync, mkdtempSync, openSync, readFileSync, readSync, rmSync, statSync, writeFileSync, writeSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { performance } from 'node:perf_hooks';
import { fileURLToPath } from 'node:url';
import { ConversationPrototype, records, type SyntheticRecord } from './conversation-access-prototype.js';
import { option, positiveInteger, sha256 } from './benchmark-common.js';

interface Corpus {
  name: string;
  files: string[];
  database: string;
  records: number;
  bytes: number;
  target: { thread: string; turn: string; needle: string; file: string; offset: number; bytes: number; textHash: string };
}
const rounds = positiveInteger('--rounds', 30);
assert(rounds <= 100, 'Rounds must be at most 100');
function measured<T>(work: () => T) { const start = performance.now(); const value = work(); return { ms: performance.now() - start, value }; }
function stats(samples: number[]) {
  const sorted = [...samples].sort((a, b) => a - b), q = (p: number) => sorted[Math.min(sorted.length - 1, Math.ceil(sorted.length * p) - 1)];
  return { rounds: samples.length, p50Ms: q(0.5), p95Ms: q(0.95), maximumMs: sorted.at(-1) };
}
function knownOffset(corpus: Corpus) {
  const fd = openSync(corpus.target.file, 'r');
  try {
    assert(fstatSync(fd).isFile());
    const bytes = Buffer.alloc(corpus.target.bytes);
    assert.equal(readSync(fd, bytes, 0, bytes.length, corpus.target.offset), bytes.length);
    const record = JSON.parse(bytes.toString('utf8')) as SyntheticRecord;
    assert.equal(sha256(record.text), corpus.target.textHash);
  } finally { closeSync(fd); }
}
function fullScan(corpus: Corpus) {
  let bytes = 0, matches = 0;
  for (const file of corpus.files) {
    for (const row of records(file)) {
      bytes += row.bytes.length;
      const value = JSON.parse(row.bytes.toString('utf8')) as SyntheticRecord;
      if (value.thread === corpus.target.thread && value.turn === corpus.target.turn && value.phase === 'final_answer') {
        assert.equal(sha256(value.text), corpus.target.textHash); matches++;
      }
    }
  }
  assert.equal(matches, 1);
  return bytes;
}

if (process.argv.includes('--worker')) {
  const corpus = JSON.parse(readFileSync(option('--corpus'), 'utf8')) as Corpus;
  const stage = option('--stage');
  if (stage === 'index') {
    const p = new ConversationPrototype(corpus.database, true);
    try {
      const result = measured(() => p.index(corpus.files));
      assert.equal(result.value, corpus.records);
      console.log(JSON.stringify({ elapsedMs: result.ms, indexBytes: statSync(corpus.database).size, maxRssKiB: process.resourceUsage().maxRSS }));
    } finally { p.close(); }
  } else if (stage === 'query') {
    const started = performance.now(), p = new ConversationPrototype(corpus.database);
    const filters = { sources: ['synthetic'], projects: ['/synthetic/project'], threads: [corpus.target.thread], turns: [corpus.target.turn], phases: ['final_answer'] as ('final_answer')[] };
    try {
      const query = () => {
        const list = p.list(filters, 1);
        assert.equal(list.locations.length, 1);
        const item = list.locations[0], result = p.read(item.id, { maxBytes: 256 * 1024 });
        if (corpus.target.bytes < 256 * 1024) assert.equal(sha256(result.text), corpus.target.textHash);
        else assert(result.nextOffset !== null);
        return { readBytes: result.readBytes, responseBytes: Buffer.byteLength(JSON.stringify({ list, result })) };
      };
      const first = measured(query), firstWithOpenMs = performance.now() - started;
      const samples: number[] = [], locations: number[] = [], daily: number[] = [];
      for (let i = 0; i < rounds; i++) {
        samples.push(measured(query).ms);
        locations.push(measured(() => p.list(filters, 1)).ms);
        daily.push(measured(() => p.list({ projects: ['/synthetic/project'], roles: ['user', 'assistant'], phases: ['request', 'final_answer'], since: '2026-10-14T00:00:00.000Z', until: '2026-10-15T00:00:00.000Z' }, 20)).ms);
      }
      const byteStart = p.readBytes;
      const keyword = measured(() => p.search({ threads: [corpus.target.thread] }, corpus.target.needle, { maxScanBytes: 8 * 1024 * 1024 }));
      // This intentionally allows incomplete keyword results: budgets are not a whole-task guarantee.
      assert.equal(keyword.value.readBytes, p.readBytes - byteStart);
      assert(keyword.value.readBytes <= 8 * 1024 * 1024);
      const partialRead = measured(() => {
        const id = p.list(filters, 1).locations[0].id;
        let offset = 0, hashInput = '', readBytes = 0, pages = 0;
        while (true) {
          const page = p.read(id, { offset, maxBytes: 256 * 1024 });
          hashInput += page.text; readBytes += page.readBytes; pages++;
          if (page.nextOffset === null) break;
          offset = page.nextOffset;
        }
        assert.equal(sha256(hashInput), corpus.target.textHash);
        return { pages, readBytes, amplification: readBytes / corpus.target.bytes };
      });
      console.log(JSON.stringify({ firstWithOpenMs, firstQueryMs: first.ms, locate: stats(locations), dailyPage: stats(daily), locateAndRead: stats(samples), readBytes: first.value.readBytes, responseBytes: first.value.responseBytes, fullMessageViaPages: { elapsedMs: partialRead.ms, ...partialRead.value }, keyword: { elapsedMs: keyword.ms, ...keyword.value, locations: keyword.value.locations.map(x => x.id) }, maxRssKiB: process.resourceUsage().maxRSS }));
    } finally { p.close(); }
  } else if (stage === 'baseline') {
    const scan: number[] = [], direct: number[] = [], rg: number[] = [];
    let readBytes = 0;
    const first = measured(() => fullScan(corpus));
    for (let i = 0; i < rounds; i++) {
      const result = measured(() => fullScan(corpus)); readBytes = result.value; scan.push(result.ms);
      direct.push(measured(() => knownOffset(corpus)).ms);
      const searched = measured(() => spawnSync('rg', ['--fixed-strings', '--only-matching', '--byte-offset', '--', corpus.target.needle, corpus.target.file], { encoding: 'utf8', timeout: 10_000, maxBuffer: 64 * 1024 }));
      assert.equal(searched.value.status, 0, searched.value.error?.message ?? 'Native search failed');
      assert(searched.value.stdout.includes(corpus.target.needle)); rg.push(searched.ms);
    }
    console.log(JSON.stringify({ firstScanMs: first.ms, scan: stats(scan), rgKnownFileLocate: stats(rg), knownOffsetRead: stats(direct), readBytes, maxRssKiB: process.resourceUsage().maxRSS }));
  } else throw new Error('Unknown worker stage');
} else {
  const output = path.resolve(option('--output'));
  const repository = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
  assert(!output.startsWith(repository + path.sep), 'Write experiment evidence outside the repository');
  const root = mkdtempSync(path.join(os.tmpdir(), 'wombat-conversation-bench-'));
  const results: unknown[] = [];
  const commands: string[][] = [];
  const specs = [{ name: 'short', files: 1, rows: 60, body: 512 }, { name: 'long', files: 1, rows: 24_000, body: 4096 }, { name: 'many', files: 128, rows: 96, body: 1024 }, { name: 'large-record', files: 1, rows: 3, body: 2 * 1024 * 1024 }];
  const rgVersion = spawnSync('rg', ['--version'], { encoding: 'utf8', timeout: 5000, maxBuffer: 4096 });
  assert.equal(rgVersion.status, 0, 'Install ripgrep for the native-tool baseline');
  try {
    for (const spec of specs) {
      const corpus: Corpus = { name: spec.name, files: [], database: path.join(root, spec.name + '.sqlite'), records: spec.files * spec.rows, bytes: 0, target: { thread: '', turn: '', needle: '', file: '', offset: 0, bytes: 0, textHash: '' } };
      const filler = 'x'.repeat(spec.body);
      for (let file = 0; file < spec.files; file++) {
        const source = path.join(root, `${spec.name}-${file}.jsonl`), fd = openSync(source, 'wx', 0o600);
        corpus.files.push(source);
        let offset = 0;
        try {
          for (let row = 0; row < spec.rows; row++) {
            const needle = `DAILY_ANCHOR_${file}_${row}`, text = `${needle} 中文🙂 ${filler}`;
            const record: SyntheticRecord = { source: 'synthetic', project: '/synthetic/project', thread: `thread-${file}`, turn: `turn-${Math.floor(row / 3)}`, timestamp: '2026-10-14T00:00:00.000Z', role: row % 3 === 0 ? 'user' : 'assistant', phase: row % 3 === 0 ? 'request' : row % 3 === 1 ? 'commentary' : 'final_answer', text };
            const bytes = Buffer.from(JSON.stringify(record) + '\n');
            writeSync(fd, bytes);
            if (file === spec.files - 1 && row === spec.rows - 1) corpus.target = { thread: record.thread, turn: record.turn, needle, file: source, offset, bytes: bytes.length, textHash: sha256(text) };
            offset += bytes.length;
          }
        } finally { closeSync(fd); }
        corpus.bytes += offset;
      }
      const corpusFile = path.join(root, spec.name + '.json'); writeFileSync(corpusFile, JSON.stringify(corpus));
      const worker = (stage: string) => {
        const args = ['--import', 'tsx', fileURLToPath(import.meta.url), '--worker', '--corpus', corpusFile, '--stage', stage, '--rounds', String(rounds)];
        commands.push([process.execPath, ...args]);
        const start = performance.now(), result = spawnSync(process.execPath, args, { encoding: 'utf8', timeout: 120_000, maxBuffer: 1024 * 1024 });
        assert.equal(result.status, 0, `${stage}: ${result.error?.message ?? ''}\n${result.stderr}`);
        return { ...JSON.parse(result.stdout), workerWallMs: performance.now() - start };
      };
      const index = worker('index'), query = worker('query'), baseline = worker('baseline');
      results.push({ name: spec.name, records: corpus.records, sourceBytes: corpus.bytes, index, query, baseline, speedupVsFullScanP95: baseline.scan.p95Ms / query.locateAndRead.p95Ms, speedupVsRgLocateP95: baseline.rgKnownFileLocate.p95Ms / query.locate.p95Ms, fewerReadBytes: corpus.bytes / query.readBytes, below100ms: query.locateAndRead.p95Ms < 100 });
      console.log(`${spec.name}: query P95 ${query.locateAndRead.p95Ms.toFixed(3)} ms; scan P95 ${baseline.scan.p95Ms.toFixed(3)} ms; index ${(index.indexBytes / 1024 / 1024).toFixed(2)} MiB`);
    }
    writeFileSync(output, JSON.stringify({ formatVersion: 1, scope: 'synthetic conversation-access prototype only', platform: `${process.platform}/${process.arch}`, node: process.version, rg: rgVersion.stdout.split('\n')[0], cache: 'new index and new query process; OS page cache uncontrolled and likely warm after fixture/index writes; warm samples repeat exact selection', limits: ['No Rust adapter, live service, CLI or Web latency measured', 'Index build is an additional standalone research index; production must reuse existing facts', 'Keyword search scans selected original records and can require continuation', 'Record fingerprints are experimental; product privacy policy needs explicit design review', 'No durable incremental/restart migration, encrypted/media/delta assembly, timezone conversion or semantic key-dialogue ranking validated', 'Known-offset baseline already knows the answer location and is a lower bound', 'rg includes native subprocess startup; prototype timings are in-process; worker wall times include Node/tsx startup', 'RSS is process peak, including runtime, not per-request allocation or a hard ceiling'], results, commands }, null, 2) + '\n');
  } finally { rmSync(root, { recursive: true, force: true }); }
}
