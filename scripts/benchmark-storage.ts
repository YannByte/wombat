/** Reproducible storage selection experiments; not full product acceptance. */
import assert from 'node:assert/strict';
import { createHash } from 'node:crypto';
import { spawn } from 'node:child_process';
import {
  mkdirSync,
  existsSync,
  readdirSync,
  statSync,
  statfsSync,
  writeFileSync,
  openSync,
  closeSync,
  writeSync,
} from 'node:fs';
import path from 'node:path';
import os from 'node:os';
import { fileURLToPath } from 'node:url';
import { performance } from 'node:perf_hooks';
import { DatabaseSync } from 'node:sqlite';
import { option, positiveInteger } from './benchmark-common.ts';
import {
  fixtureRow,
  pack,
  unpack,
  metric,
  type StorageFixture,
  type StorageRow,
} from './benchmark-storage-fixture.ts';
import {
  openEngine,
  createSchema,
  createIndexes,
  readSourceBlock,
  type Engine,
  type ResultRow,
} from './benchmark-storage-engine.ts';

if (process.argv.includes('--collect')) {
  const { collectStorageReport } = await import('./benchmark-storage-report.ts');
  collectStorageReport();
} else {
  const output = path.resolve(option('--output'));
  const name = option('--engine');
  assert(['baseline', 'sqlite', 'duckdb', 'tiered'].includes(name), 'Unknown engine');
  const spec: StorageFixture = {
    events: positiveInteger('--events', 30_000),
    tasks: positiveInteger('--tasks', 120),
    blockRows: positiveInteger('--block-rows', 240),
    unique: process.argv.includes('--unique'),
  };
  assert(
    spec.events % spec.tasks === 0 && spec.events / spec.tasks >= 60 && spec.blockRows <= 1000,
    'Use equally sized tasks with at least 60 events each, and at most 1000 rows per block',
  );
  const rounds = positiveInteger('--rounds', 20);
  const modulePath = path.resolve(
    option(
      '--duckdb-module',
      '/private/tmp/wombat-storage-research-runtime/node_modules/@duckdb/node-api',
    ),
  );
  mkdirSync(output, { recursive: true });
  const file = path.join(output, name + '.db'),
    reportFile = path.join(output, name + '.json');
  function footprint() {
    let bytes = 0,
      allocated = 0;
    for (const n of readdirSync(output)) {
      if (n.startsWith(name + '.db')) {
        const s = statSync(path.join(output, n));
        bytes += s.size;
        allocated += s.blocks * 512;
      }
    }
    return { bytes, allocated };
  }
  function save(value: unknown) {
    writeFileSync(reportFile, JSON.stringify(value, null, 2) + '\n');
  }
  if (!process.argv.includes('--worker')) {
    assert(!existsSync(file), 'Use a new experiment output directory');
    const free = statfsSync(output);
    assert(
      free.bavail * free.bsize >
        spec.events * (name === 'baseline' ? 1800 : name === 'tiered' ? 1500 : 350) +
          256 * 1024 * 1024,
      'Insufficient space for bounded experiment',
    );
    const child = spawn(
      process.execPath,
      ['--import', 'tsx', fileURLToPath(import.meta.url), ...process.argv.slice(2), '--worker'],
      { stdio: ['ignore', 'pipe', 'pipe'] },
    );
    let bytes = 0;
    let stopReason: string | undefined;
    const timer = setTimeout(() => {
      stopReason = '900000 ms worker deadline exceeded';
      child.kill('SIGKILL');
    }, 900_000);
    const cancel = () => {
      stopReason = 'Canceled';
      child.kill('SIGTERM');
    };
    process.once('SIGINT', cancel);
    process.once('SIGTERM', cancel);
    for (const stream of [child.stdout, child.stderr])
      stream?.on('data', (data: Buffer) => {
        bytes += data.length;
        if (bytes > 1024 * 1024) {
          stopReason = 'Worker output exceeded 1 MiB';
          child.kill('SIGKILL');
        } else (stream === child.stdout ? process.stdout : process.stderr).write(data);
      });
    try {
      const code = await new Promise<number | null>((resolve, reject) => {
        child.once('error', reject);
        child.once('exit', resolve);
      });
      if (code !== 0 && !existsSync(reportFile))
        save({
          engine: name,
          spec,
          status: 'failed',
          error: stopReason ?? `Worker exit ${code}`,
          footprint: footprint(),
        });
      assert.equal(code, 0, 'Storage worker failed; retain its report and database');
    } finally {
      clearTimeout(timer);
      process.removeListener('SIGINT', cancel);
      process.removeListener('SIGTERM', cancel);
    }
  } else {
    const start = performance.now();
    let peak = 0;
    const capture = () => {
      peak = Math.max(peak, footprint().bytes);
    };
    const expectedHash = createHash('sha256');
    let expectedTokens = 0,
      expectedMeasurements = 0,
      expectedOperations = 0,
      expectedMissing = 0;
    const taskTotals = new Map<number, number>(),
      days = new Map<number, number>();
    let failed = 0,
      duration = 0;
    const ledgerRows: Array<{
      task: number;
      project: number;
      model: number;
      time: number;
      tokens: number | null;
    }> = [];
    const observe = (r: StorageRow) => {
      expectedHash.update(r.event + '\n');
      if (r.kind === 0) {
        expectedMeasurements++;
        if (r.tokens === null) expectedMissing++;
        else expectedTokens += r.tokens;
        taskTotals.set(r.task, (taskTotals.get(r.task) ?? 0) + (r.tokens ?? 0));
        const d = Math.floor((r.time + 28800) / 86400);
        days.set(d, (days.get(d) ?? 0) + (r.tokens ?? 0));
        ledgerRows.push({
          task: r.task,
          project: r.project,
          model: r.model,
          time: r.time,
          tokens: r.tokens,
        });
      }
      if (r.kind === 1) {
        expectedOperations++;
        failed += r.status;
        duration += r.duration;
      }
    };
    let db: Engine | undefined;
    const sourceFile = path.join(output, 'synthetic-source.blocks');
    let sourceFd: number | undefined,
      sourceOffset = 0;
    const engineName = name === 'tiered' ? 'sqlite' : name;
    const decode = (row: ResultRow): unknown[] => {
      if (name !== 'tiered')
        return unpack(row.data as Uint8Array, Number(row.decoded), String(row.digest));
      assert(sourceFd !== undefined);
      return readSourceBlock(
        sourceFd,
        row.data as Uint8Array,
        Number(row.decoded),
        String(row.digest),
      );
    };
    try {
      if (name === 'baseline') {
        const base = new DatabaseSync(file);
        base.exec(`PRAGMA page_size=16384; PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL; PRAGMA cache_size=-8192;
        CREATE TABLE buckets(id INTEGER PRIMARY KEY,scope TEXT,field TEXT,UNIQUE(scope,field));
        CREATE TABLE entries(bucket INTEGER,id TEXT,payload BLOB,source_bucket INTEGER,member TEXT,PRIMARY KEY(bucket,id)) WITHOUT ROWID;
        INSERT INTO buckets VALUES(1,'parser:synthetic','events'),(2,'parser:synthetic','measurements'),(3,'parser:synthetic','operations'),(24,'projection:synthetic','events'),(25,'projection:synthetic','measurements'),(26,'projection:synthetic','operations');
        CREATE INDEX event_threads_1 ON entries(json_extract(payload,'$.threadId'),id) WHERE bucket=1 AND payload IS NOT NULL;`);
        const insert = base.prepare('INSERT INTO entries VALUES(?,?,jsonb(?),NULL,NULL)'),
          ref = base.prepare('INSERT INTO entries VALUES(?,?,NULL,?,?)');
        base.exec('BEGIN');
        for (let i = 0; i < spec.events; i++) {
          const row = fixtureRow(i, spec);
          observe(row);
          insert.run(1, row.id, row.event);
          ref.run(24, row.id, 1, '$');
          if (row.fact) {
            const fact: Record<string, unknown> = JSON.parse(row.fact);
            const bucket = row.kind === 0 ? 2 : 3;
            insert.run(
              bucket,
              String(fact.id),
              row.kind === 0
                ? JSON.stringify({
                    measurement: fact,
                    direct: true,
                    cumulative: null,
                    interval_start: null,
                    fingerprint: 'synthetic',
                  })
                : row.fact,
            );
            ref.run(
              row.kind === 0 ? 25 : 26,
              String(fact.id),
              bucket,
              row.kind === 0 ? '$.measurement' : '$',
            );
          }
          if ((i + 1) % 10_000 === 0) {
            base.exec('COMMIT');
            capture();
            base.exec('PRAGMA wal_checkpoint(TRUNCATE); BEGIN');
          }
          if ((i + 1) % 100_000 === 0)
            console.log(JSON.stringify({ engine: name, generated: i + 1 }));
        }
        base.exec('COMMIT');
        capture();
        base.exec('PRAGMA wal_checkpoint(TRUNCATE)');
        base.close();
        save({
          engine: name,
          spec,
          elapsedMs: performance.now() - start,
          settled: footprint(),
          peakFileBytes: peak,
          peakRssBytes: process.resourceUsage().maxRSS * 1024,
          oracle: {
            events: spec.events,
            measurements: expectedMeasurements,
            operations: expectedOperations,
            tokens: expectedTokens,
            missing: expectedMissing,
            eventSha256: expectedHash.digest('hex'),
          },
          scope:
            'Synthetic schema-shaped layout baseline, not a real-core scan or current-release performance claim. Omits small checkpoint/title tables; all compared event and canonical payloads are retained.',
        });
      } else {
        db = await openEngine(engineName, file, modulePath);
        await createSchema(db);
        if (name === 'tiered') sourceFd = openSync(sourceFile, 'wx+');
        await db.exec('BEGIN TRANSACTION');
        let led = await db.writer('ledger', 7),
          ops = await db.writer('operations', 7),
          blocks = await db.writer('blocks', 9);
        let group: StorageRow[] = [],
          blockId = 0;
        const flush = () => {
          if (!group.length) return;
          let bytes: Buffer, digest: string, decodedBytes: number;
          if (name === 'tiered') {
            const raw = Buffer.from('[' + group.map((r) => r.event).join(',') + ']');
            decodedBytes = raw.length;
            digest = createHash('sha256').update(raw).digest('hex');
            assert(sourceFd !== undefined);
            assert.equal(writeSync(sourceFd, raw), raw.length);
            bytes = Buffer.alloc(16);
            bytes.writeBigUInt64LE(BigInt(sourceOffset));
            bytes.writeBigUInt64LE(BigInt(raw.length), 8);
            sourceOffset += raw.length;
          } else {
            const p = pack(group, 3);
            ({ bytes, digest, decodedBytes } = p);
          }
          blocks.append([
            blockId++,
            group[0].task,
            group[0].turn,
            group.at(-1)!.turn,
            group[0].seq,
            group.length,
            decodedBytes,
            digest,
            bytes,
          ]);
          group = [];
        };
        for (let i = 0; i < spec.events; i++) {
          const r = fixtureRow(i, spec);
          observe(r);
          if (group.length && (group[0].task !== r.task || group.length >= spec.blockRows)) flush();
          group.push(r);
          if (r.kind === 0)
            led.append([r.seq, r.task, r.turn, r.project, r.model, r.time, r.tokens]);
          if (r.kind === 1)
            ops.append([r.seq, r.task, r.turn, r.project, r.time, r.status, r.duration]);
          if ((i + 1) % 10_000 === 0) {
            flush();
            led.close();
            ops.close();
            blocks.close();
            await db.exec('COMMIT');
            capture();
            await db.checkpoint();
            await db.exec('BEGIN TRANSACTION');
            led = await db.writer('ledger', 7);
            ops = await db.writer('operations', 7);
            blocks = await db.writer('blocks', 9);
          }
          if ((i + 1) % 100_000 === 0)
            console.log(JSON.stringify({ engine: name, generated: i + 1 }));
        }
        flush();
        led.close();
        ops.close();
        blocks.close();
        await db.exec('COMMIT');
        await createIndexes(db);
        await db.checkpoint();
        capture();
        const buildMs = performance.now() - start;
        db.close();
        const reopenStart = performance.now();
        db = await openEngine(engineName, file, modulePath);
        const reopenMs = performance.now() - reopenStart;
        const truthDigest = expectedHash.digest('hex');
        const queries = [
          {
            name: 'totals',
            sql: 'SELECT count(*) AS n, sum(tokens) AS tokens, count(*)-count(tokens) AS missing FROM ledger',
            truth: [{ n: expectedMeasurements, tokens: expectedTokens, missing: expectedMissing }],
          },
          {
            name: 'daily',
            sql: 'SELECT CAST(floor((ts+28800)/86400.0) AS INTEGER) AS day, sum(tokens) AS tokens FROM ledger GROUP BY day ORDER BY day',
            truth: [...days].sort((a, b) => a[0] - b[0]).map(([day, tokens]) => ({ day, tokens })),
          },
          {
            name: 'ranking',
            sql: 'SELECT task, sum(tokens) AS tokens FROM ledger GROUP BY task ORDER BY tokens DESC,task LIMIT 20',
            truth: [...taskTotals]
              .sort((a, b) => b[1] - a[1] || a[0] - b[0])
              .slice(0, 20)
              .map(([task, tokens]) => ({ task, tokens })),
          },
          {
            name: 'outcomes',
            sql: 'SELECT count(*) AS n,sum(status) AS failures,sum(duration) AS duration FROM operations',
            truth: [{ n: expectedOperations, failures: failed, duration }],
          },
        ];
        const timings: Record<string, ReturnType<typeof metric>> = {};
        for (const q of queries) {
          const samples: number[] = [];
          for (let n = 0; n < rounds; n++) {
            const t = performance.now();
            const rows: ResultRow[] = await db.all(q.sql);
            const response: string = JSON.stringify(rows);
            samples.push(performance.now() - t);
            assert.deepEqual(JSON.parse(response), q.truth, q.name);
          }
          timings[q.name] = metric(samples);
        }
        const aggregateStart = performance.now();
        for (const q of queries)
          await db.exec(`CREATE TABLE aggregate_${q.name} AS ${q.sql.replace(' LIMIT 20', '')}`);
        const aggregateBuildMs = performance.now() - aggregateStart;
        const aggregateTimings: Record<string, ReturnType<typeof metric>> = {};
        for (const q of queries) {
          const suffix =
            q.name === 'daily'
              ? ' ORDER BY day'
              : q.name === 'ranking'
                ? ' ORDER BY tokens DESC,task LIMIT 20'
                : '';
          const samples: number[] = [];
          for (let n = 0; n < rounds; n++) {
            const t = performance.now();
            const result: string = JSON.stringify(
              await db.all(`SELECT * FROM aggregate_${q.name}${suffix}`),
            );
            samples.push(performance.now() - t);
            assert.deepEqual(JSON.parse(result), q.truth);
          }
          aggregateTimings[q.name] = metric(samples);
        }
        const filters: number[] = [],
          evidence: number[] = [];
        for (let n = 0; n < rounds; n++) {
          const project = n % 24,
            model = n % 6,
            cut = 1790812800 + (n % 20) * 86400;
          const wanted = ledgerRows.filter(
            (r) => r.project === project && r.model === model && r.time >= cut,
          );
          let t = performance.now();
          const result: ResultRow[] = await db.all(
            'SELECT count(*) AS n, coalesce(sum(tokens),0) AS tokens FROM ledger WHERE project=? AND model=? AND ts>=?',
            [project, model, cut],
          );
          JSON.stringify(result);
          filters.push(performance.now() - t);
          assert.deepEqual(result, [
            { n: wanted.length, tokens: wanted.reduce((s, r) => s + (r.tokens ?? 0), 0) },
          ]);
          const task = (n * 37) % spec.tasks,
            turn = n % Math.max(1, Math.floor(Math.ceil(spec.events / spec.tasks) / 60));
          t = performance.now();
          const found: ResultRow[] = await db.all(
            'SELECT decoded,digest,data FROM blocks WHERE task=? AND first_turn<=? AND last_turn>=? ORDER BY id',
            [task, turn, turn],
          );
          const decoded: unknown[] = found.flatMap((row) => decode(row));
          const target = fixtureRow(task * Math.ceil(spec.events / spec.tasks) + turn * 60, spec);
          const targetTurn = (JSON.parse(target.event) as { turnId: string }).turnId;
          const selected: unknown[] = decoded.filter(
            (event) => (event as { turnId: string }).turnId === targetTurn,
          );
          JSON.stringify(selected);
          evidence.push(performance.now() - t);
          assert.equal(
            selected.length,
            Math.min(60, spec.events - target.seq, Math.ceil(spec.events / spec.tasks) - turn * 60),
          );
        }
        timings.filtered = metric(filters);
        timings.evidence = metric(evidence);
        const verificationStart = performance.now(),
          restored = createHash('sha256');
        let seen = 0,
          last = -1;
        while (true) {
          const rows: ResultRow[] = await db.all(
            'SELECT id,decoded,digest,data FROM blocks WHERE id>? ORDER BY id LIMIT 64',
            [last],
          );
          if (!rows.length) break;
          for (const row of rows) {
            for (const value of decode(row)) {
              restored.update(JSON.stringify(value) + '\n');
              seen++;
            }
            last = Number(row.id);
          }
        }
        assert.equal(seen, spec.events);
        assert.equal(
          restored.digest('hex'),
          truthDigest,
          'Every persisted safe event must round-trip',
        );
        const verifyMs = performance.now() - verificationStart;
        // Corrections and rollback test the storage transaction, not accounting reconciliation.
        let t = performance.now();
        await db.exec(
          'BEGIN TRANSACTION; UPDATE ledger SET tokens=coalesce(tokens,0)+1 WHERE id<10000; ROLLBACK;',
        );
        const rollbackMs = performance.now() - t;
        assert.deepEqual(await db.all(queries[0].sql), queries[0].truth);
        t = performance.now();
        await db.exec(
          'BEGIN TRANSACTION; UPDATE ledger SET tokens=tokens+1 WHERE id<10000 AND tokens IS NOT NULL; COMMIT;',
        );
        const correctionMs = performance.now() - t;
        const increment = await db.all(
          'SELECT count(*) AS n FROM ledger WHERE id<10000 AND tokens IS NOT NULL',
        );
        assert.equal(
          Number((await db.all('SELECT sum(tokens) AS tokens FROM ledger'))[0].tokens),
          expectedTokens + Number(increment[0].n),
        );
        await db.checkpoint();
        capture();
        const storageObjects =
          engineName === 'sqlite'
            ? await db.all(
                'SELECT name,sum(pgsize) AS bytes FROM dbstat GROUP BY name ORDER BY bytes DESC',
              )
            : null;
        db.close();
        db = undefined;
        save({
          engine: name,
          spec,
          buildMs,
          reopenMs,
          verifyMs,
          rollbackMs,
          correctionMs,
          timings,
          aggregateBuildMs,
          aggregateTimings,
          storageObjects,
          syntheticSourceBytes: sourceOffset,
          sourceBacked: name === 'tiered',
          settled: footprint(),
          peakFileBytes: peak,
          peakRssBytes: process.resourceUsage().maxRSS * 1024,
          oracle: {
            events: seen,
            measurements: expectedMeasurements,
            operations: expectedOperations,
            tokens: expectedTokens,
            missing: expectedMissing,
            eventSha256: truthDigest,
          },
          environment: {
            node: process.version,
            platform: process.platform,
            arch: process.arch,
            cpus: os.cpus().length,
            cpuModel: os.cpus()[0]?.model,
            memoryBytes: os.totalmem(),
            duckdb: name === 'duckdb' ? '1.5.6-r.1' : null,
            cache: 'Database reopened; physical filesystem cache uncontrolled',
            concurrency: 1,
          },
          scope:
            'Storage prototype queries plus JS serialization, not CLI/HTTP latency. Full safe events retained and checked; typed columns cover six sampled query families, not all product analyses. Aggregates demonstrate fixed UTC+8 totals/daily/ranking/outcomes reads only; incremental aggregate maintenance is not implemented. Peak RSS includes fixture generation and oracle arrays, not the process tree. File peaks are sampled at transaction/checkpoint boundaries, not continuous temp-space measurements. Source-backed variant uses an immutable synthetic safe-event source, not Codex raw-log parsing: it validates lookup feasibility only, and excludes that explicitly generated input from index bytes. No crash-injection or 24-hour claim.',
        });
      }
      console.log(
        JSON.stringify({
          engine: name,
          status: 'passed',
          report: reportFile,
          settled: footprint(),
        }),
      );
    } catch (error) {
      save({
        engine: name,
        spec,
        status: 'failed',
        error: String(error),
        elapsedMs: performance.now() - start,
        footprint: footprint(),
      });
      throw error;
    } finally {
      db?.close();
      if (sourceFd !== undefined) closeSync(sourceFd);
    }
  }
}
