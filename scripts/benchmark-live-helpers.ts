import { createHash } from 'node:crypto';
import { stripVTControlCharacters } from 'node:util';
import { DatabaseSync } from 'node:sqlite';
import { lstatSync, readFileSync, readdirSync, statSync } from 'node:fs';
import path from 'node:path';

export interface TreeIdentity {
  files: number;
  bytes: number;
  sha256: string;
}

/** A structured SYNC_TIMEOUT may be reported with a nonzero process status; no other failure is retryable. */
export function isExpectedSyncTimeout(status: number | null, value: unknown): boolean {
  return status !== null && typeof value === 'object' && value !== null
    && 'error' in value && typeof value.error === 'object' && value.error !== null
    && 'code' in value.error && value.error.code === 'SYNC_TIMEOUT';
}

interface BenchmarkCliResponse {
  status: number | null;
  signal?: string | null;
  stdout: string | null | undefined;
  stderr: string | null | undefined;
  error?: { code?: string; message: string };
}

function diagnosticText(value: unknown, maximum: number): string | undefined {
  return typeof value === 'string'
    ? stripVTControlCharacters(value).replace(/[\u0000-\u001f\u007f]/g, ' ').slice(0, maximum)
    : undefined;
}

/** Keep the CLI error envelope when failing; never dump successful rows or arbitrary details. */
export function parseBenchmarkCliResponse(operation: string, response: BenchmarkCliResponse, allowRestorePending = false): Record<string, unknown> {
  const stdout = typeof response.stdout === 'string' ? response.stdout : '';
  let value: unknown;
  let parseFailure = false;
  try { value = JSON.parse(stdout); } catch { parseFailure = true; }
  const object = typeof value === 'object' && value !== null && !Array.isArray(value) ? value as Record<string, unknown> : undefined;
  const error = object?.error;
  const structuredError = typeof error === 'object' && error !== null ? error as Record<string, unknown> : undefined;
  const expectedTimeout = !response.error && !response.signal && isExpectedSyncTimeout(response.status, value);
  if (expectedTimeout && object) return object;
  if (allowRestorePending && !response.error && !response.signal && response.status === 1
    && structuredError?.code === 'SYNC_PENDING' && object) return object;
  // Exit 2 is a successful partial view only for the documented project restore state.
  const freshness = object?.freshness;
  const quality = object?.quality;
  if (!response.error && !response.signal && response.status === 2 && object && error === undefined
    && typeof freshness === 'object' && freshness !== null && 'status' in freshness && freshness.status === 'syncing'
    && 'projectLoads' in freshness && Array.isArray(freshness.projectLoads) && freshness.projectLoads.length > 0
    && freshness.projectLoads.every(p => typeof p === 'object' && p !== null && (p.project === null || typeof p.project === 'string') && ['ready','loading','pending'].includes(p.state))
    && freshness.projectLoads.some(p => p.state !== 'ready')
    && typeof quality === 'object' && quality !== null && 'status' in quality && quality.status === 'partial') return object;
  if (response.error || response.signal || response.status !== 0 || !object || error !== undefined) {
    const diagnostic = {
      status: response.status,
      signal: response.signal ?? null,
      code: diagnosticText(structuredError?.code, 128),
      message: diagnosticText(structuredError?.message, 1024),
      processCode: diagnosticText(response.error?.code, 128),
      processMessage: diagnosticText(response.error?.message, 1024),
      protocol: parseFailure ? 'invalid JSON' : !object ? 'expected JSON object' : undefined,
      stdoutBytes: Buffer.byteLength(stdout),
      stderr: diagnosticText(response.stderr, 2048),
    };
    throw new Error(`${diagnosticText(operation, 512)} failed: ${JSON.stringify(diagnostic)}`);
  }
  return object;
}

function filesUnder(directory: string): string[] {
  return readdirSync(directory, { withFileTypes: true }).flatMap(entry => {
    const file = path.join(directory, entry.name);
    if (entry.isDirectory()) return filesUnder(file);
    return entry.isFile() ? [file] : [];
  }).sort((a, b) => path.relative(directory, a).localeCompare(path.relative(directory, b)));
}

