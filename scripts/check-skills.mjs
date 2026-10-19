import { existsSync, readFileSync, readdirSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { fromMarkdown } from 'mdast-util-from-markdown';
import { skillNames, skillPresentation } from './build-skill.ts';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const skillRoots = ['.agents/skills', 'plugin/skills'];
const errors = [];
let checked = 0;

try {
  const names=skillNames(root);
  const directories=readdirSync(path.join(root,'plugin/skills'),{withFileTypes:true}).filter(e=>e.isDirectory()).map(e=>e.name).sort();
  if(JSON.stringify([...names].sort())!==JSON.stringify(directories))errors.push('plugin/package.json: declared Skills must match the shipped directories');
  for(const name of names)skillPresentation(path.join(root,'plugin/skills',name,'agents/openai.yaml'),'$wombat:'+name);
}catch(error){errors.push(error.message);}

// Check authored resource links too: moving a procedure must not strand its callers.
function resourceLinks(directory) {
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    const file = path.join(directory, entry.name);
    if (entry.isDirectory()) resourceLinks(file);
    else if (entry.isFile() && entry.name.endsWith('.md')) {
      const targets = new Set();
      const visit = node => {
        if (node.type === 'link' || node.type === 'definition') targets.add(node.url);
        if (node.children) for (const child of node.children) visit(child);
      };
      visit(fromMarkdown(readFileSync(file, 'utf8')));
      for (const target of targets) {
        if (/^(?:[a-z][a-z0-9+.-]*:|#)/i.test(target)) continue;
        const relative = target.split('#')[0];
        if (!relative) continue;
        if (!existsSync(path.resolve(path.dirname(file), relative))) {
          const relativeFile = path.relative(root, file).split(path.sep).join('/');
          errors.push(`${relativeFile}: missing linked resource ${target}`);
        }
      }
    }
  }
}

for (const relativeRoot of skillRoots) {
  const skillsRoot = path.join(root, relativeRoot);
  if (!existsSync(skillsRoot)) continue;
  for (const entry of readdirSync(skillsRoot, { withFileTypes: true })) {
    if (!entry.isDirectory()) continue;
    const relative = `${relativeRoot}/${entry.name}/SKILL.md`;
    const file = path.join(skillsRoot, entry.name, 'SKILL.md');
    if (!existsSync(file)) {
      errors.push(`${relative}: missing skill instructions`);
      continue;
    }
    checked++;
    resourceLinks(path.dirname(file));
    const lines = readFileSync(file, 'utf8').split(/\r?\n/);
    const end = lines.indexOf('---', 1);
    if (lines[0] !== '---' || end < 2) {
      errors.push(`${relative}: expected closed YAML frontmatter at the start`);
      continue;
    }
    const fields = new Map();
    for (const line of lines.slice(1, end)) {
      const match = /^([a-z][a-z-]*):\s*(.*)$/.exec(line);
      if (!match) {
        errors.push(`${relative}: frontmatter must use single-line key: value fields`);
        continue;
      }
      if (fields.has(match[1])) errors.push(`${relative}: duplicate ${match[1]} field`);
      fields.set(match[1], match[2].trim().replace(/^(["'])(.*)\1$/, '$2'));
    }
    if (!/^[a-z0-9]+(?:-[a-z0-9]+)*$/.test(entry.name) || fields.get('name') !== entry.name) {
      errors.push(`${relative}: name must be kebab-case and match its directory`);
    }
    if (!fields.get('description')) errors.push(`${relative}: description is required`);
    if (!lines.slice(end + 1).join('\n').trim()) errors.push(`${relative}: instruction body is empty`);
  }
}

if (errors.length) {
  for (const error of errors) console.error(error);
  process.exitCode = 1;
} else {
  console.log(`Skill metadata passed: ${checked} skill(s).`);
}
