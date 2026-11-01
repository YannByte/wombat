import {checkWindowsImports} from './native-binary.ts';
import { spawnSync } from 'node:child_process';
import { existsSync, mkdirSync, copyFileSync, chmodSync, renameSync, rmSync } from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const executable = process.platform === 'win32' ? 'cargo.exe' : 'cargo';
const local = path.join(os.homedir(), '.cargo', 'bin', executable);
const cargo = process.env.WOMBAT_CARGO || (existsSync(local) ? local : executable);
const args=process.argv.slice(2);
if(args.length>1||args.some(arg=>!['--test','--check','--fetch'].includes(arg)))throw new Error('Choose one of --test, --check or --fetch');
const test = args.includes('--test');
const check = args.includes('--check');
const fetch = args.includes('--fetch');
const buildEnv = { ...process.env };
if (!test && !check && !fetch) {
  if (process.platform === 'darwin') buildEnv.MACOSX_DEPLOYMENT_TARGET = '11.0';
  // Keep developer home/check-out paths out of distributed panic/source metadata.
  // Encoded flags preserve paths containing spaces and retain caller-supplied flags.
  const flags = buildEnv.CARGO_ENCODED_RUSTFLAGS !== undefined
    ? buildEnv.CARGO_ENCODED_RUSTFLAGS.split('\x1f').filter(Boolean)
    : (buildEnv.RUSTFLAGS || '').trim().split(/\s+/).filter(Boolean);
  if (process.platform === 'win32') flags.push('-C', 'target-feature=+crt-static');
  flags.push(`--remap-path-prefix=${os.homedir()}=/build-home`);
  if (buildEnv.CARGO_HOME) flags.push(`--remap-path-prefix=${buildEnv.CARGO_HOME}=/cargo-home`);
  flags.push(`--remap-path-prefix=${root}=/wombat`);
  buildEnv.CARGO_ENCODED_RUSTFLAGS = flags.join('\x1f');
  delete buildEnv.RUSTFLAGS;
}
const result = spawnSync(cargo, [fetch ? 'fetch' : test ? 'test' : 'build', '--locked', ...(test || check || fetch ? [] : ['--release']), '--manifest-path', 'core/Cargo.toml'], { cwd: root, env: buildEnv, stdio: 'inherit' });
if (result.error) { process.stderr.write(`Rust 构建工具不可用：${result.error.message}\n请安装 Rust 工具链后重试。\n`); process.exit(1); }
if (result.status !== 0) process.exit(result.status ?? 1);
if (!test && !check && !fetch) {
  const binary = process.platform === 'win32' ? 'wombat-core.exe' : 'wombat-core';
  mkdirSync(path.join(root, 'dist'), { recursive: true });
  const target = path.join(root, 'dist', binary);
  const staged = `${target}.${process.pid}.tmp`;
  try {
    copyFileSync(path.join(root, 'core', 'target', 'release', binary), staged);
    if (process.platform === 'win32') checkWindowsImports(staged);
    chmodSync(staged, 0o755);
    renameSync(staged, target);
  } finally { rmSync(staged, { force: true }); }
  const notices = path.join(root, 'dist/licenses');
  mkdirSync(notices, { recursive: true });
  for (const [source, name] of [
    ['LICENSE', 'wombat-MIT.txt'],
    ['licenses/THIRD_PARTY_NOTICES.md', 'third-party-notices.md'],
    ['licenses/node-dependencies.txt', 'node-dependencies.txt'],
    ['licenses/rust-dependencies.txt', 'rust-dependencies.txt'],
  ]) copyFileSync(path.join(root, source), path.join(notices, name));
}
