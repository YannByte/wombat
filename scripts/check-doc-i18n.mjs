import { inspectPair, recordDifferences } from './doc-pairing.mjs';
import { createHash } from 'node:crypto';
import { existsSync, readFileSync, readdirSync, writeFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const manifest = JSON.parse(readFileSync(path.join(root, 'scripts/doc-i18n.manifest.json'), 'utf8'));
const args = process.argv.slice(2).filter(arg => arg !== '--');
const write = args[0] === '--write';
const migrate = args[0] === '--migrate';
if (write || migrate) args.shift();
const requested = args;
const errors = [];
const pairs = manifest.pairs;
const known = new Set([...manifest.legacyUnpaired, ...manifest.excluded]);
if (known.size !== manifest.legacyUnpaired.length + manifest.excluded.length) errors.push('duplicate legacy or excluded manifest entry');
const pairByPath = new Map();
const content = file => readFileSync(path.join(root, file), 'utf8');
const hash = value => createHash('sha256').update(value).digest('hex');

for (const pair of pairs) {
  for (const file of [pair.zh, pair.en, pair.record]) {
    if (known.has(file) || pairByPath.has(file)) errors.push(`${file}: duplicate i18n manifest entry`);
    pairByPath.set(file, pair);
    known.add(file);
  }
}

// Localized screenshots are reviewed pairs too, without prose hash records.
for (const pair of manifest.assetPairs ?? []) {
  for (const language of ['zh', 'en']) {
    const file = pair[language];
    if (typeof file !== 'string' || !/^assets\/[a-z0-9-]+\.(?:png|svg|jpe?g)$/.test(file)
      || !existsSync(path.join(root, file)) || pairByPath.has(file)) {
      errors.push(`Invalid or duplicate ${language} asset pair: ${file}`);
    } else pairByPath.set(file, pair);
  }
}

function walk(directory) {
  const found = [];
  for (const entry of readdirSync(path.join(root, directory), { withFileTypes: true })) {
    if (entry.isDirectory() && ['target', 'dist', 'node_modules', 'vendor', '.git'].includes(entry.name)) continue;
    const relative = path.posix.join(directory, entry.name);
    if (entry.isDirectory()) found.push(...walk(relative));
    else if (entry.name.endsWith('.md') || entry.name.endsWith('.i18n.json')) found.push(relative);
  }
  return found;
}

for (const directory of ['docs', 'core', 'client', 'cli']) {
  for (const file of walk(directory)) {
    if (!known.has(file)) errors.push(`${file}: add a complete bilingual pair to the manifest or justify an exclusion`);
  }
}
if (existsSync(path.join(root, '.github'))) {
  for (const file of walk('.github')) if (!known.has(file)) errors.push(`${file}: add a complete bilingual pair to the manifest or justify an exclusion`);
}
for (const entry of readdirSync(root, { withFileTypes: true })) {
  if (entry.isFile() && entry.name.endsWith('.md') && !known.has(entry.name)) {
    errors.push(`${entry.name}: add a complete bilingual pair to the manifest or justify an exclusion`);
  }
}
for (const file of [...manifest.legacyUnpaired, ...manifest.excluded]) {
  if (!existsSync(path.join(root, file))) errors.push(`${file}: stale i18n manifest entry`);
}

const records = new Map();
const selected = requested.length ? [...new Set(requested.map(file => pairByPath.get(file)).filter(Boolean))] : pairs;
for (const pair of selected) {
  const missing = [pair.zh, pair.en, pair.record].filter(file => !existsSync(path.join(root, file)));
  if (missing.length && !(write && missing.length === 1 && missing[0] === pair.record)) {
    errors.push(`${pair.zh}: incomplete pair; missing ${missing.join(', ')}`);
    continue;
  }
  const zh = content(pair.zh);
  const en = content(pair.en);
  const inspected = inspectPair(pair, zh, en, pairByPath);
  errors.push(...inspected.errors);
  records.set(pair, JSON.stringify(inspected.record, null, 2) + '\n');
  if (!write && !missing.length) {
    let recorded;
    try { recorded = JSON.parse(content(pair.record)); } catch { errors.push(`${pair.record}: invalid JSON`); continue; }
    if (migrate && recorded.version !== 2) {
      if (recorded.zh !== hash(zh) || recorded.en !== hash(en)) errors.push(`${pair.record}: changed since whole-file confirmation; review and record this pair first`);
    } else {
      errors.push(...recordDifferences(recorded, inspected.record).map(error => `${pair.record}: ${error}`));
      if (!migrate && JSON.stringify(recorded) === JSON.stringify(inspected.record) && content(pair.record) !== records.get(pair)) errors.push(`${pair.record}: noncanonical record formatting`);
    }
  }
}

if (write) {
  if (!requested.length) errors.push('pass one or more confirmed pair paths after --write');
  for (const anchor of requested) if (!pairByPath.has(anchor)) errors.push(`${anchor}: pair is not declared`);
}
for (const anchor of requested) if (!pairByPath.has(anchor)) errors.push(`${anchor}: pair is not declared`);

if (errors.length) {
  for (const error of errors) console.error(error);
  process.exit(1);
}
if (write || migrate) {
  for (const pair of selected) {
    const record = records.get(pair);
    writeFileSync(path.join(root, pair.record), record);
    console.log(`Confirmed pair recorded: ${pair.record}`);
  }
} else {
  console.log(`Bilingual documentation passed: ${selected.length} pair(s), ${manifest.legacyUnpaired.length} legacy unpaired page(s).`);
}
