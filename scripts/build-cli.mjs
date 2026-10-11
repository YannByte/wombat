import { build } from 'esbuild';
import { chmodSync, existsSync, mkdirSync, mkdtempSync, readdirSync, renameSync, rmSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const dist = path.join(root, 'dist');
mkdirSync(dist, { recursive: true });
const staging = mkdtempSync(path.join(dist, '.cli-build-'));
const backup = path.join(staging, 'previous');
const isCliFile = file => /^wombat(?:-.*)?\.js$/.test(file);
let preserveStaging = false;
try {
  const output = path.join(staging, 'next');
  await build({ absWorkingDir: root, entryPoints: { wombat: 'cli/src/index.ts' }, bundle: true, minify: true, splitting: true, platform: 'node', format: 'esm', target: 'node22', outdir: output, chunkNames: 'wombat-[name]-[hash]', banner: { js: '#!/usr/bin/env node\nimport {createRequire as wombatCreateRequire} from "node:module"; const require = wombatCreateRequire(import.meta.url);' } });
  chmodSync(path.join(output, 'wombat.js'), 0o755);
  mkdirSync(backup);
  const previous = readdirSync(dist).filter(isCliFile);
  const next = readdirSync(output).filter(isCliFile);
  const installed = [];
  try {
    for (const file of previous) renameSync(path.join(dist, file), path.join(backup, file));
    for (const file of next) {
      renameSync(path.join(output, file), path.join(dist, file));
      installed.push(file);
    }
  } catch (error) {
    for (const file of installed) rmSync(path.join(dist, file), { force: true });
    try {
      for (const file of previous) if (existsSync(path.join(backup, file)))
        renameSync(path.join(backup, file), path.join(dist, file));
    } catch (restoreError) {
      preserveStaging = true;
      throw new AggregateError([error, restoreError], `CLI restore failed; previous files remain in ${backup}`);
    }
    throw error;
  }
} finally {
  if (!preserveStaging) rmSync(staging, { recursive: true, force: true });
}
