import assert from 'node:assert/strict';
import { appendFileSync, mkdtempSync, readFileSync, renameSync, rmSync, statSync, unlinkSync, utimesSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { test } from 'node:test';
import { ConversationPrototype, records, type SyntheticRecord } from './conversation-access-prototype.js';

const record = (text: string, extras: Partial<SyntheticRecord> = {}): SyntheticRecord => ({ source: 'synthetic', project: '/synthetic/project', thread: 'a', turn: '1', timestamp: '2026-10-14T00:00:00.000Z', role: 'assistant', phase: 'final_answer', text, ...extras });
function fixture(rows: SyntheticRecord[], action: (p: ConversationPrototype, source: string, db: string) => void, tail = '') {
  const root = mkdtempSync(path.join(os.tmpdir(), 'wombat-conversation-test-'));
  const source = path.join(root, 'synthetic.jsonl'), db = path.join(root, 'index.sqlite');
  writeFileSync(source, rows.map(x => JSON.stringify(x) + '\n').join('') + tail);
  const prototype = new ConversationPrototype(db, true);
  try { assert.equal(prototype.index([source]), rows.length); action(prototype, source, db); }
  finally { prototype.close(); rmSync(root, { recursive: true, force: true }); }
}

test('composes dates, projects, turns, roles and phases without usage records', () => {
  fixture([record('request', { role: 'user', phase: 'request' }), record('answer'), record('other', { project: '/other', thread: 'b' }), record('undated', { timestamp: null }), record('tomorrow', { timestamp: '2026-10-15T00:00:00.000Z' })], p => {
    const filters = { projects: ['/synthetic/project'], threads: ['a'], turns: ['1'], roles: ['assistant'] as const, phases: ['final_answer'] as const, since: '2026-10-14T00:00:00.000Z', until: '2026-10-15T00:00:00.000Z' };
    const selected = p.list({ ...filters, roles: [...filters.roles], phases: [...filters.phases] });
    assert.deepEqual(selected.locations.map(x => x.id), [2]);
    assert.equal(p.read(2).text, 'answer');
    assert.deepEqual(p.list({ ...filters, roles: [...filters.roles], phases: [...filters.phases], includeUndated: true }).locations.map(x => x.id), [2, 4]);
    assert.equal(p.list({}).locations.length, 5);
  });
});
test('pagination binds every selection condition and survives index reopen', () => {
  fixture([record('one'), record('two'), record('three')], (p, _source, db) => {
    const first = p.list({ threads: ['a'] }, 1);
    assert(first.cursor);
    assert.deepEqual(p.list({ threads: ['a'] }, 1, first.cursor).locations.map(x => x.id), [2]);
    assert.throws(() => p.list({ projects: ['/synthetic/project'] }, 1, first.cursor!), /CURSOR_MISMATCH/);
    const reopened = new ConversationPrototype(db);
    try { assert.equal(reopened.list({ threads: ['a'] }, 20, first.cursor).locations.length, 2); }
    finally { reopened.close(); }
  });
});
test('context stays in selected turn unless explicitly expanded to thread', () => {
  fixture([record('one'), record('two'), record('three', { turn: '2' }), record('unrelated', { thread: 'b' })], p => {
    assert.deepEqual(p.read(2, { before: 2, after: 2 }).context.map(x => x.id), [1]);
    assert.deepEqual(p.read(2, { before: 2, after: 2, contextScope: 'thread' }).context.map(x => x.id), [1, 3]);
  });
});
test('source selection and context never merge identical foreign task identities', () => {
  fixture([record('one'), record('other source', { source: 'other' }), record('two')], p => {
    assert.deepEqual(p.list({ sources: ['other'], threads: ['a'] }).locations.map(x => x.id), [2]);
    assert.deepEqual(p.read(1, { after: 10, contextScope: 'thread' }).context.map(x => x.id), [3]);
  });
});
test('batch reads accept different per-target options and bound the full JSON envelope', () => {
  fixture([record('中文🙂'), record('second')], p => {
    const result = p.readMany([{ id: 1, options: { maxBytes: 6 } }, { id: 2, options: { offset: 2, maxBytes: 3 } }]);
    assert.deepEqual(result.items.map(x => x.text), ['中文', 'con']);
    assert.equal(result.items[0].nextOffset, 2);
    const bytes = Buffer.byteLength(JSON.stringify(result));
    assert.equal(Buffer.byteLength(JSON.stringify(p.readMany([{ id: 1, options: { maxBytes: 6 } }, { id: 2, options: { offset: 2, maxBytes: 3 } }], bytes))), bytes);
    assert.throws(() => p.readMany([{ id: 1, options: { maxBytes: 6 } }, { id: 2, options: { offset: 2, maxBytes: 3 } }], bytes - 1), /RESOURCE_LIMIT/);
    assert.throws(() => p.readMany([]), /INVALID_ARGUMENT/);
  });
});
test('bounded Unicode reads reconstruct exact text and respect byte envelopes', () => {
  const expected = '中文🙂é /PRIVATE_SYNTHETIC_SENTINEL/'.repeat(200);
  fixture([record(expected)], p => {
    let offset = 0, reconstructed = '';
    do {
      const result = p.read(1, { offset, maxBytes: 13 });
      assert(Buffer.byteLength(result.text) <= 13);
      reconstructed += result.text;
      if (result.nextOffset === null) break;
      assert(result.nextOffset > offset); offset = result.nextOffset;
    } while (true);
    assert.equal(reconstructed, expected);
    assert.throws(() => p.read(1, { maxBytes: 1 }), /RESOURCE_LIMIT/);
  });
});
test('keyword search has composable filters, bounded scan and exact continuation', () => {
  fixture([record('needle first'), record('absent'), record('needle 中文'), record('needle excluded', { thread: 'b' })], p => {
    const first = p.search({ threads: ['a'] }, 'needle', { limit: 1 });
    assert.deepEqual(first.locations.map(x => x.id), [1]); assert.equal(first.complete, false);
    const second = p.search({ threads: ['a'] }, 'needle', { cursor: first.cursor! });
    assert.deepEqual(second.locations.map(x => x.id), [3]); assert.equal(second.complete, true);
    assert.throws(() => p.search({ threads: ['a'] }, '中文', { cursor: first.cursor! }), /CURSOR_MISMATCH/);
    const limited = p.search({ threads: ['a'] }, 'missing', { maxScanBytes: first.readBytes });
    assert.equal(limited.scannedRecords, 1); assert.equal(limited.complete, false); assert(limited.cursor);
    assert.throws(() => p.search({ threads: ['a'] }, 'needle', { maxScanBytes: 1 }), /RESOURCE_LIMIT/);
  });
});
test('metadata listing reads no original content and persists no plaintext', () => {
  fixture([record('PRIVATE_SYNTHETIC_SENTINEL')], (p, _source, db) => {
    p.readBytes = 0;
    assert.equal(p.list({}).locations.length, 1); assert.equal(p.readBytes, 0);
    assert(!readFileSync(db).includes(Buffer.from('PRIVATE_SYNTHETIC_SENTINEL')));
  });
});
test('append preserves indexed reads and excludes unindexed records and open tails', () => {
  fixture([record('original')], (p, source) => {
    appendFileSync(source, JSON.stringify(record('later')) + '\n');
    assert.equal(p.read(1).text, 'original'); assert.equal(p.list({}).locations.length, 1);
  }, JSON.stringify(record('open')));
});
test('truncation and missing sources produce explicit failures', () => {
  fixture([record('original')], (p, source) => {
    writeFileSync(source, ''); assert.throws(() => p.read(1), /SOURCE_CHANGED/);
    unlinkSync(source); assert.throws(() => p.read(1), /SOURCE_UNREADABLE/);
    assert.equal(p.list({}).locations.length, 1);
  });
});
test('same-size replacement with preserved timestamps cannot rebind a location', () => {
  fixture([record('original')], (p, source) => {
    const before = statSync(source), replacement = source + '.replacement';
    writeFileSync(replacement, readFileSync(source)); utimesSync(replacement, before.atime, before.mtime);
    renameSync(replacement, source);
    assert.throws(() => p.read(1), /SOURCE_CHANGED/);
  });
});
test('same-file rewrite with restored timestamp is caught by record fingerprint', () => {
  fixture([record('original')], (p, source) => {
    const before = statSync(source);
    writeFileSync(source, JSON.stringify(record('modified')) + '\n');
    utimesSync(source, before.atime, before.mtime);
    assert.throws(() => p.read(1), /SOURCE_CHANGED/);
  });
});
test('large records have bounded chunks and oversized records fail indexing atomically', () => {
  fixture([record('中'.repeat(50_000))], p => {
    const first = p.read(1, { maxBytes: 4096 }); assert(first.nextOffset);
    assert(Buffer.byteLength(first.text) <= 4096);
  });
  const root = mkdtempSync(path.join(os.tmpdir(), 'wombat-conversation-large-'));
  const source = path.join(root, 'source'), p = new ConversationPrototype(path.join(root, 'db'), true);
  try {
    writeFileSync(source, JSON.stringify(record('x'.repeat(4 * 1024 * 1024))) + '\n');
    assert.throws(() => p.index([source]), /RESOURCE_LIMIT/); assert.equal(p.list({}).locations.length, 0);
  } finally { p.close(); rmSync(root, { recursive: true, force: true }); }
});
test('byte locations survive UTF-8 chunk splits, CRLF and an unfinished trailing record', () => {
  const root = mkdtempSync(path.join(os.tmpdir(), 'wombat-conversation-bytes-'));
  const source = path.join(root, 'source');
  const first = JSON.stringify(record('x'.repeat(60_000) + '中文🙂'.repeat(5000))) + '\r\n';
  const second = JSON.stringify(record('second')) + '\r\n';
  try {
    writeFileSync(source, first + second + '{"unfinished":');
    const rows = [...records(source)];
    assert.equal(rows.length, 2);
    assert.equal(rows[1].offset, Buffer.byteLength(first));
    assert.equal(rows[0].bytes.toString('utf8'), first);
    assert.equal(rows[1].bytes.toString('utf8'), second);
  } finally { rmSync(root, { recursive: true, force: true }); }
});
test('invalid inputs, arbitrary path fields, cross-index cursors and cancellation fail', () => {
  fixture([record('one'), record('two')], p => {
    assert.throws(() => p.list({ threads: [] }), /INVALID_ARGUMENT/);
    assert.throws(() => p.list({ until: 'tomorrow' }), /INVALID_ARGUMENT/);
    assert.throws(() => p.list({ since: '2026-10-15T00:00:00.000Z', until: '2026-10-14T00:00:00.000Z' }), /INVALID_ARGUMENT/);
    assert.throws(() => p.list(JSON.parse('{"path":"/etc/passwd"}')), /INVALID_ARGUMENT/);
    assert.throws(() => p.list({}, 0), /INVALID_ARGUMENT/);
    assert.throws(() => p.read(999), /NOT_FOUND/);
    assert.throws(() => p.read(1, { offset: 100 }), /INVALID_ARGUMENT/);
    assert.throws(() => p.search({}, 'one'), /INVALID_ARGUMENT/);
    const signal = AbortSignal.abort();
    assert.throws(() => p.list({}, 20, undefined, signal), /CANCELLED/);
    assert.throws(() => p.read(1, { signal }), /CANCELLED/);
    assert.throws(() => p.search({ threads: ['a'] }, 'one', { signal }), /CANCELLED/);
    const cursor = p.list({}, 1).cursor!, changed = JSON.parse(cursor); changed.revision = 'other';
    assert.throws(() => p.list({}, 1, JSON.stringify(changed)), /CURSOR_MISMATCH/);
  });
});
