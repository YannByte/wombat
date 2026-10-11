import test from 'node:test';
import assert from 'node:assert/strict';
import { DatabaseSync } from 'node:sqlite';
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import {
  directoryBytes,
  isExpectedSyncTimeout,
  liveIndexPayloadFootprint,
  normalizeUsageOracle,
  parseBenchmarkCliResponse,
  snapshotFootprint,
  sourceTreeIdentity,
} from './benchmark-live-helpers.js';

test('source identity frames sorted relative names and exact bytes; directory size counts logical bytes', () => {
  const root = mkdtempSync(path.join(os.tmpdir(), 'wombat-live-helper-test-'));
  const first = path.join(root, 'first'); const second = path.join(root, 'second');
  try {
    mkdirSync(path.join(first, 'sessions'), { recursive: true });
    mkdirSync(path.join(second, 'sessions'), { recursive: true });
    writeFileSync(path.join(first, 'sessions', 'b.jsonl'), 'second\n');
    writeFileSync(path.join(first, 'sessions', 'a.jsonl'), 'first\n');
    writeFileSync(path.join(second, 'sessions', 'a.jsonl'), 'first\n');
    writeFileSync(path.join(second, 'sessions', 'b.jsonl'), 'second\n');
    const before = sourceTreeIdentity(first);
    assert.deepEqual(before, sourceTreeIdentity(second));
    assert.equal(before.files, 2);
    assert.equal(before.bytes, Buffer.byteLength('first\nsecond\n'));
    assert.equal(directoryBytes(first), before.bytes);
    writeFileSync(path.join(second, 'sessions', 'b.jsonl'), 'changed\n');
    assert.notEqual(sourceTreeIdentity(second).sha256, before.sha256);
    assert.equal(sourceTreeIdentity(second).bytes, before.bytes + 1);
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test('oracle normalization excludes only changing view headers and retains amounts, page totals and event time', () => {
  const base = {
    outputVersion: 5,
    action: 'usage',
    snapshotRef: { snapshotId: 'live:first', createdAt: '2026-10-01T00:00:00Z' },
    freshness: { status: 'current', revision: 'one', checkedAt: '2026-10-01T00:00:00Z' },
    capturedAt: '2026-10-01T00:00:00Z',
    scope: { allTime: true, timezone: 'UTC' },
    availableRange: { since: '2026-09-29', until: '2026-09-30' },
    summary: { measurementCount: 1, tokens: { total: 110 }, price: { knownCost: '0.000265', cost: '0.000265' } },
    items: [{ kind: 'usage', date: '2026-09-29', isSubtotal: true, timestamp: '2026-09-29T00:00:01Z', usage: { tokens: { total: 110 }, price: { knownCost: '0.000265' } } }],
    page: { offset: 0, limit: 100, total: 1, nextOffset: null },
    quality: { status: 'complete', sources: [{ source: { id: 'codex:synthetic', root: '/synthetic' }, adapterVersion: '1', sourceVersions: ['events:1'], capabilities: { events: true }, status: 'complete', filesRead: 1, bytesRead: 123, issues: [] }] },
    facets: { discoveredThreadCount: 1 },
    distribution: { peakTokenDates: ['2026-09-29'] },
    priceUpdate: { attemptedAt: '2026-10-01T00:00:00Z' },
  };
  const changedHeaders = {
    ...base,
    snapshotRef: { snapshotId: 'live:second', createdAt: '2026-10-02T00:00:00Z' },
    freshness: { status: 'current', revision: 'two', checkedAt: '2026-10-02T00:00:00Z' },
    capturedAt: '2026-10-02T00:00:00Z',
    priceUpdate: { attemptedAt: '2026-10-02T00:00:00Z' },
    quality: { ...base.quality, sources: [{ ...base.quality.sources[0], filesRead: 0, bytesRead: 0 }] },
  };
  assert.deepEqual(normalizeUsageOracle(base), normalizeUsageOracle(changedHeaders));
  assert.notDeepEqual(normalizeUsageOracle(base), normalizeUsageOracle({
    ...changedHeaders,
    quality: { ...base.quality, sources: [{ ...base.quality.sources[0], status: 'partial' }] },
  }));
  assert.notDeepEqual(normalizeUsageOracle(base), normalizeUsageOracle({
    ...changedHeaders,
    quality: { ...base.quality, sources: [{ ...base.quality.sources[0], issues: [{ code: 'incompleteTail' }] }] },
  }));
  assert.notDeepEqual(normalizeUsageOracle(base), normalizeUsageOracle({
    ...changedHeaders,
    summary: { ...base.summary, price: { ...base.summary.price, knownCost: '0.000530' } },
  }));
  assert.notDeepEqual(normalizeUsageOracle(base), normalizeUsageOracle({
    ...changedHeaders,
    items: [{ ...base.items[0], timestamp: '2026-09-29T00:00:02Z' }],
  }));
  assert.notDeepEqual(normalizeUsageOracle(base), normalizeUsageOracle({
    ...changedHeaders,
    page: { ...base.page, total: 2 },
  }));
});

test('only a structured SYNC_TIMEOUT is eligible for fresh-query retry', () => {
  assert.equal(isExpectedSyncTimeout(2, { error: { code: 'SYNC_TIMEOUT' } }), true);
  assert.equal(isExpectedSyncTimeout(0, { error: { code: 'SYNC_TIMEOUT' } }), true);
  assert.equal(isExpectedSyncTimeout(2, { error: { code: 'SOURCE_UNREADABLE' } }), false);
  assert.equal(isExpectedSyncTimeout(2, 'SYNC_TIMEOUT'), false);
  assert.equal(isExpectedSyncTimeout(null, { error: { code: 'SYNC_TIMEOUT' } }), false);
});

test('fixed snapshot footprint separates event, ledger, thread and manifest logical file lengths', () => {
  const root = mkdtempSync(path.join(os.tmpdir(), 'wombat-snapshot-helper-test-'));
  try {
    writeFileSync(path.join(root, 'manifest.json'), JSON.stringify({
      ledger: { file: 'ledger.json' },
      events: { partitions: [{ chunks: [{ file: { file: 'events-0.json' } }] }] },
      threads: [{ file: { file: 'thread-0.jsonl' } }],
    }));
    writeFileSync(path.join(root, 'ledger.json'), 'ledger');
    writeFileSync(path.join(root, 'events-0.json'), 'events');
    writeFileSync(path.join(root, 'thread-0.jsonl'), 'thread');
    writeFileSync(path.join(root, 'other.bin'), 'other');
    assert.deepEqual(snapshotFootprint(root), {
      totalBytes: directoryBytes(root),
      manifestBytes: Buffer.byteLength(JSON.stringify({ ledger: { file: 'ledger.json' }, events: { partitions: [{ chunks: [{ file: { file: 'events-0.json' } }] }] }, threads: [{ file: { file: 'thread-0.jsonl' } }] })),
      ledgerBytes: 6,
      eventShardBytes: 6,
      threadShardBytes: 6,
      otherBytes: 5,
    });
  } finally {
    rmSync(root, { recursive: true, force: true });
  }
});

test('SQLite logical payload report distinguishes owned JSONB bytes from projection expansion', () => {
  const root = mkdtempSync(path.join(os.tmpdir(), 'wombat-index-helper-test-'));
  const databaseFile = path.join(root, 'index.sqlite');
  const database = new DatabaseSync(databaseFile);
  try {
    database.exec(`
      CREATE TABLE buckets (id INTEGER PRIMARY KEY, scope TEXT NOT NULL, field TEXT NOT NULL, UNIQUE(scope,field));
      CREATE TABLE entries (bucket INTEGER NOT NULL, id TEXT NOT NULL, payload BLOB, source_bucket INTEGER, member TEXT, PRIMARY KEY(bucket,id));
      INSERT INTO buckets VALUES (1,'parser:synthetic:1:1:facts','events');
      INSERT INTO buckets VALUES (2,'parser:synthetic:1:1:facts','measurements');
      INSERT INTO buckets VALUES (3,'parser:synthetic:1:1:facts','operations');
      INSERT INTO buckets VALUES (4,'projection:synthetic','events');
      INSERT INTO buckets VALUES (5,'projection:synthetic','operations');
      INSERT INTO buckets VALUES (6,'projection:synthetic','measurements');
    `);
    database.prepare('INSERT INTO entries(bucket,id,payload) VALUES(?,?,jsonb(?))').run(1, 'event-1', '{"kind":"message","text":"合成"}');
    database.prepare('INSERT INTO entries(bucket,id,payload) VALUES(?,?,jsonb(?))').run(2, 'measurement-1', '{"measurement":{"timestamp":"2026-10-01T00:00:01Z","tokens":{"total":7}}}');
    database.prepare('INSERT INTO entries(bucket,id,payload) VALUES(?,?,jsonb(?))').run(3, 'operation-1', '{"name":"read_file","callId":"synthetic"}');
    database.prepare('INSERT INTO entries(bucket,id,source_bucket,member) VALUES(?,?,?,?)').run(4, 'event-1', 1, '$');
    database.prepare('INSERT INTO entries(bucket,id,source_bucket,member) VALUES(?,?,?,?)').run(5, 'operation-1', 3, '$');
    database.prepare('INSERT INTO entries(bucket,id,source_bucket,member) VALUES(?,?,?,?)').run(6, 'measurement-1', 2, '$.measurement');
    const expectedStored = Object.fromEntries([1, 2, 3].map(bucket => [bucket, Number(database.prepare('SELECT length(payload) AS n FROM entries WHERE bucket=?').get(bucket)!.n)]));
    database.close();

    const footprint = liveIndexPayloadFootprint(databaseFile);
    assert.deepEqual(footprint, [
      { scopeKind: 'parserFacts', field: 'events', entries: 1, storedJsonbPayloadBytes: expectedStored[1], expandedJsonBytes: Buffer.byteLength('{"kind":"message","text":"合成"}') },
      { scopeKind: 'parserFacts', field: 'measurements', entries: 1, storedJsonbPayloadBytes: expectedStored[2], expandedJsonBytes: Buffer.byteLength('{"measurement":{"timestamp":"2026-10-01T00:00:01Z","tokens":{"total":7}}}') },
      { scopeKind: 'parserFacts', field: 'operations', entries: 1, storedJsonbPayloadBytes: expectedStored[3], expandedJsonBytes: Buffer.byteLength('{"name":"read_file","callId":"synthetic"}') },
      { scopeKind: 'projection', field: 'events', entries: 1, storedJsonbPayloadBytes: 0, expandedJsonBytes: Buffer.byteLength('{"kind":"message","text":"合成"}') },
      { scopeKind: 'projection', field: 'measurements', entries: 1, storedJsonbPayloadBytes: 0, expandedJsonBytes: Buffer.byteLength('{"timestamp":"2026-10-01T00:00:01Z","tokens":{"total":7}}') },
      { scopeKind: 'projection', field: 'operations', entries: 1, storedJsonbPayloadBytes: 0, expandedJsonBytes: Buffer.byteLength('{"name":"read_file","callId":"synthetic"}') },
    ]);
  } finally {
    try { database.close(); } catch { /* already closed before the read-only helper check */ }
    rmSync(root, { recursive: true, force: true });
  }
});


test('benchmark failures retain the bounded CLI code and message, without dumping source rows or details',()=>{
 const stdout=JSON.stringify({outputVersion:3,error:{code:'RESOURCE_LIMIT',message:'Synthetic snapshot limit',details:{source:'SYNTHETIC_PRIVATE_DETAIL'}},rows:[{text:'SYNTHETIC_PRIVATE_BODY'}]});
 assert.throws(()=>parseBenchmarkCliResponse('refresh --json',{status:1,stdout,stderr:'progress'}),(error:unknown)=>{
  assert.ok(error instanceof Error);assert.match(error.message,/RESOURCE_LIMIT/);assert.match(error.message,/Synthetic snapshot limit/);assert.match(error.message,/"status":1/);
  assert.doesNotMatch(error.message,/SYNTHETIC_PRIVATE_DETAIL|SYNTHETIC_PRIVATE_BODY|rows/);return true;
 });
 const unsafe='\u001b[31m'+('x'.repeat(10000))+'\u001b[0m\n';
 assert.throws(()=>parseBenchmarkCliResponse(unsafe,{status:1,stdout:JSON.stringify({error:{code:unsafe,message:unsafe}}),stderr:unsafe}),(error:unknown)=>{
  assert.ok(error instanceof Error);assert.ok(error.message.length<5000);assert.doesNotMatch(error.message,/\u001b|\n/);return true;
 });
});

test('benchmark parser accepts success and only structured sync timeouts; process and protocol failures stay failures',()=>{
 const success={summary:{tokens:{total:0}}};
 assert.deepEqual(parseBenchmarkCliResponse('usage',{status:0,stdout:JSON.stringify(success),stderr:''}),success);
 for(const status of [0,2]){
  const timeout={error:{code:'SYNC_TIMEOUT',message:'pending'}};
  assert.deepEqual(parseBenchmarkCliResponse('usage',{status,stdout:JSON.stringify(timeout),stderr:''}),timeout);
 }
 for(const status of [0,1])assert.throws(()=>parseBenchmarkCliResponse('usage',{status,stdout:JSON.stringify({error:{code:'SOURCE_UNREADABLE',message:'read failed'}}),stderr:''}),/SOURCE_UNREADABLE/);
 for(const stdout of ['', 'not JSON', 'null', '[]', '42'])assert.throws(()=>parseBenchmarkCliResponse('usage',{status:0,stdout,stderr:''}),/invalid JSON|expected JSON object/);
 assert.throws(()=>parseBenchmarkCliResponse('usage',{status:null,signal:'SIGTERM',stdout:'',stderr:''}),/SIGTERM/);
 assert.throws(()=>parseBenchmarkCliResponse('usage',{status:null,stdout:null,stderr:null,error:{code:'ETIMEDOUT',message:'process timed out'}}),/ETIMEDOUT/);
 assert.throws(()=>parseBenchmarkCliResponse('usage',{status:2,stdout:JSON.stringify({error:'SYNC_TIMEOUT'}),stderr:''}));
 assert.throws(()=>parseBenchmarkCliResponse('usage',{status:null,signal:'SIGKILL',stdout:JSON.stringify({error:{code:'SYNC_TIMEOUT'}}),stderr:''}),/SIGKILL/);
});

test('benchmark accepts documented project batches but rejects unrelated partial failures',()=>{
 const partial={quality:{status:'partial'},freshness:{status:'syncing',projectLoads:[{project:'/synthetic/a',state:'ready'},{project:'/synthetic/b',state:'loading'}]}};
 const parse=(value:unknown)=>parseBenchmarkCliResponse('cached project',{status:2,stdout:JSON.stringify(value),stderr:''});
 assert.deepEqual(parse(partial),partial);
 for(const value of [
  {...partial,error:{code:'INDEX_UNAVAILABLE',message:'failed'}},
  {...partial,freshness:{...partial.freshness,status:'failed'}},
  {...partial,freshness:{...partial.freshness,projectLoads:[]}},
  {...partial,freshness:{...partial.freshness,projectLoads:[{project:null,state:'unexpected'}]}},
  {...partial,quality:{status:'complete'}},
 ])assert.throws(()=>parse(value));
});

test('only restore polling accepts bounded SYNC_PENDING responses',()=>{
 const response={status:1,stdout:JSON.stringify({error:{code:'SYNC_PENDING',message:'loading'}}),stderr:''};
 assert.throws(()=>parseBenchmarkCliResponse('usage',response));
 assert.equal((parseBenchmarkCliResponse('restore',response,true).error as {code:string}).code,'SYNC_PENDING');
 for(const failure of [
  {...response,signal:'SIGKILL'},
  {...response,error:{code:'ETIMEDOUT',message:'process timed out'}},
  {...response,stdout:JSON.stringify({error:{code:'INDEX_UNAVAILABLE'}})},
 ])assert.throws(()=>parseBenchmarkCliResponse('restore',failure,true));
});

test('compressed index report counts physical frames once and preserves fact counts', () => {
  const root=mkdtempSync(path.join(os.tmpdir(),'wombat-packed-footprint-'));
  const file=path.join(root,'index.sqlite');const db=new DatabaseSync(file);
  try {
    db.exec(`PRAGMA user_version=6;
      CREATE TABLE buckets(id INTEGER PRIMARY KEY,scope TEXT,field TEXT);
      CREATE TABLE event_blocks(bucket INTEGER,count INTEGER,data BLOB,bytes INTEGER);
      CREATE TABLE fact_blocks(bucket INTEGER,data BLOB,bytes INTEGER);
      CREATE TABLE fact_rows(bucket INTEGER);
      INSERT INTO buckets VALUES(1,'parser:synthetic:1:facts','events'),(2,'projection:synthetic','events'),(3,'projection:synthetic','measurements'),(4,'view:synthetic:evidence','partitions');
      INSERT INTO event_blocks VALUES(1,200,zeroblob(20),2000);
      INSERT INTO fact_blocks VALUES(3,zeroblob(10),1000);
      INSERT INTO fact_rows VALUES(3),(3);
      INSERT INTO fact_blocks VALUES(4,zeroblob(12),1200);
      INSERT INTO fact_rows VALUES(4);`);
    assert.deepEqual(liveIndexPayloadFootprint(file),[
      {scopeKind:'parserFacts',field:'events',entries:200,storedJsonbPayloadBytes:0,expandedJsonBytes:0,storedCompressedPayloadBytes:20,decodedFrameBytes:2000},
      {scopeKind:'projection',field:'measurements',entries:2,storedJsonbPayloadBytes:0,expandedJsonBytes:0,storedCompressedPayloadBytes:10,decodedFrameBytes:1000},
      {scopeKind:'queryEvidence',field:'partitions',entries:1,storedJsonbPayloadBytes:0,expandedJsonBytes:0,storedCompressedPayloadBytes:12,decodedFrameBytes:1200},
    ]);
  } finally {db.close();rmSync(root,{recursive:true,force:true});}
});
