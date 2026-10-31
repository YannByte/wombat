import test from 'node:test';
import assert from 'node:assert/strict';
import { mkdtempSync, mkdirSync, rmSync, writeFileSync } from 'node:fs';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { spawnSync } from 'node:child_process';
import { checkProject, dictionaryErrors, findCopy, readDictionary } from './check-copy.ts';

const dictionary = { 'state.complete': '已完成', 'state.failed': '失败', 'state.running': '运行中', 'state.unknown': '未知' };
const english = { 'state.complete': 'Completed', 'state.failed': 'Failed', 'state.running': 'Running', 'state.unknown': 'Unknown' };
function fixture(run: (root: string) => void): void {
  const root = mkdtempSync(path.join(tmpdir(), 'wombat-i18n-check-'));
  try {
    for (const dir of ['client/src/locale', 'cli/src', 'cli/src']) mkdirSync(path.join(root, dir), { recursive: true });
    writeFileSync(path.join(root, 'package.json'), '{"type":"module"}');
    writeFileSync(path.join(root, 'client/src/locale/index.ts'), `import {zh} from './zh.js';
export type MessageKey = keyof typeof zh;
export type Translate = <K extends MessageKey>(key: K) => string;
export declare const t: Translate;
export declare const locale: {t: Translate};
export declare function labels<K extends string>(keys: Record<K, MessageKey>): Record<K, string>;`);
    saveDictionaries(root);
    run(root);
  } finally { rmSync(root, { recursive: true, force: true }); }
}
function saveDictionaries(root: string, zh: Record<string, string> = dictionary, en: Record<string, string> = english): void {
  writeFileSync(path.join(root, 'client/src/locale/zh.ts'), `export const zh = ${JSON.stringify(zh)} as const;`);
  writeFileSync(path.join(root, 'client/src/locale/en.ts'), `export const en = ${JSON.stringify(en)} as const;`);
}
const script = fileURLToPath(new URL('./check-copy.ts', import.meta.url));

test('dictionary validation retains complete languages and rejects missing or malformed entries',()=>{
  assert.deepEqual(dictionaryErrors(dictionary,english),[]);
  assert.ok(dictionaryErrors(dictionary,{}).length>0);
  assert.ok(readDictionary('en.ts','export const en = {"x":"a", "x":"b"};').errors.length>0);
});
test('CLI presentation sinks require locale copy and do not treat source values as translated prose',()=>{
  assert.ok(findCopy('cli.ts','const notice={message:"Please retry"};').length>0);
  assert.deepEqual(findCopy('cli.ts','const source={originalText:"原文 Keep unchanged", status:"running"};'),[]);
  assert.ok(findCopy('cli.ts','const retry="Please retry";const notice={message:retry};').length>0);
});
test('native result keys resolve without built artifacts and fail for missing or open status keys',()=>fixture(root=>{
  mkdirSync(path.join(root,'client/src/node'),{recursive:true});
  const native=path.join(root,'client/src/node/index.ts'),source=path.join(root,'cli/src/render.ts');
  writeFileSync(native,"export declare function createClient():{query():Promise<{status:'complete'|'failed'}>};");
  writeFileSync(source,"import {createClient} from '@wombat/client/node';import {t} from '@wombat/client/locale';export async function render(){const result=await createClient().query();return t(`state.${result.status}`);}");
  assert.deepEqual(checkProject(root),[]);
  writeFileSync(native,"export declare function createClient():{query():Promise<{status:'missing'}>};");
  assert.ok(checkProject(root).some(e=>e.includes('state.missing')));
  writeFileSync(native,"export declare function createClient():{query():Promise<{status:string}>};");
  assert.ok(checkProject(root).some(e=>e.includes('not finite')));
}));
test('executed CLI copy check rejects untranslated copy with a nonzero exit',()=>fixture(root=>{
  const source=path.join(root,'cli/src/render.ts');
  writeFileSync(source,"import {t} from '@wombat/client/locale';export function render(){return {message:t('state.running')};}");
  const run=()=>spawnSync(process.execPath,[script,root],{encoding:'utf8',timeout:15000,maxBuffer:1024*1024});
  assert.equal(run().status,0);
  writeFileSync(source,'export const notice={message:"Please retry"};');
  const invalid=run();assert.equal(invalid.status,1);assert.match(invalid.stderr,/locale dictionary/);
}));
