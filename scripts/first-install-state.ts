/** Preparation for disposable first-install runs. Source paths are never targets. */
import assert from 'node:assert/strict';
import {existsSync, lstatSync, realpathSync, rmSync} from 'node:fs';
import path from 'node:path';

export function prepareFirstInstall(workspace: string, targets: string[]): {clearedPaths: string[]; fresh: true} {
  assert(path.isAbsolute(workspace), 'First-install workspace must be absolute');
  const root = realpathSync(workspace);
  assert(lstatSync(root).isDirectory(), 'First-install workspace must be a directory');
  const checked = targets.map(target => {
    assert(path.isAbsolute(target), 'First-install targets must be absolute');
    const relative = path.relative(workspace, target);
    assert(relative && relative !== '..' && !relative.startsWith('..' + path.sep) && !path.isAbsolute(relative),
      'First-install cleanup must stay inside its disposable workspace');
    const canonical = path.join(root, relative);
    let parent = canonical;
    while (parent !== root) {
      let stat;
      try { stat = lstatSync(parent); }
      catch (error) { if ((error as NodeJS.ErrnoException).code !== 'ENOENT') throw error; }
      assert(!stat?.isSymbolicLink(), 'First-install cleanup refuses symlink paths');
      parent = path.dirname(parent);
    }
    return canonical;
  });
  // Validate every target before deleting any state.
  for (const target of checked) rmSync(target, {recursive: true, force: true});
  assert(checked.every(target => !existsSync(target)), 'First-install preparation left historical state');
  return {clearedPaths: checked, fresh: true};
}
