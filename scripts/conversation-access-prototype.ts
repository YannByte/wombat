/** Synthetic-only storage experiment, not a product API or Codex source adapter.
 * Tests seek-based access and composable selection before Rust contract design.
 * Content is never stored in SQLite. This experiment hashes complete fixture
 * records; production instead retains bounded source-text block fingerprints.
 */
import { createHash, randomUUID } from 'node:crypto';
import { closeSync, fstatSync, openSync, readSync, statSync, type Stats } from 'node:fs';
import { DatabaseSync } from 'node:sqlite';

const digest = (bytes: string | Buffer) => createHash('sha256').update(bytes).digest('hex');
const RECORD_BYTES = 4 * 1024 * 1024;
export interface Selection {
  sources?: string[];
  projects?: string[];
  threads?: string[];
  turns?: string[];
  roles?: ('user' | 'assistant')[];
  phases?: ('request' | 'commentary' | 'final_answer')[];
  since?: string;
  until?: string;
  includeUndated?: boolean;
}
export interface Location {
  id: number;
  file: number;
  sequence: number;
  source: string;
  thread: string;
  turn: string;
  timestamp: string | null;
  role: 'user' | 'assistant';
  phase: 'request' | 'commentary' | 'final_answer';
  offset: number;
  bytes: number;
  hash: string;
}
export interface Page {
  locations: Location[];
  cursor: string | null;
}
export interface SearchPage extends Page {
  scannedRecords: number;
  readBytes: number;
  complete: boolean;
}
export interface ReadPage {
  text: string;
  nextOffset: number | null;
  readBytes: number;
  context: Location[];
}
export interface ReadOptions {
  offset?: number;
  maxBytes?: number;
  before?: number;
  after?: number;
  contextScope?: 'turn' | 'thread';
  signal?: AbortSignal;
}
export interface SyntheticRecord {
  source: string;
  thread: string;
  turn: string;
  project: string;
  timestamp: string | null;
  role: Location['role'];
  phase: Location['phase'];
  text: string;
}
type FileRow = { id: number; path: string; physical: string; bytes: number };
function location(row: Record<string, unknown>): Location {
  if (row.role !== 'user' && row.role !== 'assistant') fail('INVALID_INDEX');
  if (row.phase !== 'request' && row.phase !== 'commentary' && row.phase !== 'final_answer') fail('INVALID_INDEX');
  return { id: Number(row.id), file: Number(row.file), sequence: Number(row.sequence), source: String(row.source), thread: String(row.thread), turn: String(row.turn), timestamp: row.timestamp === null ? null : String(row.timestamp), role: row.role, phase: row.phase, offset: Number(row.offset), bytes: Number(row.bytes), hash: String(row.hash) };
}

function fail(code: string): never { throw new Error(code); }
function check(signal?: AbortSignal) { if (signal?.aborted) fail('CANCELLED'); }
function physical(s: Stats) { return `${s.dev}:${s.ino}:${s.birthtimeMs}`; }
function integer(value: number, max: number, allowZero = false) {
  if (!Number.isSafeInteger(value) || value < (allowZero ? 0 : 1) || value > max) fail('INVALID_ARGUMENT');
}
/** Byte offsets remain correct for UTF-8, split chunks, CRLF and a final open tail. */
export function* records(path: string, signal?: AbortSignal): Generator<{ offset: number; bytes: Buffer }> {
  const fd = openSync(path, 'r');
  try {
    const chunk = Buffer.alloc(64 * 1024);
    let parts: Buffer[] = [], size = 0, offset = 0;
    while (true) {
      check(signal);
      const n = readSync(fd, chunk, 0, chunk.length, null);
      if (!n) break;
      const current = Buffer.from(chunk.subarray(0, n));
      let start = 0, newline: number;
      while ((newline = current.indexOf(10, start)) !== -1) {
        const last = current.subarray(start, newline + 1);
        size += last.length;
        if (size > RECORD_BYTES) fail('RESOURCE_LIMIT');
        parts.push(last);
        const bytes = parts.length === 1 ? parts[0] : Buffer.concat(parts, size);
        parts = []; size = 0;
        yield { offset, bytes };
        offset += bytes.length;
        start = newline + 1;
      }
      const tail = current.subarray(start);
      if (tail.length) { parts.push(tail); size += tail.length; }
      if (size > RECORD_BYTES) fail('RESOURCE_LIMIT');
    }
    // Unclosed records belong to a later scan, even if their JSON is parseable.
  } finally { closeSync(fd); }
}

