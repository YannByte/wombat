import { parseArgs } from 'node:util';
import { CoreError } from '../client/src/errors.ts';
import { runHarnessTask } from '../client/src/node/codex/task-trial.ts';

const controller = new AbortController();
const cancel = () => controller.abort();
process.once('SIGINT', cancel);
process.once('SIGTERM', cancel);

try {
  const { values } = parseArgs({
    args: process.argv.slice(2).filter((arg, index) => index !== 0 || arg !== '--'),
    options: {
      manifest: { type: 'string' }, output: { type: 'string' }, model: { type: 'string' },
      'codex-bin': { type: 'string' }, 'codex-home': { type: 'string' },
      'allow-instruction': { type: 'string', multiple: true },
    },
  });
  if (!values.manifest || !values.output)
    throw new CoreError('TRIAL_ARGUMENT', 'Use --manifest and --output');
  const result = await runHarnessTask(values.manifest, values.output, {
    model: values.model,
    codexHome: values['codex-home'],
    codexBinaryPath: values['codex-bin'],
    allowedInstructionSources: values['allow-instruction'],
    signal: controller.signal,
  });
  console.log(JSON.stringify(result));
} catch (error) {
  if (error instanceof CoreError) console.error(JSON.stringify({ code: error.code, message: error.message, details: error.details }));
  else console.error(JSON.stringify({ code: 'TRIAL_EXECUTION', message: 'Could not open the trial inputs or output directory' }));
  process.exitCode = 1;
} finally {
  process.off('SIGINT', cancel);
  process.off('SIGTERM', cancel);
}
