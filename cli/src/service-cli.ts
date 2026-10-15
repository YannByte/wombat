import { CoreError, type ServiceRequest } from '@wombat/client';
import { createNodeClient } from '@wombat/client/node';
import { t } from '@wombat/client/locale';

export async function runServiceCli(argv: string[]): Promise<number> {
  const action = argv[0]?.startsWith('-') ? 'status' : (argv.shift() ?? 'status');
  if (
    argv.some((arg) => !['--json', '--help', '-h'].includes(arg)) ||
    new Set(argv).size !== argv.length ||
    !['status', 'start', 'pause', 'resume', 'stop', 'enable', 'disable'].includes(action)
  )
    throw new CoreError('INVALID_ARGUMENT', t('service.help'));
  if (argv.includes('--help') || argv.includes('-h')) {
    process.stdout.write(t('service.help') + '\n');
    return 0;
  }
  const controller = new AbortController(),
    abort = () => controller.abort();
  process.once('SIGINT', abort);
  process.once('SIGTERM', abort);
  try {
    const result = await createNodeClient().service!(
      { action: action as ServiceRequest['action'] },
      { signal: controller.signal },
    );
    process.stdout.write(
      argv.includes('--json')
        ? JSON.stringify(result) + '\n'
        : t('service.summary', {
            state: !result.running
              ? t('service.stopped')
              : result.stopping
                ? t('service.stopping')
                : result.paused
                  ? t('service.paused')
                  : result.preparing
                    ? t('service.preparing')
                    : t('service.running'),
            startup: result.autoStart ? t('service.enabled') : t('service.disabled'),
          }) + '\n',
    );
    return 0;
  } finally {
    process.off('SIGINT', abort);
    process.off('SIGTERM', abort);
  }
}
