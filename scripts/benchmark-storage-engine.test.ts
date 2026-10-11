import test from 'node:test';
import assert from 'node:assert/strict';
import {
  mkdtempSync,
  openSync,
  closeSync,
  rmSync,
  writeFileSync,
  writeSync,
  ftruncateSync,
} from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { hash } from './benchmark-storage-fixture.ts';
import { readSourceBlock } from './benchmark-storage-engine.ts';

test('source-backed experiment rejects changed, truncated and invalid source ranges', () => {
  const dir = mkdtempSync(path.join(os.tmpdir(), 'wombat-storage-range-'));
  const file = path.join(dir, 'source');
  const raw = Buffer.from('[{"kind":"synthetic","value":0}]');
  writeFileSync(file, Buffer.concat([Buffer.from('prefix'), raw]));
  const fd = openSync(file, 'r+'),
    locator = Buffer.alloc(16);
  locator.writeBigUInt64LE(6n);
  locator.writeBigUInt64LE(BigInt(raw.length), 8);
  try {
    assert.deepEqual(readSourceBlock(fd, locator, raw.length, hash(raw)), [
      { kind: 'synthetic', value: 0 },
    ]);
    assert.throws(() => readSourceBlock(fd, locator.subarray(1), raw.length, hash(raw)), /locator/);
    assert.throws(() => readSourceBlock(fd, locator, raw.length + 1, hash(raw)), /length/);
    const invalid = Buffer.from(locator);
    invalid.writeBigUInt64LE(2n ** 63n);
    assert.throws(() => readSourceBlock(fd, invalid, raw.length, hash(raw)), /offset/);
    writeSync(fd, Buffer.from('1'), 0, 1, 6 + raw.indexOf('0'));
    assert.throws(() => readSourceBlock(fd, locator, raw.length, hash(raw)), /generation changed/);
    ftruncateSync(fd, 7);
    assert.throws(() => readSourceBlock(fd, locator, raw.length, hash(raw)), /range unavailable/);
  } finally {
    closeSync(fd);
    rmSync(dir, { recursive: true, force: true });
  }
});
