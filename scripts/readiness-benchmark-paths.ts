import assert from 'node:assert/strict';
import { existsSync, realpathSync, lstatSync, readFileSync } from 'node:fs';
import path from 'node:path';
function physical(input: string): string {
  const resolved = path.resolve(input);
  let parent = resolved;
  const suffix: string[] = [];
  while (!existsSync(parent)) {
    suffix.unshift(path.basename(parent));
    const next = path.dirname(parent);
    assert(next !== parent, 'No existing path ancestor');
    parent = next;
  }
  return path.join(realpathSync(parent), ...suffix);
}
const contains = (parent: string, child: string) =>
  child === parent || child.startsWith(parent + path.sep);

/** Use normal installed discovery only when the selected CLI owns this exact core. */
export function readinessCliEnvironment(core: string, cli: string, env: NodeJS.ProcessEnv): NodeJS.ProcessEnv {
  const root = path.resolve(path.dirname(cli), '../../..');
  const marker = path.join(root, '.managed-by-wombat');
  const pointer = path.join(root, 'current.txt');
  if (!existsSync(marker) || !existsSync(pointer)) return {...env};
  if ([root, marker, pointer].some(file => lstatSync(file).isSymbolicLink())) return {...env};
  if (lstatSync(marker).size > 128 || lstatSync(pointer).size > 128
    || readFileSync(marker, 'utf8').trim() !== 'managed GitHub installation') return {...env};
  const version = readFileSync(pointer, 'utf8').trim();
  if (!/^[0-9A-Za-z][0-9A-Za-z._-]{0,127}$/.test(version)) return {...env};
  const lib = path.join(root, 'versions', version, 'lib');
  if (path.resolve(cli) !== path.join(lib, 'wombat.js')
    || realpathSync(core) !== realpathSync(path.join(lib, process.platform === 'win32' ? 'wombat-core.exe' : 'wombat-core'))) return {...env};
  const normal:NodeJS.ProcessEnv = {...env, WOMBAT_INSTALL_ROOT:root};
  delete normal.WOMBAT_CORE_BIN;
  return normal;
}
/** Validate deletion targets before preparation writes; source roots never become product paths. */
export function validateReadinessPaths(output: string, repo: string, source?: string): void {
  if (existsSync(output))
    assert(
      lstatSync(output).isDirectory() && !lstatSync(output).isSymbolicLink(),
      'Output must be an owned regular directory',
    );
  const out = physical(output),
    repository = physical(repo);
  assert(
    !contains(repository, out) && !contains(out, repository),
    'Output must be separate from the repository',
  );
  if (source) {
    const input = physical(source);
    assert(!contains(input, out), 'Output cannot write inside source logs');
    for (const name of ['data', 'runtime']) {
      const product = physical(path.join(out, name));
      assert(
        !contains(product, input) && !contains(input, product),
        'Source and disposable product paths must be separate',
      );
    }
  }
}
