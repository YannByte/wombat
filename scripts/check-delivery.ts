import { existsSync, readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';

/** The shipped surface has one owner; removed hosts must not silently re-enter assembly. */
export function deliveryErrors(root: string): string[] {
  const errors: string[] = [];
  for (const removed of ['ui', 'web', 'client/src/http', 'cli/src/web-cli.ts', 'cli/src/handoff-cli.ts', 'cli/src/skill-cli.ts', 'client/src/node/skill-installation.ts', 'plugin/hooks', 'plugin/scripts']) {
    if (existsSync(path.join(root, removed))) errors.push(`Removed delivery surface: ${removed}`);
  }
  for (const file of ['package.json', 'client/package.json', 'cli/package.json']) {
    const value = readFileSync(path.join(root, file), 'utf8');
    if (/@wombat\/(?:ui|web)|build-web|verify:event-upgrade:browser|"\.\/http"/.test(value)) errors.push(`${file}: removed host dependency or entry`);
  }
  const metadata = JSON.parse(readFileSync(path.join(root, 'plugin/package.json'), 'utf8'));
  if (JSON.stringify(metadata.skills) !== '["wombat"]') errors.push('plugin/package.json: ship exactly one main Skill');
  return errors;
}
if (process.argv[1] && pathToFileURL(path.resolve(process.argv[1])).href === import.meta.url) {
  const errors = deliveryErrors(path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..'));
  if (errors.length) { console.error(errors.join('\n')); process.exitCode = 1; }
  else console.log('CLI and one main Skill delivery boundary passed.');
}
