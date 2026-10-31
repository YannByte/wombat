import {checkWindowsImports} from './native-binary.ts';
import { createHash } from 'node:crypto';
import { execFileSync } from 'node:child_process';
import { existsSync, readFileSync, statSync } from 'node:fs';
import path from 'node:path';
import { releaseNodeVersion } from './github-release.ts';

export const nativeTargets = ['darwin-arm64', 'darwin-x64', 'linux-x64', 'linux-arm64', 'win32-x64'] as const;
export type NativeTarget = typeof nativeTargets[number];
export function checkedSourceRevision(): string {
  const paths = ['core', 'client', 'cli', 'scripts', 'package.json', 'pnpm-lock.yaml', 'pnpm-workspace.yaml', 'rust-toolchain.toml', 'tsconfig.json'];
  const dirty = execFileSync('git', ['status', '--porcelain', '--untracked-files=all', '--', ...paths], { encoding: 'utf8' }).trim();
  if (dirty) throw new Error('Commit source changes before exporting or assembling universal native artifacts; use --current-platform for local testing');
  return execFileSync('git', ['rev-parse', 'HEAD'], { encoding: 'utf8' }).trim();
}
export function nativeBinary(target: string): string { return target.startsWith('win32-') ? 'wombat-core.exe' : 'wombat-core'; }
export function nodeRuntimeBinary(target: string): string { return target.startsWith('win32-') ? 'node.exe' : 'node'; }
export const nodeRuntimeNotice = 'node-runtime.txt';
export function resolveNodeRuntimeLicense(explicit?: string): string {
  const executable = path.resolve(process.execPath);
  const roots = process.platform === 'win32'
    ? [path.dirname(executable)]
    : [path.dirname(path.dirname(executable)), path.dirname(executable)];
  const candidates = [explicit, process.env.WOMBAT_NODE_RUNTIME_LICENSE,
    ...roots.flatMap(root => ['LICENSE', 'LICENSE.txt', 'LICENSE.md'].map(name => path.join(root, name)))].filter(Boolean) as string[];
  const license = candidates.find(file => existsSync(file) && statSync(file).isFile());
  if (!license) throw new Error('Node runtime license was not found; pass --runtime-license <Node distribution LICENSE>');
  return path.resolve(license);
}
export function currentNativeTarget(): NativeTarget {
  const target = process.platform + '-' + process.arch;
  if (!(nativeTargets as readonly string[]).includes(target)) throw new Error('Unsupported native target: ' + target);
  return target as NativeTarget;
}
export function sha256(file: string): string { return createHash('sha256').update(readFileSync(file)).digest('hex'); }
export const nativeNotices = ['node-dependencies.txt', 'rust-dependencies.txt', 'inventory.json', nodeRuntimeNotice] as const;
export interface NativeManifest { target: NativeTarget; version: string; source: string; sha256: string; runtime: {name: 'node'; version: string; sha256: string}; notices: Record<string,string>; }
export function inspectNative(directory: string, target: NativeTarget, version: string, source: string): NativeManifest {
  const folder = path.join(directory, target);
  const manifest: NativeManifest = JSON.parse(readFileSync(path.join(folder, 'manifest.json'), 'utf8'));
  const binary = path.join(folder, nativeBinary(target));
  const runtime = path.join(folder, 'runtime', nodeRuntimeBinary(target));
  if (manifest.target !== target || manifest.version !== version || manifest.source !== source || manifest.sha256 !== sha256(binary)
    || manifest.runtime?.name !== 'node' || manifest.runtime.version !== releaseNodeVersion || manifest.runtime.sha256 !== sha256(runtime))
    throw new Error('Native artifact identity/hash mismatch: ' + target);
  if (!statSync(binary).isFile() || !statSync(runtime).isFile()) throw new Error('Native artifact is not a file: ' + target);
  for (const file of nativeNotices) if (manifest.notices?.[file] !== sha256(path.join(folder,'licenses',file)))
    throw new Error('Native artifact notice hash mismatch: ' + target + '/' + file);
  if (target === 'win32-x64') checkWindowsImports(binary);
  return manifest;
}
