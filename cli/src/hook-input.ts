import {CoreError} from '@wombat/client';
import {receiveCodexHook} from '@wombat/client/node';
import {t} from '@wombat/client/locale';
/** Advisory Hooks always exit successfully and emit no stdout, including failures. */
export async function runHookCli(argv: string[]): Promise<number> {
  if (argv.length !== 1 || argv[0] !== 'codex') throw new CoreError('INVALID_ARGUMENT', t('cli.config.invalid', { value: 'system hook codex' }));
  const controller = new AbortController(), timer = setTimeout(() => controller.abort(), 4000); let bytes = 0; const chunks: Buffer[] = [];
  const abortInput = () => process.stdin.destroy(); controller.signal.addEventListener('abort', abortInput, { once: true });
  try {
    for await (const chunk of process.stdin) { const buffer = Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk); bytes += buffer.length; if (bytes > 64 * 1024) throw new CoreError('INPUT_LIMIT', 'Hook input limit'); chunks.push(buffer); }
    if (controller.signal.aborted) return 0;
    const body: unknown = JSON.parse(Buffer.concat(chunks).toString('utf8'));
    await receiveCodexHook(body, { signal: controller.signal });
  } catch { process.stderr.write(t('collection.hookFailed') + '\n'); }
  finally { clearTimeout(timer); controller.signal.removeEventListener('abort', abortInput); }
  return 0;
}
