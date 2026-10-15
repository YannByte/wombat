import { createHash } from 'node:crypto';
import { spawnSync } from 'node:child_process';
import {
  createReadStream,
  createWriteStream,
  existsSync,
  lstatSync,
  mkdirSync,
  mkdtempSync,
  readFileSync,
  readdirSync,
  renameSync,
  rmSync,
  statSync,
  unlinkSync,
  writeFileSync,
} from 'node:fs';
import { Transform } from 'node:stream';
import { pipeline } from 'node:stream/promises';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { CoreError } from '@wombat/client';
import { t } from '@wombat/client/locale';

const repository = 'YannByte/wombat';
const maximumArchiveBytes = 400 * 1024 * 1024;
const maximumMetadataBytes = 4 * 1024 * 1024;

interface ReleaseAsset {
  target: string;
  archive: string;
  sha256: string;
  bytes: number;
}
interface ReleaseSet {
  format: number;
  version: string;
  source: string;
  sourceSha256: string;
  assets: ReleaseAsset[];
}
interface ReleaseMetadata {
  format: number;
  version: string;
  source: string;
  sourceSha256: string;
  target: string;
  runtime: { name: string; version: string };
}
export interface UpdateResult {
  outputVersion: 1;
  action: 'update';
  currentVersion: string;
  availableVersion: string;
  updateAvailable: boolean;
  updated: boolean;
  checked: boolean;
  target: string;
  serviceRestart?: 'confirmed' | 'unconfirmed';
}

export function updateHelp(): string {
  return t('cli.update.help');
}

export function parseUpdateArgs(argv: string[]): {
  version: string;
  check: boolean;
  json: boolean;
  help: boolean;
} {
  let version = 'latest',
    check = false,
    json = false,
    help = false;
  for (let index = 0; index < argv.length; index++) {
    const arg = argv[index];
    if (arg === '--check') {
      if (check) throw new CoreError('INVALID_ARGUMENT', t('cli.update.repeated', { value: arg }));
      check = true;
    } else if (arg === '--json') {
      if (json) throw new CoreError('INVALID_ARGUMENT', t('cli.update.repeated', { value: arg }));
      json = true;
    } else if (arg === '--help' || arg === '-h') help = true;
    else if (arg === '--version' || arg.startsWith('--version=')) {
      if (version !== 'latest')
        throw new CoreError('INVALID_ARGUMENT', t('cli.update.repeated', { value: '--version' }));
      version = arg === '--version' ? argv[++index] : arg.slice('--version='.length);
      if (
        !version ||
        version.startsWith('-') ||
        !/^v?\d+\.\d+\.\d+(?:[-+][0-9A-Za-z.-]+)?$/.test(version)
      )
        throw new CoreError('INVALID_ARGUMENT', t('cli.update.invalidVersion'));
    } else throw new CoreError('INVALID_ARGUMENT', t('cli.update.invalidArgument', { value: arg }));
  }
  return { version, check, json, help };
}

function target(): string {
  const value = `${process.platform}-${process.arch}`;
  if (!['darwin-arm64', 'darwin-x64', 'linux-x64', 'linux-arm64', 'win32-x64'].includes(value))
    throw new Error(t('cli.update.unsupported', { value }));
  return value;
}

function releaseBase(version: string): string {
  if (version === 'latest') return `https://github.com/${repository}/releases/latest/download`;
  return `https://github.com/${repository}/releases/download/${version.startsWith('v') ? version : `v${version}`}`;
}

async function* httpChunks(url: string, maximumBytes: number): AsyncGenerator<Uint8Array> {
  const result = await fetch(url, {
    redirect: 'follow',
    signal: AbortSignal.timeout(60_000),
  });
  if (!result.ok || !result.body) {
    await result.body?.cancel();
    throw new Error(t('cli.update.downloadFailed', { value: `${result.status} ${url}` }));
  }
  if (Number(result.headers.get('content-length') ?? 0) > maximumBytes) {
    await result.body.cancel();
    throw new Error(t('cli.update.downloadTooLarge'));
  }
  const reader = result.body.getReader();
  let complete = false;
  try {
    for (;;) {
      const chunk = await reader.read();
      if (chunk.done) {
        complete = true;
        break;
      }
      yield chunk.value;
    }
  } finally {
    try {
      if (!complete) await reader.cancel();
    } finally {
      reader.releaseLock();
    }
  }
}

