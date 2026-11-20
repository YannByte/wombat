import { parseArgs } from 'node:util';
import { CoreError } from '../client/src/errors.ts';
import { verifyHarnessTrial } from '../client/src/node/codex/trial-probe.ts';

const controller = new AbortController();
const cancel = () => controller.abort();
process.once('SIGINT', cancel);
process.once('SIGTERM', cancel);

try {
  const { values } = parseArgs({
    args: process.argv.slice(2).filter((arg, index) => index !== 0 || arg !== '--'),
    options: { 'codex-bin': { type: 'string' } },
  });
  if (!values['codex-bin']) throw new CoreError('TRIAL_PROBE_BINARY', 'Use --codex-bin with the absolute executable path');
  const report = await verifyHarnessTrial(values['codex-bin'], controller.signal);
  console.log(JSON.stringify(report));
} catch (error) {
  const failure = error instanceof CoreError
    ? { code: error.code, message: error.message, details: error.details }
    : { code: 'TRIAL_PROBE_FAILED', message: 'Trial qualification failed' };
  console.error(JSON.stringify({ status: 'failed', ...failure }));
  process.exitCode = 1;
} finally {
  process.off('SIGINT', cancel);
  process.off('SIGTERM', cancel);
}
