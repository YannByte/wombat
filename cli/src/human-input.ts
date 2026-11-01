import path from 'node:path';
import {stat, access} from 'node:fs/promises';
import {constants} from 'node:fs';
import {createInterface} from 'node:readline/promises';
import {CoreError} from '@wombat/client';
import {t} from '@wombat/client/locale';

export function invalidInput(value: string): never {
  throw new CoreError('INVALID_ARGUMENT', t('cli.config.invalid', {value}));
}
export function commandLine(args:readonly string[]):string {
  const quote=(value:string)=>/^[\w./:-]+$/.test(value)?value:"'"+value.replaceAll("'",process.platform==='win32'?"''":"'\\''")+"'";
  return ['wombat',...args].map(quote).join(' ');
}
export function parseOptions(argv: readonly string[], valued: readonly string[], booleans: readonly string[] = [], repeated: readonly string[] = []) {
  const values = new Map<string, string[]>(), flags = new Set<string>(), positionals: string[] = [];
  for (let i = 0; i < argv.length; i++) {
    const arg = argv[i];
    if(arg==='--'){positionals.push(...argv.slice(i+1));break;}
    if (!arg.startsWith('-')) {positionals.push(arg); continue;}
    const [name, inline] = arg.split(/=(.*)/s);
    if (['--help', '-h', '--json', '--no-input', ...booleans].includes(name)) {
      if (inline !== undefined || flags.has(name)) invalidInput(name);
      flags.add(name); continue;
    }
    if (!valued.includes(name) || values.has(name) && !repeated.includes(name)) invalidInput(name);
    const value = inline ?? argv[++i];
    if (!value || value.startsWith('--') || value.includes('\0')) invalidInput(name);
    values.set(name, [...values.get(name) ?? [], value]);
  }
  return {values, flags, positionals, get: (key: string) => values.get(key)?.[0], all: (key: string) => values.get(key) ?? [], help: flags.has('--help') || flags.has('-h'), json: flags.has('--json')};
}
export function pageNumber(value: string | undefined, fallback: number, minimum = 0, maximum = Number.MAX_SAFE_INTEGER): number {
  if (value === undefined) return fallback;
  const n = Number(value);
  if (!/^\d+$/.test(value) || !Number.isSafeInteger(n) || n < minimum || n > maximum) invalidInput(value);
  return n;
}
export function canPrompt(args: readonly string[]): boolean {
  return Boolean(process.stdin.isTTY && process.stdout.isTTY && !args.includes('--json') && !args.includes('--no-input'));
}
export async function ask(question: string, signal: AbortSignal): Promise<string> {
  const timeout = AbortSignal.timeout(120_000), combined = AbortSignal.any([signal, timeout]);
  const rl = createInterface({input: process.stdin, output: process.stderr});
  try {
    const answer = await rl.question(question, {signal: combined});
    if (Buffer.byteLength(answer) > 4096) invalidInput('input');
    return answer.trim();
  } catch (error) {
    if (signal.aborted) throw new CoreError('CANCELLED', t('common.cancelled'));
    if (timeout.aborted) throw new CoreError('TIMEOUT', t('cli.human.inputTimeout'));
    throw error;
  } finally {rl.close();}
}
export async function projectDirectory(value: string | undefined, args: readonly string[], signal: AbortSignal): Promise<string> {
  let project = value ? path.resolve(value) : process.cwd();
  if (!value) {
    const markers = ['.git', 'AGENTS.md', 'package.json', 'Cargo.toml', 'pyproject.toml', 'go.mod'];
    const found = await Promise.all(markers.map(name => access(path.join(project, name)).then(() => true, () => false)));
    if (!found.some(Boolean)) {
      if (!canPrompt(args)) invalidInput(t('cli.human.projectRequired'));
      const answer = await ask(t('cli.human.projectQuestion'), signal);
      if (!answer) throw new CoreError('CANCELLED', t('common.cancelled'));
      project = path.resolve(answer);
    }
  }
  const info = await stat(project).catch(() => undefined);
  if (!info?.isDirectory()) invalidInput(project);
  await access(project,constants.R_OK|constants.X_OK).catch(() => invalidInput(project));
  return project;
}
export async function withCancellation<T>(run: (signal: AbortSignal) => Promise<T>): Promise<T> {
  const controller = new AbortController(), stop = () => controller.abort();
  process.once('SIGINT', stop); process.once('SIGTERM', stop);
  try {return await run(controller.signal);}
  finally {process.off('SIGINT', stop); process.off('SIGTERM', stop);}
}