function frame(hash: ReturnType<typeof createHash>, bytes: Buffer): void {
  const length = Buffer.alloc(8);
  length.writeBigUInt64BE(BigInt(bytes.byteLength));
  hash.update(length).update(bytes);
}

/** Hash regular files by sorted relative path and exact bytes, independent of creation order. */
export function sourceTreeIdentity(directory: string): TreeIdentity {
  const files = filesUnder(directory).sort((a, b) =>
    path.relative(directory, a).localeCompare(path.relative(directory, b)));
  const hash = createHash('sha256').update('wombat-synthetic-source-v1\0');
  let bytes = 0;
  for (const file of files) {
    const relative = Buffer.from(path.relative(directory, file).split(path.sep).join('/'));
    const contents = readFileSync(file);
    frame(hash, relative);
    frame(hash, contents);
    bytes += contents.byteLength;
  }
  return { files: files.length, bytes, sha256: hash.digest('hex') };
}

/** Sum logical file lengths without following symlinks; this is not allocated filesystem blocks. */
export function directoryBytes(directory: string): number {
  return readdirSync(directory, { withFileTypes: true }).reduce((sum, entry) => {
    const target = path.join(directory, entry.name);
    if (entry.isDirectory()) return sum + directoryBytes(target);
    if (!entry.isFile()) return sum;
    return sum + lstatSync(target).size;
  }, 0);
}

export interface SnapshotFootprint {
  totalBytes: number;
  manifestBytes: number;
  ledgerBytes: number;
  eventShardBytes: number;
  threadShardBytes: number;
  otherBytes: number;
}

/** Exact logical file lengths for an immutable synthetic snapshot generation. */
export function snapshotFootprint(directory: string): SnapshotFootprint {
  const manifestPath = path.join(directory, 'manifest.json');
  const manifest = JSON.parse(readFileSync(manifestPath, 'utf8')) as {
    ledger: { file: string };
    events: { partitions: Array<{ chunks: Array<{ file: { file: string } }> }> };
    threads: Array<{ file: { file: string } }>;
  };
  const fileSize = (name: string) => statSync(path.join(directory, name)).size;
  const manifestBytes = fileSize('manifest.json');
  const ledgerBytes = fileSize(manifest.ledger.file);
  const eventShardBytes = manifest.events.partitions.flatMap(partition => partition.chunks)
    .reduce((sum, chunk) => sum + fileSize(chunk.file.file), 0);
  const threadShardBytes = manifest.threads.reduce((sum, thread) => sum + fileSize(thread.file.file), 0);
  const totalBytes = directoryBytes(directory);
  return {
    totalBytes,
    manifestBytes,
    ledgerBytes,
    eventShardBytes,
    threadShardBytes,
    otherBytes: totalBytes - manifestBytes - ledgerBytes - eventShardBytes - threadShardBytes,
  };
}

export interface IndexPayloadFootprint {
  scopeKind: 'parserFacts' | 'projection';
  field: 'events' | 'operations' | 'measurements';
  entries: number;
  storedJsonbPayloadBytes: number;
  expandedJsonBytes: number;
  storedCompressedPayloadBytes?: number;
  decodedFrameBytes?: number;
}