function selection(value: Selection) {
  const clauses: string[] = [], args: (string | number)[] = [];
  const allowed = new Set(['sources', 'projects', 'threads', 'turns', 'roles', 'phases', 'since', 'until', 'includeUndated']);
  if (Object.keys(value).some(key => !allowed.has(key))) fail('INVALID_ARGUMENT');
  for (const [key, column, choices] of [
    ['sources', 'source', null], ['projects', 'project', null], ['threads', 'thread', null], ['turns', 'turn', null],
    ['roles', 'role', ['user', 'assistant']], ['phases', 'phase', ['request', 'commentary', 'final_answer']],
  ] as const) {
    const list = value[key];
    if (list === undefined) continue;
    if (!Array.isArray(list) || !list.length || list.length > 100 || list.some(x => typeof x !== 'string' || !x || x.length > 4096 || (choices && !(choices as readonly string[]).includes(x)))) fail('INVALID_ARGUMENT');
    clauses.push(`${column} IN (${list.map(() => '?').join(',')})`);
    args.push(...list);
  }
  if (value.includeUndated !== undefined && typeof value.includeUndated !== 'boolean') fail('INVALID_ARGUMENT');
  const dates: string[] = [], dateArgs: string[] = [];
  for (const [key, op] of [['since', '>='], ['until', '<']] as const) {
    const time = value[key];
    if (time === undefined) continue;
    // The prototype accepts canonical UTC instants only; production uses Rust timezone rules.
    if (typeof time !== 'string' || !Number.isFinite(Date.parse(time)) || new Date(time).toISOString() !== time) fail('INVALID_ARGUMENT');
    dates.push(`timestamp ${op} ?`); dateArgs.push(time);
  }
  if (value.since && value.until && value.since >= value.until) fail('INVALID_ARGUMENT');
  if (dates.length) {
    clauses.push(`(${dates.join(' AND ')}${value.includeUndated ? ' OR timestamp IS NULL' : ''})`);
    args.push(...dateArgs);
  }
  return { clauses, args, key: digest(JSON.stringify({ clauses, args })) };
}

