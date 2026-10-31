import {execFileSync} from 'node:child_process';
import {existsSync, readFileSync, readdirSync, statSync} from 'node:fs';
import path from 'node:path';
import {hash, hashFile, inventory, type PayloadFile} from './artifact-files.ts';
export function sourceIdentity(root: string): string {
  const paths = ['plugin', 'core', 'client', 'cli', 'scripts', 'licenses', 'LICENSE', 'package.json', 'pnpm-lock.yaml', 'pnpm-workspace.yaml', 'rust-toolchain.toml', 'tsconfig.json'];
  const files = execFileSync('git', ['ls-files', '--cached', '--others', '--exclude-standard', '-z', '--', ...paths], {cwd: root, encoding: 'utf8'}).split('\0').filter(f => f && existsSync(path.join(root, f)));
  return hash([...new Set(files)].sort().map(f => f+'\0'+hashFile(path.join(root, f))).join('\n'));
}
export function builtFiles(root: string): PayloadFile[] {
  const dist = path.join(root, 'dist');
  return readdirSync(dist).sort().filter(f => /^wombat(?:-.*)?\.js$/.test(f) || ['wombat-core','wombat-core.exe','licenses','skill'].includes(f)).flatMap(f => {
    if (f === 'licenses' || f === 'skill') return inventory(path.join(dist, f)).map(item => ({...item, path: f+'/'+item.path}));
    const file = path.join(dist, f), stat = statSync(file);
    return [{path: f, size: stat.size, sha256: hashFile(file), executable: (stat.mode & 0o111) !== 0}];
  });
}
export function checkBuild(root: string): void {
  const file = path.join(root, 'dist/build.json');
  if (!existsSync(file)) throw new Error('No build receipt; run pnpm build first');
  const receipt = JSON.parse(readFileSync(file, 'utf8'));
  if (receipt.format !== 1 || receipt.target !== process.platform+'-'+process.arch || receipt.sourceSha256 !== sourceIdentity(root) || JSON.stringify(receipt.files) !== JSON.stringify(builtFiles(root)))
    throw new Error('Build is stale or modified; run pnpm build again');
}
