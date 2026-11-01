import { existsSync, readFileSync, readdirSync } from 'node:fs';
import ts from 'typescript';
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
  const routing = path.join(root, 'cli/src/command-routing.ts');
  const source = ts.createSourceFile(routing, existsSync(routing) ? readFileSync(routing, 'utf8') : '', ts.ScriptTarget.Latest, true);
  let commands: string[] = [];
  function visit(node: ts.Node): void {
    if (ts.isVariableDeclaration(node) && node.name.getText(source) === 'commandNames' && node.initializer && ts.isArrayLiteralExpression(node.initializer)) {
      commands = node.initializer.elements.map(e => ts.isStringLiteral(e) ? e.text : '');
    }
    ts.forEachChild(node, visit);
  }
  visit(source);
  if (JSON.stringify(commands) !== '["history","usage","check","clean","system"]') errors.push('cli/src/command-routing.ts: expose exactly history, usage, check, clean and system');
  const references = path.join(root, 'plugin/skills/wombat/references');
  const files = existsSync(references) ? readdirSync(references).filter(f => f.endsWith('.md')).sort() : [];
  if (JSON.stringify(files) !== '["cleanup.md","diagnose.md","environment.md","query.md","verify.md"]') errors.push('plugin/skills/wombat/references: retain exactly cleanup.md, diagnose.md, environment.md, query.md and verify.md');
  const main = path.join(root, 'plugin/skills/wombat/SKILL.md');
  const links = existsSync(main) ? [...readFileSync(main, 'utf8').matchAll(/\]\(references\/([^/)]+\.md)\)/g)].map(m => m[1]).sort() : [];
  if (JSON.stringify(links) !== JSON.stringify(files) || links.length !== 5) errors.push('plugin/skills/wombat/SKILL.md: link directly to each of the five task references once');
  return errors;
}
if (process.argv[1] && pathToFileURL(path.resolve(process.argv[1])).href === import.meta.url) {
  const errors = deliveryErrors(path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..'));
  if (errors.length) { console.error(errors.join('\n')); process.exitCode = 1; }
  else console.log('CLI and one main Skill delivery boundary passed.');
}
