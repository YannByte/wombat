/** Locate only the managed installation that owns this runtime entry. */
import { existsSync, lstatSync, readFileSync } from 'node:fs';
import path from 'node:path';

export function managedInstallationRoot(): string | undefined {
  const entry = process.argv[1] && path.resolve(process.argv[1]);
  const candidate =
    process.env.WOMBAT_INSTALL_ROOT ??
    (entry ? path.dirname(path.dirname(path.dirname(path.dirname(entry)))) : undefined);
  if (!candidate) return;
  const root = path.resolve(candidate),
    marker = path.join(root, '.managed-by-wombat');
  if (!existsSync(marker)) return;
  const info = lstatSync(marker);
  if (
    lstatSync(root).isSymbolicLink() ||
    !info.isFile() ||
    info.isSymbolicLink() ||
    info.size > 128 ||
    readFileSync(marker, 'utf8').trim() !== 'managed GitHub installation'
  )
    return;
  if (!process.env.WOMBAT_INSTALL_ROOT) {
    const pointer = path.join(root, 'current.txt');
    if (
      !existsSync(pointer) ||
      lstatSync(pointer).isSymbolicLink() ||
      lstatSync(pointer).size > 128
    )
      return;
    const release = readFileSync(pointer, 'utf8').trim();
    if (
      !/^[0-9A-Za-z][0-9A-Za-z._-]{0,127}$/.test(release) ||
      entry !== path.join(root, 'versions', release, 'lib', 'wombat.js')
    )
      return;
  }
  return root;
}
