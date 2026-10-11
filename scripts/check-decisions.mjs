import { readFileSync, readdirSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const notesRoot = path.join(root, 'docs', 'decisions');
const errors = [];
let checked = 0;
const required = {
  proposed: ['## 问题', '## 方案', '## 考虑过的方案', '## 验收条件'],
  implemented: ['## 问题', '## 决定', '## 考虑过的方案', '## 影响与验证'],
  rejected: ['## 问题', '## 方案', '## 考虑过的方案'],
};

function walk(directory) {
  for (const entry of readdirSync(directory, { withFileTypes: true })) {
    const file = path.join(directory, entry.name);
    if (entry.isDirectory()) { walk(file); continue; }
    if (!entry.name.endsWith('.md') || entry.name.endsWith('.en.md') || ['README.md', 'AGENTS.md'].includes(entry.name)) continue;
    const relative = path.relative(root, file).split(path.sep).join('/');
    if (relative === 'docs/decisions/README.zh-CN.md') continue;
    const match = /^docs\/decisions\/(proposed|implemented|rejected)\/(architecture|product|process)\/(\d{4}-\d{2}-\d{2})-[a-z0-9]+(?:-[a-z0-9]+)*\.md$/.exec(relative);
    if (!match) { errors.push(`${relative}: invalid lifecycle, class, or dated filename`); continue; }
    checked++;
    const [, lifecycle, , date] = match;
    const parsedDate = new Date(`${date}T00:00:00Z`);
    if (!Number.isFinite(parsedDate.valueOf()) || parsedDate.toISOString().slice(0, 10) !== date) errors.push(`${relative}: invalid date`);
    const lines = readFileSync(file, 'utf8').split(/\r?\n/);
    if (!/^# 决策记录：\S/.test(lines[0])) errors.push(`${relative}: expected a Decision Note title`);
    const status = lines.find(line => line.startsWith('Status: '));
    const expected = `Status: ${lifecycle}`;
    if (lifecycle === 'rejected' ? !/^Status: rejected — \S/.test(status ?? '') : status !== expected) {
      errors.push(`${relative}: status must match ${expected}${lifecycle === 'rejected' ? ' with a reason' : ''}`);
    }
    const headings = lines.filter(line => /^## /.test(line));
    let previous = -1;
    for (const heading of required[lifecycle]) {
      const index = headings.indexOf(heading);
      if (index <= previous) errors.push(`${relative}: missing or out-of-order ${heading}`);
      previous = index;
    }
    if (lifecycle === 'implemented' && headings.some(heading => ['## 方案', '## 验收条件', '## 迁移计划'].includes(heading))) {
      errors.push(`${relative}: implemented notes describe shipped decisions, not proposal sections`);
    }
  }
}

walk(notesRoot);
if (errors.length) {
  for (const error of errors) console.error(error);
  process.exitCode = 1;
} else {
  console.log(`Decision Note format passed: ${checked} active note(s).`);
}