export class ConversationPrototype {
  private readonly db: DatabaseSync;
  private readonly revision: string;
  readBytes = 0;
  constructor(database: string, create = false) {
    this.db = new DatabaseSync(database);
    this.db.exec('PRAGMA cache_size=-2048; PRAGMA temp_store=FILE;');
    if (create) {
      this.db.exec(`CREATE TABLE metadata(revision TEXT NOT NULL);
        CREATE TABLE files(id INTEGER PRIMARY KEY,path TEXT UNIQUE,physical TEXT,bytes INTEGER);
        CREATE TABLE messages(id INTEGER PRIMARY KEY,file INTEGER,sequence INTEGER,source TEXT,project TEXT,thread TEXT,turn TEXT,timestamp TEXT,role TEXT,phase TEXT,offset INTEGER,bytes INTEGER,hash TEXT);
        CREATE INDEX messages_thread ON messages(thread,id);
        CREATE INDEX messages_turn ON messages(thread,turn,id);
        CREATE INDEX messages_date ON messages(timestamp,id);
        CREATE INDEX messages_project ON messages(project,id);`);
      this.db.prepare('INSERT INTO metadata VALUES (?)').run(randomUUID());
    }
    this.revision = String(this.db.prepare('SELECT revision FROM metadata').get()!.revision);
  }
  close() { this.db.close(); }
  index(paths: string[], signal?: AbortSignal) {
    const file = this.db.prepare('INSERT INTO files(path,physical,bytes) VALUES (?,?,?)');
    const row = this.db.prepare('INSERT INTO messages(file,sequence,source,project,thread,turn,timestamp,role,phase,offset,bytes,hash) VALUES (?,?,?,?,?,?,?,?,?,?,?,?)');
    let count = 0;
    this.db.exec('BEGIN');
    try {
      for (const path of paths) {
        const before = statSync(path);
        if (!before.isFile()) fail('SOURCE_UNREADABLE');
        const id = Number(file.run(path, physical(before), before.size).lastInsertRowid);
        let sequence = 0;
        for (const record of records(path, signal)) {
          check(signal);
          const data = JSON.parse(record.bytes.toString('utf8')) as SyntheticRecord;
          // Source grammar here is fixture-only, independent of the product adapter.
          if (!data.source || !data.thread || !data.turn || !data.project || typeof data.text !== 'string' || !['user', 'assistant'].includes(data.role) || !['request', 'commentary', 'final_answer'].includes(data.phase)) fail('INVALID_FIXTURE');
          row.run(id, sequence++, data.source, data.project, data.thread, data.turn, data.timestamp, data.role, data.phase, record.offset, record.bytes.length, digest(record.bytes));
          count++;
        }
        const after = statSync(path);
        if (physical(after) !== physical(before) || after.size !== before.size || after.mtimeMs !== before.mtimeMs) fail('SOURCE_CHANGED');
      }
      this.db.exec('COMMIT');
      this.db.exec('ANALYZE');
      return count;
    } catch (error) { this.db.exec('ROLLBACK'); throw error; }
  }
  private cursor(key: string, after: number) { return JSON.stringify({ revision: this.revision, key, after }); }
  private after(key: string, cursor?: string): number {
    if (!cursor) return 0;
    if (cursor.length > 1024) fail('INVALID_ARGUMENT');
    let token: { revision: string; key: string; after: number };
    try { token = JSON.parse(cursor); } catch { return fail('INVALID_ARGUMENT'); }
    if (token.revision !== this.revision || token.key !== key) fail('CURSOR_MISMATCH');
    integer(token.after, Number.MAX_SAFE_INTEGER);
    return token.after;
  }
  list(filters: Selection, limit = 20, cursor?: string, signal?: AbortSignal): Page {
    check(signal); integer(limit, 200);
    const query = selection(filters), after = this.after(query.key, cursor);
    const rows = this.db.prepare(`SELECT id,file,sequence,source,thread,turn,timestamp,role,phase,offset,bytes,hash FROM messages WHERE ${[...query.clauses, 'id > ?'].join(' AND ')} ORDER BY id LIMIT ?`).all(...query.args, after, limit + 1).map(location);
    check(signal);
    return { locations: rows.slice(0, limit), cursor: rows.length > limit ? this.cursor(query.key, rows[limit - 1].id) : null };
  }
  private location(id: number) {
    integer(id, Number.MAX_SAFE_INTEGER);
    const row = this.db.prepare('SELECT id,file,sequence,source,thread,turn,timestamp,role,phase,offset,bytes,hash FROM messages WHERE id=?').get(id);
    if (!row) fail('NOT_FOUND');
    return location(row);
  }
  private text(row: Location, signal?: AbortSignal): string {
    check(signal);
    const stored = this.db.prepare('SELECT * FROM files WHERE id=?').get(row.file);
    if (!stored) fail('INVALID_INDEX');
    const source: FileRow = { id: Number(stored.id), path: String(stored.path), physical: String(stored.physical), bytes: Number(stored.bytes) };
    let fd: number;
    try { fd = openSync(source.path, 'r'); } catch { return fail('SOURCE_UNREADABLE'); }
    try {
      const before = fstatSync(fd);
      if (physical(before) !== source.physical || before.size < source.bytes) fail('SOURCE_CHANGED');
      const bytes = Buffer.alloc(row.bytes);
      let offset = 0;
      while (offset < bytes.length) {
        check(signal);
        const n = readSync(fd, bytes, offset, bytes.length - offset, row.offset + offset);
        this.readBytes += n;
        if (!n) fail('SOURCE_CHANGED');
        offset += n;
      }
      const after = fstatSync(fd), current = statSync(source.path);
      if (physical(after) !== physical(before) || physical(current) !== physical(before) || after.size !== before.size || after.mtimeMs !== before.mtimeMs || digest(bytes) !== row.hash) fail('SOURCE_CHANGED');
      check(signal);
      const data = JSON.parse(bytes.toString('utf8')) as SyntheticRecord;
      return data.text;
    } finally { closeSync(fd); }
  }
  read(id: number, options: ReadOptions = {}): ReadPage {
    const { offset = 0, maxBytes = 4096, before = 0, after = 0, contextScope = 'turn', signal } = options;
    integer(offset, RECORD_BYTES, true); integer(maxBytes, 256 * 1024); integer(before, 20, true); integer(after, 20, true);
    if (!['turn', 'thread'].includes(contextScope)) fail('INVALID_ARGUMENT');
    const row = this.location(id), start = this.readBytes, text = this.text(row, signal);
    // Offset is a UTF-16 string offset, bound to the same indexed record hash.
    if (offset > text.length || (offset > 0 && /[\uDC00-\uDFFF]/u.test(text[offset]))) fail('INVALID_ARGUMENT');
    let lo = offset, hi = Math.min(text.length, offset + maxBytes);
    while (lo < hi) {
      const mid = Math.ceil((lo + hi) / 2);
      if (Buffer.byteLength(text.slice(offset, mid)) <= maxBytes) lo = mid;
      else hi = mid - 1;
    }
    let end = lo;
    if (end < text.length && end > offset && /[\uD800-\uDBFF]/u.test(text[end - 1])) end--;
    if (end === offset && offset < text.length) fail('RESOURCE_LIMIT');
    const clause = contextScope === 'turn' ? ' AND turn=?' : '', args = contextScope === 'turn' ? [row.source, row.thread, row.turn] : [row.source, row.thread];
    const context = (direction: '<' | '>', count: number) => this.db.prepare(`SELECT id,file,sequence,source,thread,turn,timestamp,role,phase,offset,bytes,hash FROM messages WHERE source=? AND thread=?${clause} AND id ${direction} ? ORDER BY id ${direction === '<' ? 'DESC' : 'ASC'} LIMIT ?`).all(...args, row.id, count).map(location);
    check(signal);
    return { text: text.slice(offset, end), nextOffset: end < text.length ? end : null, readBytes: this.readBytes - start, context: [...context('<', before).reverse(), ...context('>', after)] };
  }
  readMany(targets: { id: number; options?: ReadOptions }[], maxResponseBytes = 256 * 1024) {
    integer(targets.length, 20); integer(maxResponseBytes, 1024 * 1024);
    const start = this.readBytes, items: ({ id: number } & ReadPage)[] = [];
    for (const target of targets) {
      items.push({ id: target.id, ...this.read(target.id, target.options) });
      if (Buffer.byteLength(JSON.stringify({ items, readBytes: this.readBytes - start })) > maxResponseBytes) fail('RESOURCE_LIMIT');
    }
    return { items, readBytes: this.readBytes - start };
  }
  search(filters: Selection, needle: string, options: { limit?: number; maxScanBytes?: number; cursor?: string; signal?: AbortSignal } = {}): SearchPage {
    if (typeof needle !== 'string' || !needle || Buffer.byteLength(needle) > 4096 || (!filters.threads?.length && !filters.projects?.length)) fail('INVALID_ARGUMENT');
    const { limit = 10, maxScanBytes = 8 * 1024 * 1024, signal } = options;
    integer(limit, 200); integer(maxScanBytes, 32 * 1024 * 1024);
    const query = selection(filters), key = digest(query.key + needle), start = this.readBytes;
    let after = this.after(key, options.cursor), scannedRecords = 0;
    const locations: Location[] = [];
    while (true) {
      check(signal);
      const page = this.list(filters, 64, after ? this.cursor(query.key, after) : undefined, signal);
      if (!page.locations.length) return { locations, cursor: null, scannedRecords, readBytes: this.readBytes - start, complete: true };
      for (const row of page.locations) {
        if (row.bytes > maxScanBytes - (this.readBytes - start)) {
          if (!scannedRecords) fail('RESOURCE_LIMIT');
          return { locations, cursor: this.cursor(key, after), scannedRecords, readBytes: this.readBytes - start, complete: false };
        }
        const body = this.text(row, signal);
        after = row.id; scannedRecords++;
        if (body.includes(needle)) locations.push(row);
        if (locations.length === limit) return { locations, cursor: this.cursor(key, after), scannedRecords, readBytes: this.readBytes - start, complete: false };
      }
      if (!page.cursor) return { locations, cursor: null, scannedRecords, readBytes: this.readBytes - start, complete: true };
    }
  }
}
