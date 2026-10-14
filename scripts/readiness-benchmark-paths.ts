import assert from 'node:assert/strict';
import { existsSync, realpathSync, lstatSync } from 'node:fs';
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