async function* boundedDownload(url: string, maximumBytes: number): AsyncGenerator<Uint8Array> {
  const source =
    new URL(url).protocol === 'file:'
      ? createReadStream(fileURLToPath(url))
      : httpChunks(url, maximumBytes);
  let bytes = 0;
  for await (const chunk of source) {
    // Node file streams expose an untyped chunk; both inputs must remain bytes.
    if (!(chunk instanceof Uint8Array))
      throw new Error(t('cli.update.downloadFailed', { value: url }));
    bytes += chunk.byteLength;
    if (bytes > maximumBytes) throw new Error(t('cli.update.downloadTooLarge'));
    yield chunk;
  }
}

async function downloadText(url: string): Promise<string> {
  const chunks: Uint8Array[] = [];
  for await (const chunk of boundedDownload(url, maximumMetadataBytes)) chunks.push(chunk);
  return Buffer.concat(chunks).toString('utf8');
}

async function downloadFile(
  url: string,
  destination: string,
): Promise<{ sha256: string; bytes: number }> {
  const digest = createHash('sha256');
  let bytes = 0;
  try {
    const measured = new Transform({
      transform(chunk: Buffer, _encoding, callback) {
        bytes += chunk.byteLength;
        digest.update(chunk);
        callback(null, chunk);
      },
    });
    await pipeline(
      boundedDownload(url, maximumArchiveBytes),
      measured,
      createWriteStream(destination, { flags: 'wx' }),
    );
  } catch (error) {
    rmSync(destination, { force: true });
    throw error;
  }
  return { sha256: digest.digest('hex'), bytes };
}

function runTar(args: string[], cwd?: string): string {
  // Git Bash's GNU tar treats Windows drive letters as remote archive names.
  const program =
    process.platform === 'win32'
      ? path.join(process.env.SystemRoot ?? 'C:\\Windows', 'System32', 'tar.exe')
      : 'tar';
  const result = spawnSync(program, args, {
    cwd,
    encoding: 'utf8',
    timeout: 120_000,
    maxBuffer: 16 * 1024 * 1024,
  });
  if (result.error || result.status !== 0)
    throw new Error(
      t('cli.update.extractFailed', {
        value: result.error?.message ?? result.stderr.trim(),
      }),
    );
  return result.stdout;
}

function validateArchive(archive: string): void {
  const directory = path.dirname(archive),
    filename = path.basename(archive);
  const names = runTar(['-tzf', filename], directory).split(/\r?\n/).filter(Boolean);
  if (!names.length || names.length > 10_000) throw new Error(t('cli.update.invalidArchive'));
  for (const name of names) {
    const normalized = name.replace(/\/$/, '');
    if (
      !normalized ||
      normalized.includes('\\') ||
      normalized.startsWith('/') ||
      (normalized !== 'wombat' && !normalized.startsWith('wombat/')) ||
      normalized.split('/').some((part) => part === '..' || part === '.')
    )
      throw new Error(t('cli.update.invalidArchive'));
  }
  const rows = runTar(['-tvzf', filename], directory).split(/\r?\n/).filter(Boolean);
  if (rows.length !== names.length || rows.some((row) => !['-', 'd'].includes(row[0])))
    throw new Error(t('cli.update.invalidArchive'));
}

function validateTree(directory: string, budget = { entries: 10_000 }): void {
  for (const name of readdirSync(directory)) {
    if (--budget.entries < 0) throw new Error(t('cli.update.invalidArchive'));
    const entry = path.join(directory, name),
      stat = lstatSync(entry);
    if (stat.isSymbolicLink() || (!stat.isDirectory() && !stat.isFile()))
      throw new Error(t('cli.update.invalidArchive'));
    if (stat.isDirectory()) validateTree(entry, budget);
  }
}

function readJson<T>(file: string): T {
  return JSON.parse(readFileSync(file, 'utf8')) as T;
}
function safeIdentity(value: string): boolean {
  return /^[0-9A-Za-z][0-9A-Za-z._-]{0,127}$/.test(value);
}

