import { spawnSync } from 'node:child_process';
import { createHash } from 'node:crypto';
import { readFileSync } from 'node:fs';
import { performance } from 'node:perf_hooks';

export const sha256 = (data: Buffer | string): string => createHash('sha256').update(data).digest('hex');
export const fileSha256 = (path: string): string => sha256(readFileSync(path));

export function option(name: string, fallback?: string): string {
  const index = process.argv.indexOf(name);
  if (index < 0) {
    if (fallback !== undefined) return fallback;
    throw new Error(`Missing ${name}`);
  }
  const value = process.argv[index + 1];
  if (!value || value.startsWith('--')) throw new Error(`Missing value for ${name}`);
  return value;
}

export function positiveInteger(name: string, fallback: number): number {
  const value = Number(option(name, String(fallback)));
  if (!Number.isSafeInteger(value) || value < 1) throw new Error(`${name} must be a positive integer`);
  return value;
}

export function run(program: string, args: string[], env = process.env, timeout = 120_000, input?: string): { stdout: string; stderr: string; wallMs: number } {
  const start = performance.now();
  const result = spawnSync(program, args, {input, env, encoding: 'utf8', timeout, maxBuffer: 64 * 1024 * 1024 });
  if (result.error || result.status !== 0) {
    throw new Error(`${program} ${args[0] ?? ''} failed (${result.status}): ${result.error?.message ?? ''} ${result.stdout?.slice(-1500) ?? ''} ${result.stderr?.slice(-1500) ?? ''}`);
  }
  return { stdout: result.stdout, stderr: result.stderr, wallMs: Math.round((performance.now() - start) * 1000) / 1000 };
}

export function moneyMicros(value: string): bigint {
  if (!/^\d+(?:\.\d{1,6})?$/.test(value)) throw new Error(`Invalid USD amount: ${value}`);
  const [whole, fraction = ''] = value.split('.');
  return BigInt(whole) * 1_000_000n + BigInt(fraction.padEnd(6, '0'));
}

export function formatMicros(value: bigint): string {
  return `${value / 1_000_000n}.${String(value % 1_000_000n).padStart(6, '0')}`;
}
