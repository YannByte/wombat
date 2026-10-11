import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath, pathToFileURL } from 'node:url';
import { fromMarkdown } from 'mdast-util-from-markdown';
import { gfm } from 'micromark-extension-gfm';
import { gfmFromMarkdown } from 'mdast-util-gfm';

type MarkdownNode = ReturnType<typeof fromMarkdown>['children'][number];

/** Inspect authored Markdown without interpreting code blocks as instructions or prose. */
export function structureErrors(file: string, source: string): string[] {
  const tree = fromMarkdown(source, { extensions: [gfm()], mdastExtensions: [gfmFromMarkdown()] });
  const errors: string[] = [];
  const titles = tree.children.filter(node => node.type === 'heading' && node.depth === 1);
  if (titles.length !== 1 || tree.children.find(node => node.type !== 'html') !== titles[0]) {
    errors.push(`${file}: start with exactly one level-one title`);
  }
  function visit(node: MarkdownNode): void {
    if (node.type === 'paragraph' && node.position && node.position.start.line !== node.position.end.line) {
      errors.push(`${file}:${node.position.start.line}: keep each prose paragraph on one physical line`);
    }
    if (node.type === 'heading' && node.children.length === 0) {
      errors.push(`${file}:${node.position?.start.line}: heading must have a title`);
    }
    // Quoted material preserves its original line breaks; code and tables are not prose paragraphs.
    if (node.type === 'blockquote') return;
    if ('children' in node) for (const child of node.children) visit(child);
  }
  for (const node of tree.children) visit(node);
  return errors;
}

/** Keep release operations out of pages written for product users. */
export function audienceBoundaryErrors(files: Record<string, string>): string[] {
  const errors: string[] = [];
  const internalReleaseMarkers = [
    'corepack pnpm release:',
    'corepack pnpm github:pack',
    'release-set.json',
    '.agents/skills/wombat-release',
    'reference/distribution',
  ];
  for (const [file, source] of Object.entries(files)) {
    if (file !== 'README.md' && file !== 'README.zh-CN.md' && !file.startsWith('docs/guides/')) continue;
    for (const marker of internalReleaseMarkers) {
      if (source.includes(marker)) errors.push(`${file}: user documentation contains internal release material: ${marker}`);
    }
  }
  return errors;
}

/** Keep every directory's default README in English, with a separate Chinese entry. */
export function readmePairErrors(pairs: Array<{ zh: string; en: string }>): string[] {
  return pairs.flatMap(({ zh, en }) => {
    if (![zh, en].some(file => /^README(?:\.[^/]+)?\.md$/.test(path.posix.basename(file)))) return [];
    return path.posix.basename(en) === 'README.md'
      && path.posix.basename(zh) === 'README.zh-CN.md'
      && path.posix.dirname(en) === path.posix.dirname(zh)
      ? [] : [`${en} / ${zh}: use English README.md and Chinese README.zh-CN.md in the same directory`];
  });
}

function main(): void {
  const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
  const manifest = JSON.parse(readFileSync(path.join(root, 'scripts/doc-i18n.manifest.json'), 'utf8')) as {
    pairs: Array<{ zh: string; en: string }>;
  };
  const budgets = JSON.parse(readFileSync(path.join(root, 'scripts/doc-budgets.json'), 'utf8')) as Record<string, number>;
  const files = new Set([
    ...manifest.pairs.flatMap(pair => [pair.zh, pair.en]),
    ...Object.keys(budgets).filter(file => path.basename(file) === 'AGENTS.md'),
  ]);
  const sources = Object.fromEntries([...files].map(file => [file, readFileSync(path.join(root, file), 'utf8')]));
  const errors = [
    ...readmePairErrors(manifest.pairs),
    ...Object.entries(sources).flatMap(([file, source]) => structureErrors(file, source)),
    ...audienceBoundaryErrors(sources),
  ];
  if (errors.length) {
    for (const error of errors) console.error(error);
    process.exitCode = 1;
  } else console.log(`Documentation structure passed: ${files.size} authored files.`);
}

if (process.argv[1] && pathToFileURL(path.resolve(process.argv[1])).href === import.meta.url) main();
