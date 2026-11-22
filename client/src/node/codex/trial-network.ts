import { execFile } from 'node:child_process';
import { isIP } from 'node:net';
import { promisify } from 'node:util';

const run = promisify(execFile);
const keys = ['HTTP_PROXY', 'HTTPS_PROXY', 'NO_PROXY'] as const;

export function systemTrialProxy(settings: string): NodeJS.ProcessEnv {
  const fields = new Map(
    settings.split('\n').map(line => line.match(/^  ([A-Za-z]+)\s*:\s*(\S+)$/))
      .flatMap(match => match ? [[match[1], match[2]]] : []),
  );
  const env: NodeJS.ProcessEnv = {};
  for (const [prefix, variable] of [['HTTP', 'HTTP_PROXY'], ['HTTPS', 'HTTPS_PROXY']]) {
    const hostname = fields.get(prefix + 'Proxy');
    const rawPort = fields.get(prefix + 'Port');
    const port = rawPort && /^\d{1,5}$/.test(rawPort) ? Number(rawPort) : 0;
    if (fields.get(prefix + 'Enable') !== '1' || !hostname || !port || port > 65535) continue;
    const address = isIP(hostname);
    if (!address && !/^[a-zA-Z0-9](?:[a-zA-Z0-9.-]{0,251}[a-zA-Z0-9])?$/.test(hostname)) continue;
    env[variable] = `http://${address === 6 ? '[' + hostname + ']' : hostname}:${port}`;
  }
  if (env.HTTP_PROXY || env.HTTPS_PROXY) {
    const exceptions = settings.match(/^  ExceptionsList\s*:\s*<array>\s*\{([^}]*)\}/m)?.[1] ?? '';
    const safe = [...exceptions.matchAll(/^\s+\d+\s*:\s*([a-zA-Z0-9.*:/_-]+)\s*$/gm)].map(match => match[1]);
    env.NO_PROXY = [...new Set(['localhost', '127.0.0.1', '::1', ...safe])].join(',');
  }
  return env;
}

/** Use the host's existing manual proxy only for the model process. */
export async function trialNetwork(signal: AbortSignal): Promise<NodeJS.ProcessEnv> {
  const explicit = Object.fromEntries(keys.flatMap(key => {
    const value = process.env[key] ?? process.env[key.toLowerCase()];
    return value ? [[key, value]] : [];
  }));
  if (process.platform !== 'darwin' || explicit.HTTP_PROXY || explicit.HTTPS_PROXY) return explicit;
  try {
    const { stdout } = await run('/usr/sbin/scutil', ['--proxy'], { timeout: 2000, maxBuffer: 32 * 1024, signal });
    return { ...systemTrialProxy(stdout), ...explicit };
  } catch {
    signal.throwIfAborted();
    return explicit;
  }
}
