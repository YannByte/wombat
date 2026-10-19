import { enableCompileCache } from 'node:module';
import path from 'node:path';
import { managedInstallationRoot } from './installation.js';

/** Cache compiled modules with their owning version, never user query data. */
export function enableInstalledCompileCache(): void {
  if (process.env.NODE_COMPILE_CACHE || process.env.NODE_DISABLE_COMPILE_CACHE) return;
  try {
    const root = managedInstallationRoot();
    if (!root || !process.argv[1]) return;
    const parts = path.relative(root, path.resolve(process.argv[1])).split(path.sep);
    if (parts.length !== 4 || parts[0] !== 'versions' || parts[2] !== 'lib'
      || parts[3] !== 'wombat.js' || !/^[0-9A-Za-z][0-9A-Za-z._-]{0,127}$/.test(parts[1])) return;
    enableCompileCache(path.join(root, 'versions', parts[1], 'cache', 'node'));
  } catch {
    // Optional runtime reuse must not prevent calls on read-only installations.
  }
}
