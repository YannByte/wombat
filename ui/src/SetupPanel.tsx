import { useEffect, useId, useState } from 'react';
import type { CollectionResult, SetupResult, UsageClient, UsageResult } from '@wombat/client';
import { t, type MessageKey } from '@wombat/client/locale';
import { Modal, timestamp } from './components.js';
import { setupRegistrationLabel } from './setup-state.js';
import { ServicePanel } from './ServicePanel.js';
const skillStates = {
  available: 'skill.available',
  ambiguous: 'skill.ambiguous',
  disabled: 'skill.disabled',
  missing: 'skill.missing',
  unavailable: 'skill.unavailable',
  selection_changed: 'skill.unavailable',
} as const satisfies Record<SetupResult['discovery']['status'], MessageKey>;
const collectionStates = {
  logs_only: 'collection.logsOnly',
  waiting: 'collection.waiting',
  received: 'collection.received',
  paused: 'collection.paused',
} as const satisfies Record<CollectionResult['state'], MessageKey>;
function CopyText({ value, question = false }: { value: string; question?: boolean }) {
  const label = useId(),
    [copy, setCopy] = useState<{ value: string; status: 'setup.copied' | 'setup.copyFailed' }>();
  const status = copy?.value === value ? copy.status : undefined;
  return (
    <div className="setup-copy">
      <code id={label}>{value}</code>
      <button
        aria-describedby={label}
        onClick={() => {
          void (
            navigator.clipboard ? navigator.clipboard.writeText(value) : Promise.reject(new Error())
          ).then(
            () => setCopy({ value, status: 'setup.copied' }),
            () => setCopy({ value, status: 'setup.copyFailed' }),
          );
        }}
      >
        {t(question ? 'setup.copyQuestion' : 'setup.copyCommand')}
      </button>
      <span role="status">{status ? t(status) : ''}</span>
    </div>
  );
}
export function SetupPanel({
  client,
  project,
  sourceInstanceId,
  overview,
  timezone,
  initial = 'mode',
  close,
}: {
  client: UsageClient;
  project?: string;
  sourceInstanceId?: string;
  overview?: UsageResult;
  timezone: string;
  initial?: 'mode' | 'use';
  close: () => void;
}) {
  const [step, setStep] = useState<'mode' | 'connect' | 'history' | 'use'>(initial);
  const [setup, setSetup] = useState<SetupResult>(),
    [collection, setCollection] = useState<CollectionResult>(),
    [error, setError] = useState(false),
    [busy, setBusy] = useState(false),
    [revision, setRevision] = useState(0);
  useEffect(() => {
    const controller = new AbortController();
    let current = true;
    setBusy(true);
    setError(false);
    setSetup(undefined);
    setCollection(undefined);
    const load = async () => {
      const [setupResult, collectionResult] = await Promise.allSettled([
        client.setup?.({ project }, { signal: controller.signal }),
        client.collection?.(
          { action: 'status', project, sourceInstanceId },
          { signal: controller.signal },
        ),
      ]);
      if (!current) return;
      if (setupResult.status === 'fulfilled') {
        setSetup(setupResult.value);
        if (setupResult.value?.errorCodes.length) setError(true);
      } else setError(true);
      if (collectionResult.status === 'fulfilled') setCollection(collectionResult.value);
      else setError(true);
      setBusy(false);
    };
    void load();
    return () => {
      current = false;
      controller.abort();
    };
  }, [client, project, sourceInstanceId, revision]);
  const [eventsOpen, setEventsOpen] = useState(false),
    [eventsAfter, setEventsAfter] = useState<number>(),
    [events, setEvents] = useState<CollectionResult>(),
    [eventsError, setEventsError] = useState(false),
    [eventsBusy, setEventsBusy] = useState(false);
  useEffect(() => {
    if (!eventsOpen || !client.collection) return;
    const controller = new AbortController();
    let active = true;
    setEventsBusy(true);
    setEventsError(false);
    setEvents(undefined);
    void client
      .collection(
        { action: 'events', project, sourceInstanceId, after: eventsAfter, limit: 50 },
        { signal: controller.signal },
      )
      .then(
        (result) => {
          if (active) setEvents(result);
        },
        () => {
          if (active) setEventsError(true);
        },
      )
      .finally(() => {
        if (active) setEventsBusy(false);
      });
    return () => {
      active = false;
      controller.abort();
    };
  }, [client, project, sourceInstanceId, eventsOpen, eventsAfter, revision]);
  const hookStatus = setupRegistrationLabel(project, setup?.hooks);
  const shellPath = (value: string) => "'" + value.replaceAll("'", "'\\''") + "'";
  const instances = setup?.discovery.instances.filter((i) => i.enabled) ?? [];
  return (
    <Modal title={t('setup.title')} onClose={close}>
      <div className="setup-layout">
        <nav className="setup-steps" aria-label={t('setup.steps')}>
          {(['mode', 'connect', 'history', 'use'] as const).map((s, index) => (
            <button
              key={s}
              className={step === s ? 'active' : ''}
              aria-current={step === s ? 'step' : undefined}
              onClick={() => setStep(s)}
            >
              <span>{index + 1}</span>
              {t(`setup.step.${s}`)}
            </button>
          ))}
        </nav>
        <section className="setup-content" aria-busy={busy}>
          {error && (
            <p role="alert" className="read-notice">
              {t('setup.failed')}
            </p>
          )}
          {step === 'mode' ? (
            <>
              <h3>{t('setup.step.mode')}</h3>
              <p>{t('setup.modeNote')}</p>
              <dl className="setup-status">
                <dt>{t('setup.currentMode')}</dt>
                <dd>
                  {collection
                    ? t(collection.mode === 'hooks' ? 'setup.liveMode' : 'collection.logsOnly')
                    : t('setup.unchecked')}
                </dd>
              </dl>
              <div className="setup-option">
                <h4>{t('setup.liveMode')}</h4>
                <p>{t('setup.liveNote')}</p>
                <CopyText value="wombat collection mode hooks --json" />
              </div>
              <div className="setup-option">
                <h4>{t('collection.logsOnly')}</h4>
                <p>{t('setup.logsNote')}</p>
                <CopyText value="wombat collection mode logs --json" />
              </div>
              <button onClick={() => setStep('connect')}>{t('setup.continue')}</button>
            </>
          ) : step === 'connect' ? (
            <>
              <h3>{t('setup.step.connect')}</h3>
              <dl className="setup-status">
                <dt>{t('setup.localService')}</dt>
                <dd>{t('setup.serviceAvailable')}</dd>
                <dt>Wombat</dt>
                <dd>{setup?.runtimeVersion ?? t('setup.unchecked')}</dd>
                <dt>Codex</dt>
                <dd>{setup?.nativeVersion ?? t('setup.unchecked')}</dd>
                <dt>Skill</dt>
                <dd>{setup ? t(skillStates[setup.discovery.status]) : t('setup.unchecked')}</dd>
                <dt>{t('setup.registration')}</dt>
                <dd>{t(hookStatus)}</dd>
                <dt>{t('setup.receipt')}</dt>
                <dd>{collection ? t(collectionStates[collection.state]) : t('setup.unchecked')}</dd>
              </dl>
              {setup?.runtimeChecks?.map((check) => (
                <p key={check.path}>
                  {t('setup.runtimeCheck', {
                    version: check.pluginVersion ?? '—',
                    status: t(`setup.runtime.${check.status}`),
                  })}
                </p>
              ))}
              {setup?.errorCodes.map((code) => (
                <p className="note" key={code}>
                  {t('setup.errorLine', { code })}
                </p>
              ))}
              <p>{t('setup.logsReadyNote')}</p>
              <p>{t('setup.registrationNote')}</p>
              <p>{t('setup.receiptScope')}</p>
              {!project && <p>{t('setup.projectNote')}</p>}
              <p>{t('setup.installNote')}</p>
              {setup?.marketplacePath && (
                <>
                  <CopyText
                    value={`codex plugin marketplace add ${shellPath(setup.marketplacePath)}`}
                  />
                  <CopyText value="codex plugin add wombat@wombat-local" />
                </>
              )}
              <p>{t('setup.hookInstallNote')}</p>
              {setup?.marketplacePath && (
                <CopyText value="codex plugin add wombat-collection@wombat-local" />
              )}
              <p>{t('setup.nativeTrust')}</p>
              <button disabled={busy} onClick={() => setRevision((v) => v + 1)}>
                {t('setup.recheck')}
              </button>
              {setup && (
                <p className="muted">
                  {t('setup.checkedAt', { time: timestamp(setup.checkedAt, timezone) })}
                </p>
              )}
              {collection?.lastReceivedAt && (
                <p>
                  {t('collection.lastReceipt', {
                    time: timestamp(collection.lastReceivedAt, timezone),
                  })}
                </p>
              )}
              {collection && (
                <p>
                  {t('setup.buffer', {
                    buffered: collection.buffered,
                    gaps: collection.gaps,
                    unknown: collection.identityUnknown,
                  })}
                </p>
              )}
              <details onToggle={(e) => setEventsOpen(e.currentTarget.open)}>
                <summary>{t('setup.events')}</summary>
                <p>{t('setup.eventNote')}</p>
                {eventsBusy && <p role="status">{t('setup.preparing')}</p>}
                {eventsError && <p role="alert">{t('setup.failed')}</p>}
                {events && (
                  <>
                    <div className="setup-events">
                      <table>
                        <thead>
                          <tr>
                            <th>{t('setup.eventKind')}</th>
                            <th>{t('setup.eventSession')}</th>
                            <th>{t('setup.eventAssociation')}</th>
                            <th>{t('setup.receipt')}</th>
                          </tr>
                        </thead>
                        <tbody>
                          {events.events.map((e) => (
                            <tr key={e.sequence}>
                              <td>
                                <code>{e.observation.kind}</code>
                              </td>
                              <td>
                                <code>{e.observation.sessionId}</code>
                                <br />
                                <code>{e.observation.turnId}</code>
                              </td>
                              <td>
                                {t(`setup.association.${e.association.state}`)}
                                {e.association.threadId && (
                                  <>
                                    <br />
                                    <code>{e.association.threadId}</code>
                                  </>
                                )}
                                {e.association.sourceEpoch && (
                                  <>
                                    <br />
                                    <small>{e.association.sourceEpoch}</small>
                                  </>
                                )}
                              </td>
                              <td>{timestamp(e.receivedAt, timezone)}</td>
                            </tr>
                          ))}
                        </tbody>
                      </table>
                    </div>
                    {!events.events.length && <p>{t('setup.noEvents')}</p>}
                    <div className="controls">
                      <button
                        disabled={eventsAfter === undefined || eventsBusy}
                        onClick={() => setEventsAfter(undefined)}
                      >
                        {t('setup.firstEvents')}
                      </button>
                      <button
                        disabled={events.nextAfter == null || eventsBusy}
                        onClick={() => {
                          if (events.nextAfter != null) setEventsAfter(events.nextAfter);
                        }}
                      >
                        {t('webui.next')}
                      </button>
                    </div>
                  </>
                )}
              </details>
              <p>{t('setup.pauseNote')}</p>
              <CopyText value="wombat collection pause --json" />
              <CopyText value="wombat collection resume --json" />
            </>
          ) : step === 'history' ? (
            <>
              <h3>{t('setup.step.history')}</h3>
              <p>{t('setup.historyNote')}</p>
              <dl className="setup-status">
                <dt>{t('setup.history')}</dt>
                <dd>
                  {!overview
                    ? t('setup.unchecked')
                    : overview.freshness?.initialScan
                      ? t('setup.preparing')
                      : overview.quality.status === 'partial'
                        ? t('setup.partial')
                        : overview.summary.measurementCount === 0
                          ? t('setup.emptyScope')
                          : t('setup.historyAvailable')}
                </dd>
                <dt>{t('setup.view')}</dt>
                <dd>{overview?.snapshotRef.snapshotId ?? t('setup.unchecked')}</dd>
              </dl>
              <p>{t('setup.refreshNote')}</p>
              <CopyText value="wombat usage --json" />
              <p>{t('setup.backgroundNote')}</p>
              <ServicePanel client={client} />
              <CopyText value="wombat service status --json" />
              <button onClick={close}>{t('setup.viewData')}</button>
            </>
          ) : (
            <>
              <h3>{t('setup.step.use')}</h3>
              <p>{t('setup.skillNote')}</p>
              {setup?.discovery.status === 'available' && instances.length === 1 ? (
                <CopyText value={`$${instances[0].name}`} />
              ) : (
                <p>{setup ? t(skillStates[setup.discovery.status]) : t('setup.unchecked')}</p>
              )}
              {setup?.discovery.status === 'ambiguous' && (
                <ul>
                  {instances.map((i) => (
                    <li key={i.path}>
                      <code>${i.name}</code>
                      <br />
                      <span>{i.path}</span>
                    </li>
                  ))}
                </ul>
              )}
              <div className="setup-example">
                {(['setup.exampleUsage', 'setup.exampleConfig', 'setup.exampleWeb'] as const).map(
                  (key) => (
                    <CopyText
                      key={key}
                      question
                      value={`${setup?.discovery.status === 'available' && instances.length === 1 ? '$' + instances[0].name + '\n' : ''}${t(key)}${project ? '\n' + t('setup.exampleProject', { project }) : ''}`}
                    />
                  ),
                )}
              </div>
              <p>{t('setup.conversationNote')}</p>
              <button onClick={close}>{t('setup.viewData')}</button>
            </>
          )}
        </section>
      </div>
    </Modal>
  );
}
