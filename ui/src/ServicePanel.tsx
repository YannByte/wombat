import { useEffect, useRef, useState } from 'react';
import type { UsageClient, ServiceRequest, ServiceResult } from '@wombat/client';
import { t } from '@wombat/client/locale';

/** Service lifetime is independent of this panel and its cancellable requests. */
export function ServicePanel({ client }: { client: UsageClient }) {
  const [result, setResult] = useState<ServiceResult>(),
    [error, setError] = useState(false),
    [busy, setBusy] = useState(false);
  const pending = useRef<AbortController | undefined>(undefined);
  const run = (action: ServiceRequest['action']) => {
    pending.current?.abort();
    const controller = new AbortController();
    pending.current = controller;
    setBusy(true);
    setError(false);
    void client
      .service?.({ action }, { signal: controller.signal })
      .then(
        (value) => {
          if (!controller.signal.aborted) setResult(value);
        },
        () => {
          if (!controller.signal.aborted) setError(true);
        },
      )
      .finally(() => {
        if (!controller.signal.aborted) setBusy(false);
      });
  };
  useEffect(() => {
    if (client.service) run('status');
    return () => pending.current?.abort();
  }, [client]);
  if (!client.service) return null;
  const state = !result
    ? t('setup.unchecked')
    : !result.running
      ? t('service.stopped')
      : result.stopping
        ? t('service.stopping')
        : result.paused
          ? t('service.paused')
          : result.preparing
            ? t('service.preparing')
            : t('service.running');
  return (
    <section>
      <h4>{t('service.title')}</h4>
      <p>{t('service.note')}</p>
      <p role="status">
        {state}
        {result?.running
          ? ' · ' + t(result.resident ? 'service.resident' : 'service.onDemand')
          : ''}
      </p>
      {error && <p role="alert">{t('service.failure')}</p>}
      <div className="controls">
        <button disabled={busy} onClick={() => run('status')}>
          {t('service.recheck')}
        </button>
        {result?.controlsAvailable && (
          <button
            disabled={busy || result.stopping}
            onClick={() => run(!result.running ? 'start' : result.paused ? 'resume' : 'pause')}
          >
            {t(
              !result.running
                ? 'service.start'
                : result.paused
                  ? 'service.resume'
                  : 'service.pause',
            )}
          </button>
        )}
      </div>
    </section>
  );
}