/** Read only the synthetic live index; row payload sizes exclude SQLite page and row overhead. */
export function liveIndexPayloadFootprint(databaseFile: string): IndexPayloadFootprint[] {
  const database = new DatabaseSync(databaseFile, { readOnly: true });
  try {
    const schema = Number(database.prepare('PRAGMA user_version').get()?.user_version);
    if (schema === 5) {
      return database.prepare(`
        SELECT CASE WHEN b.scope LIKE 'parser:%:facts' THEN 'parserFacts' ELSE 'projection' END AS scopeKind,
          b.field, SUM(e.count) AS entries, SUM(length(e.data)) AS compressed, SUM(e.bytes) AS decoded
        FROM buckets b JOIN event_blocks e ON e.bucket=b.id GROUP BY b.id
        UNION ALL
        SELECT 'projection', b.field, (SELECT COUNT(*) FROM fact_rows r WHERE r.bucket=b.id), SUM(length(f.data)), SUM(f.bytes)
        FROM buckets b JOIN fact_blocks f ON f.bucket=b.id GROUP BY b.id
        ORDER BY scopeKind, field
      `).all().map(row => ({scopeKind: row.scopeKind as IndexPayloadFootprint['scopeKind'], field: row.field as IndexPayloadFootprint['field'], entries:Number(row.entries), storedJsonbPayloadBytes:0, expandedJsonBytes:0, storedCompressedPayloadBytes:Number(row.compressed), decodedFrameBytes:Number(row.decoded)}));
    }
    const rows = database.prepare(`
      SELECT
        CASE WHEN b.scope LIKE 'parser:%:facts' THEN 'parserFacts' ELSE 'projection' END AS scopeKind,
        b.field AS field,
        COUNT(*) AS entries,
        COALESCE(SUM(CASE WHEN e.payload IS NULL THEN 0 ELSE length(e.payload) END), 0) AS storedJsonbPayloadBytes,
        COALESCE(SUM(CASE
          WHEN e.payload IS NOT NULL THEN length(CAST(json(e.payload) AS BLOB))
          WHEN source.payload IS NOT NULL THEN length(CAST(json_extract(source.payload, e.member) AS BLOB))
          ELSE 0
        END), 0) AS expandedJsonBytes
      FROM buckets b
      JOIN entries e ON e.bucket = b.id
      LEFT JOIN entries source ON source.bucket = e.source_bucket AND source.id = e.id
      WHERE b.field IN ('events', 'operations', 'measurements')
        AND (b.scope LIKE 'parser:%:facts' OR b.scope LIKE 'projection:%')
      GROUP BY scopeKind, b.field
      ORDER BY scopeKind, b.field
    `).all();
    return rows.map(row => ({
      scopeKind: row.scopeKind as IndexPayloadFootprint['scopeKind'],
      field: row.field as IndexPayloadFootprint['field'],
      entries: Number(row.entries),
      storedJsonbPayloadBytes: Number(row.storedJsonbPayloadBytes),
      expandedJsonBytes: Number(row.expandedJsonBytes),
    }));
  } finally {
    database.close();
  }
}

/** Keep business totals, pages and source occurrence times; omit only changing view headers. */
export function normalizeUsageOracle(result: Record<string, any>): Record<string, unknown> {
  const quality = result.quality === undefined ? undefined : {
    ...result.quality,
    // These report work performed by this collection pass. Appending an unchanged corpus
    // can read zero bytes while a fixed snapshot rebuild reads the full source.
    sources: result.quality.sources.map(({ filesRead: _filesRead, bytesRead: _bytesRead, ...stable }: Record<string, unknown>) => stable),
  };
  const stable: Record<string, unknown> = {
    outputVersion: result.outputVersion,
    action: result.action,
    scope: result.scope,
    availableRange: result.availableRange,
    summary: result.summary,
    items: result.items,
    page: result.page,
    quality,
  };
  if (result.facets !== undefined) stable.facets = result.facets;
  if (result.distribution !== undefined) stable.distribution = result.distribution;
  return structuredClone(stable);
}

/** SQLite page ownership includes frame payloads, key indexes, and unused page space. */
export function liveIndexPages(databaseFile:string) {
  const db=new DatabaseSync(databaseFile,{readOnly:true});
  try { return db.prepare('SELECT name, SUM(pgsize) AS bytes, SUM(payload) AS payloadBytes, SUM(unused) AS unusedBytes FROM dbstat GROUP BY name ORDER BY bytes DESC').all().map(row=>({name:String(row.name),bytes:Number(row.bytes),payloadBytes:Number(row.payloadBytes),unusedBytes:Number(row.unusedBytes)})); }
  finally {db.close();}
}