function installation(entryFile: string): {
  root: string;
  currentId: string;
  release: ReleaseMetadata;
} {
  const payload = path.dirname(path.dirname(path.resolve(entryFile)));
  const versions = path.dirname(payload),
    root = path.dirname(versions),
    currentId = path.basename(payload);
  const releaseFile = path.join(payload, 'release.json'),
    pointer = path.join(root, 'current.txt');
  if (path.basename(versions) !== 'versions' || !existsSync(releaseFile) || !existsSync(pointer))
    throw new Error(t('cli.update.notManaged'));
  const selected = readFileSync(pointer, 'utf8').trim();
  if (selected !== currentId || !safeIdentity(selected))
    throw new Error(t('cli.update.notCurrent'));
  return { root, currentId, release: readJson<ReleaseMetadata>(releaseFile) };
}

function switchPointer(root: string, next: string): void {
  const current = path.join(root, 'current.txt'),
    temporary = path.join(root, `.current-${process.pid}.tmp`);
  writeFileSync(temporary, next + '\n', { flag: 'wx' });
  try {
    renameSync(temporary, current);
  } catch (error) {
    const backup = path.join(root, `.current-${process.pid}.previous`);
    renameSync(current, backup);
    try {
      renameSync(temporary, current);
      unlinkSync(backup);
    } catch (replacementError) {
      renameSync(backup, current);
      throw new AggregateError([error, replacementError], t('cli.update.switchFailed'));
    }
  }
}

