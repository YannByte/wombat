/** Isolated research engines. None is wired into the product. */
import { DatabaseSync, type SQLInputValue } from 'node:sqlite';
import { createRequire } from 'node:module';
import { readFileSync, readSync } from 'node:fs';
import { createHash } from 'node:crypto';
import path from 'node:path';
import assert from 'node:assert/strict';

export type Cell = number | string | null | Uint8Array;
export type ResultRow = Record<string, unknown>;
export function readSourceBlock(
  fd: number,
  locator: Uint8Array,
  expectedSize: number,
  digest: string,
): unknown[] {
  assert.equal(locator.byteLength, 16, 'Invalid source locator');
  const pointer = Buffer.from(locator);
  const offset = Number(pointer.readBigUInt64LE(0)),
    length = Number(pointer.readBigUInt64LE(8));
  assert(Number.isSafeInteger(offset) && offset >= 0, 'Invalid source offset');
  assert(
    Number.isSafeInteger(length) &&
      length > 0 &&
      length <= 4 * 1024 * 1024 &&
      length === expectedSize,
    'Invalid source length',
  );
  const raw = Buffer.alloc(length);
  let done = 0;
  while (done < length) {
    const n = readSync(fd, raw, done, length - done, offset + done);
    assert(n > 0, 'Source range unavailable');
    done += n;
  }
  assert.equal(createHash('sha256').update(raw).digest('hex'), digest, 'Source generation changed');
  const values: unknown = JSON.parse(raw.toString());
  assert(Array.isArray(values));
  return values;
}
interface Appender {
  appendInteger(v: number): void;
  appendVarchar(v: string): void;
  appendBlob(v: Uint8Array): void;
  appendNull(): void;
  endRow(): void;
  closeSync(): void;
}
interface Connection {
  run(sql: string, values?: Cell[]): Promise<unknown>;
  runAndReadAll(sql: string, values?: Cell[]): Promise<{ getRowObjects(): ResultRow[] }>;
  createAppender(table: string): Promise<Appender>;
  closeSync(): void;
}
interface Instance {
  connect(): Promise<Connection>;
  closeSync(): void;
}
interface DuckModule {
  DuckDBInstance: { create(file: string, options: Record<string, string>): Promise<Instance> };
}
export interface Engine {
  name: string;
  exec(sql: string): Promise<void>;
  all(sql: string, values?: Cell[]): Promise<ResultRow[]>;
  writer(table: string, columns: number): Promise<{ append(row: Cell[]): void; close(): void }>;
  checkpoint(): Promise<void>;
  close(): void;
}
export async function openEngine(name: string, file: string, modulePath: string): Promise<Engine> {
  if (name === 'sqlite') {
    const db = new DatabaseSync(file);
    db.exec(
      'PRAGMA journal_mode=WAL; PRAGMA synchronous=FULL; PRAGMA cache_size=-8192; PRAGMA temp_store=FILE; PRAGMA wal_autocheckpoint=512;',
    );
    return {
      name,
      exec: async (sql) => {
        db.exec(sql);
      },
      all: async (sql, values = []) =>
        db
          .prepare(sql)
          .all(...(values as SQLInputValue[]))
          .map((row) => ({ ...row })),
      writer: async (table, columns) => {
        assert(/^[a-z_]+$/.test(table));
        const stmt = db.prepare(
          `INSERT INTO ${table} VALUES(${Array(columns).fill('?').join(',')})`,
        );
        return {
          append: (row) => {
            stmt.run(...(row as SQLInputValue[]));
          },
          close: () => {},
        };
      },
      checkpoint: async () => {
        db.exec('PRAGMA wal_checkpoint(TRUNCATE)');
      },
      close: () => db.close(),
    };
  }
  assert.equal(name, 'duckdb');
  const pkg = JSON.parse(readFileSync(path.join(modulePath, 'package.json'), 'utf8'));
  assert.equal(pkg.name, '@duckdb/node-api');
  assert.equal(pkg.version, '1.5.6-r.1');
  // Explicit external experiment boundary; validate the pinned package before loading it.
  const api: DuckModule = createRequire(import.meta.url)(modulePath);
  assert.equal(typeof api.DuckDBInstance?.create, 'function');
  const instance = await api.DuckDBInstance.create(file, { threads: '4', memory_limit: '256MB' });
  const db = await instance.connect();
  return {
    name,
    exec: async (sql) => {
      await db.run(sql);
    },
    all: async (sql, values = []) => {
      const rows = (await db.runAndReadAll(sql, values)).getRowObjects();
      return rows.map((row) =>
        Object.fromEntries(
          Object.entries(row).map(([k, v]) => [
            k,
            typeof v === 'bigint'
              ? (assert(
                  v <= BigInt(Number.MAX_SAFE_INTEGER) && v >= BigInt(Number.MIN_SAFE_INTEGER),
                ),
                Number(v))
              : v !== null && typeof v === 'object' && 'bytes' in v && v.bytes instanceof Uint8Array
                ? v.bytes
                : v,
          ]),
        ),
      );
    },
    writer: async (table) => {
      assert(/^[a-z_]+$/.test(table));
      const a = await db.createAppender(table);
      return {
        append: (row) => {
          for (const v of row) {
            if (v === null) a.appendNull();
            else if (typeof v === 'number') a.appendInteger(v);
            else if (typeof v === 'string') a.appendVarchar(v);
            else a.appendBlob(v);
          }
          a.endRow();
        },
        close: () => a.closeSync(),
      };
    },
    checkpoint: async () => {
      await db.run('CHECKPOINT');
    },
    close: () => {
      db.closeSync();
      instance.closeSync();
    },
  };
}
export async function createSchema(db: Engine) {
  await db.exec(`CREATE TABLE ledger(id INTEGER PRIMARY KEY, task INTEGER, turn INTEGER, project INTEGER, model INTEGER, ts INTEGER, tokens INTEGER);
    CREATE TABLE operations(id INTEGER PRIMARY KEY, task INTEGER, turn INTEGER, project INTEGER, ts INTEGER, status INTEGER, duration INTEGER);
    CREATE TABLE blocks(id INTEGER PRIMARY KEY, task INTEGER, first_turn INTEGER, last_turn INTEGER, first_seq INTEGER, count INTEGER, decoded INTEGER, digest VARCHAR, data BLOB);
    CREATE TABLE meta(key VARCHAR PRIMARY KEY,value VARCHAR);`);
}
export async function createIndexes(db: Engine) {
  await db.exec(`CREATE INDEX ledger_filter ON ledger(project,model,ts);
    CREATE INDEX ledger_task ON ledger(task,turn);
    CREATE INDEX op_task ON operations(task,turn);
    CREATE INDEX block_task ON blocks(task,first_turn);`);
}