export async function updateInstalled(
  options: {
    version?: string;
    check?: boolean;
    entryFile?: string;
    baseUrl?: string;
  } = {},
): Promise<UpdateResult> {
  const installed = installation(options.entryFile ?? fileURLToPath(import.meta.url));
  const platform = target(),
    base = options.baseUrl ?? releaseBase(options.version ?? 'latest');
  const set = JSON.parse(await downloadText(`${base}/release-set.json`)) as ReleaseSet;
  if (
    set.format !== 1 ||
    !safeIdentity(set.version) ||
    !/^[0-9a-f]{40}$/.test(set.source) ||
    !/^[0-9a-f]{64}$/.test(set.sourceSha256)
  )
    throw new Error(t('cli.update.invalidMetadata'));
  const asset = set.assets.find((item) => item.target === platform);
  if (
    !asset ||
    asset.archive !== `wombat-${platform}.tar.gz` ||
    !/^[0-9a-f]{64}$/.test(asset.sha256) ||
    !Number.isSafeInteger(asset.bytes) ||
    asset.bytes <= 0 ||
    asset.bytes > maximumArchiveBytes
  )
    throw new Error(t('cli.update.invalidMetadata'));
  const current =
    installed.release.version === set.version &&
    installed.release.source === set.source &&
    installed.release.sourceSha256 === set.sourceSha256;
  const result: UpdateResult = {
    outputVersion: 1,
    action: 'update',
    currentVersion: installed.release.version,
    availableVersion: set.version,
    updateAvailable: !current,
    updated: false,
    checked: Boolean(options.check),
    target: platform,
  };
  if (current || options.check) return result;

  const checksums = await downloadText(`${base}/SHA256SUMS`);
  const checksum = checksums
    .split(/\r?\n/)
    .map((line) => line.match(/^([0-9a-f]{64})  ([^/\\]+)$/))
    .find((match) => match?.[2] === asset.archive)?.[1];
  if (checksum !== asset.sha256) throw new Error(t('cli.update.invalidChecksum'));
  mkdirSync(path.join(installed.root, 'versions'), { recursive: true });
  const scratch = mkdtempSync(path.join(installed.root, '.update-'));
  try {
    const archive = path.join(scratch, asset.archive);
    const downloaded = await downloadFile(`${base}/${asset.archive}`, archive);
    if (downloaded.sha256 !== asset.sha256 || downloaded.bytes !== asset.bytes)
      throw new Error(t('cli.update.invalidChecksum'));
    validateArchive(archive);
    const extracted = path.join(scratch, 'extracted');
    mkdirSync(extracted);
    runTar(['-xzf', asset.archive, '-C', 'extracted'], scratch);
    const payload = path.join(extracted, 'wombat');
    validateTree(payload);
    const release = readJson<ReleaseMetadata>(path.join(payload, 'release.json'));
    if (
      release.format !== 1 ||
      release.version !== set.version ||
      release.source !== set.source ||
      release.sourceSha256 !== set.sourceSha256 ||
      release.target !== platform ||
      release.runtime?.name !== 'node'
    )
      throw new Error(t('cli.update.invalidArchive'));
    const runtime = path.join(
      payload,
      'runtime',
      process.platform === 'win32' ? 'node.exe' : 'node',
    );
    const cli = path.join(payload, 'lib', 'wombat.js');
    if (!statSync(runtime).isFile() || !statSync(cli).isFile())
      throw new Error(t('cli.update.invalidArchive'));
    const identity = `${set.version}-${set.source.slice(0, 12)}-${set.sourceSha256.slice(0, 12)}`;
    if (!safeIdentity(identity)) throw new Error(t('cli.update.invalidMetadata'));
    const destination = path.join(installed.root, 'versions', identity);
    if (!existsSync(destination)) renameSync(payload, destination);
    else {
      const existing = readJson<ReleaseMetadata>(path.join(destination, 'release.json'));
      if (
        existing.source !== set.source ||
        existing.sourceSha256 !== set.sourceSha256 ||
        existing.version !== set.version ||
        existing.target !== platform
      )
        throw new Error(t('cli.update.invalidArchive'));
    }
    switchPointer(installed.root, identity);
    for (const name of readdirSync(path.join(installed.root, 'versions'))) {
      if (name !== identity && name !== installed.currentId && safeIdentity(name))
        rmSync(path.join(installed.root, 'versions', name), {
          recursive: true,
          force: true,
        });
    }
    result.updated = true;
    const registration = path.join(installed.root, 'service-registration.json');
    if (existsSync(registration)) {
      // The runtime switch has completed. Unreadable or unknown service metadata
      // must retain that result and expose unconfirmed startup without mutation.
      result.serviceRestart = 'unconfirmed';
      let service: { version?: unknown; enabled?: unknown } = {};
      try {
        const stat = lstatSync(registration);
        if (stat.isFile() && !stat.isSymbolicLink() && stat.size <= 32768)
          service = JSON.parse(readFileSync(registration, 'utf8'));
      } catch {
        /* Keep the new installation and report the independent service state. */
      }
      if (service.version === 1 && service.enabled === false) delete result.serviceRestart;
      if (service.version === 1 && service.enabled === true) {
        const nextRuntime = path.join(
          destination,
          'runtime',
          process.platform === 'win32' ? 'node.exe' : 'node',
        );
        const restarted = spawnSync(
          nextRuntime,
          [path.join(destination, 'lib', 'wombat.js'), 'service', 'enable', '--json'],
          {
            env: {
              ...process.env,
              WOMBAT_INSTALL_ROOT: installed.root,
              WOMBAT_ENDPOINT_FILE: path.join(installed.root, 'endpoint.json'),
            },
            encoding: 'utf8',
            timeout: 60_000,
            maxBuffer: 65536,
            windowsHide: true,
            killSignal: 'SIGKILL',
          },
        );
        result.serviceRestart = 'unconfirmed';
        if (!restarted.error && restarted.status === 0) {
          try {
            const status = JSON.parse(restarted.stdout) as {
              running?: unknown;
              autoStart?: unknown;
            };
            if (status.running === true && status.autoStart === true)
              result.serviceRestart = 'confirmed';
          } catch {
            /* A completed update does not establish service startup. */
          }
        }
      }
    }
    return result;
  } finally {
    rmSync(scratch, {
      recursive: true,
      force: true,
      maxRetries: 10,
      retryDelay: 200,
    });
  }
}

export async function runUpdateCli(argv: string[]): Promise<number> {
  const invocation = parseUpdateArgs(argv);
  if (invocation.help) {
    process.stdout.write(
      invocation.json
        ? JSON.stringify({ outputVersion: 1, help: updateHelp() }) + '\n'
        : updateHelp(),
    );
    return 0;
  }
  const result = await updateInstalled({
    version: invocation.version,
    check: invocation.check,
  });
  if (invocation.json) process.stdout.write(JSON.stringify(result) + '\n');
  else if (result.updated)
    process.stdout.write(
      t('cli.update.updated', {
        from: result.currentVersion,
        to: result.availableVersion,
      }) + '\n',
    );
  else if (!result.updateAvailable)
    process.stdout.write(t('cli.update.current', { version: result.currentVersion }) + '\n');
  else
    process.stdout.write(
      t('cli.update.available', {
        current: result.currentVersion,
        available: result.availableVersion,
      }) + '\n',
    );
  if (result.serviceRestart === 'unconfirmed' && !invocation.json)
    process.stderr.write(t('service.updatePending') + '\n');
  return result.serviceRestart === 'unconfirmed' ? 2 : 0;
}
