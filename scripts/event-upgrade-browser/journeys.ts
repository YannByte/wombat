import assert from 'node:assert/strict';
import { locale, t, type MessageKey } from '@wombat/client/locale';
import type { Page, Locator, Request } from './playwright.ts';
import { record, shareWhitelist } from './policy.ts';
import { privateBody, title, nativeThread, nativeTurn, type Fixture } from './fixture.ts';

export type Language = 'zh' | 'en';
export interface Case { source: 'production' | 'preview'; language: Language; width: number; journeys: string[] }
type PlainKey = { [K in MessageKey]: Parameters<typeof t<K>> extends [K] ? K : never }[MessageKey];
const label = (key: PlainKey) => t(key);
const region = (page: Page, key: PlainKey) => page.getByRole('region', { name: label(key), exact: true });
const execution = (page: Page) => region(page, 'execution.title');
const uses = (page: Page) => region(page, 'execution.uses');
async function activate(item: Locator) { await item.waitFor(); const deadline=Date.now()+15_000;while(!await item.isEnabled()){assert.ok(Date.now()<deadline,'Button remained disabled');await new Promise(resolve=>setTimeout(resolve,25));}await item.focus(); await item.press('Enter'); }
async function details(page: Page, key: PlainKey) { await activate(page.locator('summary').filter({ hasText: label(key) }).first()); }
async function text(item: Locator, expected: string) { await item.filter({ hasText: expected }).waitFor(); assert.ok((await item.innerText()).includes(expected), `Expected DOM text ${expected}`); }
async function overflow(page: Page) { assert.equal(await page.evaluate<boolean>('document.documentElement.scrollWidth > innerWidth + 1'), false, 'Horizontal document overflow'); }
async function waitCount(page: Page, selector: string, count: number) {
  await page.waitForFunction(({ selector, count }) => document.querySelectorAll(selector).length === count, { selector, count });
}
async function settled(page: Page) {
  await page.waitForFunction(() => !document.querySelector('main [inert]') && !document.querySelector('.task-turns .execution[aria-busy="true"]'));
}
export function pageErrors(page: Page): string[] {
  const errors: string[] = [];
  page.on('pageerror', error => { if (errors.length < 20) errors.push(error.message.slice(0, 1000)); });
  page.on('console', message => {
    if (message.type() !== 'error' || errors.length >= 20) return;
    let location = ''; try { location = new URL(message.location().url).pathname; } catch { /* Browser-origin errors may have no URL. */ }
    errors.push((message.text() + (location ? ` (${location})` : '')).slice(0, 1000));
  });
  return errors;
}
export function timingRequests(page: Page): Record<string, unknown>[] {
  const requests: Record<string, unknown>[] = [];
  page.on('request', (request: Request) => {
    if (request.method() !== 'POST' || new URL(request.url()).pathname !== '/api/timing') return;
    const body = request.postDataJSON();
    // Assertions run in the awaited journey, never escape an event callback.
    if (!record(body)) { requests.push({ invalid: true }); return; }
    if (requests.length >= 1000) { requests[999] = { exceeded: true }; return; } requests.push(body);
  });
  return requests;
}
function bound(requests: Record<string, unknown>[], snapshot: string, thread: string, turn: string, source: string): void {
  assert.ok(requests.length > 0);
  for (const request of requests) {
    if (request.action === 'capabilities') continue;
    assert.equal(request.snapshotId, snapshot); assert.equal(request.threadId, thread); assert.equal(request.turnId, turn);
    assert.ok(!('roots' in request)); assert.ok(!('projectRoots' in request));
    if (request.collection === 'use_records' || request.collection === 'use_objects') {
      assert.ok(record(request.scope)); assert.equal(request.scope.sourceInstanceId, source);
    }
  }
}
async function metric(page: Page, selector: string, name: string): Promise<string> {
  return page.evaluate<string>(`(()=>{const root=document.querySelector(${JSON.stringify(selector)});const dt=[...(root?.querySelectorAll('dt')??[])].find(e=>e.textContent?.trim()===${JSON.stringify(name)});if(!dt)throw Error('Metric not found');return dt.nextElementSibling?.textContent?.trim()??'';})()`);
}
async function share(page: Page, sensitive: string[]) {
  await activate(execution(page).getByRole('button', { name: label('execution.share'), exact: true }));
  const dialog = page.locator('dialog.execution-share'); await dialog.waitFor();
  const shared: unknown = JSON.parse(await dialog.locator('pre').innerText()); shareWhitelist(shared, sensitive);
  await activate(dialog.getByRole('button', { name: label('execution.copy'), exact: true }));
  await page.waitForFunction(expected => document.querySelector('dialog.execution-share [role="status"]')?.textContent === expected, label('execution.copied'));
  const copied = await page.evaluate<string>('navigator.clipboard.readText()'); assert.deepEqual(JSON.parse(copied), shared);
  await page.keyboard.press('Escape'); await dialog.waitFor({ state: 'hidden' });
  assert.equal(await page.evaluate<string>('document.activeElement?.textContent?.trim()??""'), label('execution.share'));
  return shared;
}
export async function product(page: Page, fixture: Fixture, language: Language, width: number): Promise<Case> {
  locale.setLocale(language); await page.setViewportSize({ width, height: 900 });
  await fixture.reset(); const oracle = await fixture.current();
  assert.equal(oracle.time.command.unionMs.value, 50_000); assert.equal(oracle.time.command.sumMs.value, 60_000);
  assert.equal(oracle.time.nativeTtftMs.value, 0); assert.equal(oracle.work.operationCandidates.value, 7);
  const requests = timingRequests(page), errors = pageErrors(page);
  const url = new URL(fixture.product); url.search = new URLSearchParams({ page: 'threads', allTime: '1', timezone: 'UTC', sort: 'tokens', search: 'Synthetic browser task' }).toString();
  const fragment = new URLSearchParams(url.hash.slice(1)); fragment.set('lang', language); url.hash = fragment.toString();
  await page.goto(url.href); await page.locator('.thread-row').first().waitFor();
  await activate(page.locator('.thread-row').first()); await page.locator('.turn[open]').waitFor(); await execution(page).waitFor();
  await settled(page);
  await text(page.locator('.task-detail h2'), title);
  await text(page.locator('.turn[open] > summary .token-number'), '110');
  if (width <= 760) await activate(page.locator('.task-return')); else await page.goBack();
  await page.waitForFunction(() => !new URL(location.href).searchParams.has('thread'));
  await settled(page);
  assert.equal(new URL(page.url()).searchParams.get('search'), 'Synthetic browser task'); assert.equal(new URL(page.url()).searchParams.get('sort'), 'tokens');
  await page.waitForFunction(() => document.activeElement?.classList.contains('thread-row'));
  await activate(page.locator('.thread-row').first()); await execution(page).waitFor();
  await settled(page);

  await details(page, 'execution.distribution'); await text(execution(page), label('execution.concurrent'));
  assert.equal(await metric(page, '.turn[open] .execution', label('execution.category.command')), t('execution.unionSum', { union: '50000 ms', sum: '60000 ms' }));
  await activate(page.locator(width <= 760 ? '.execution-list button' : '.execution-track button').first()); await page.locator('aside.execution-evidence').waitFor();
  assert.equal(await page.evaluate<boolean>('document.activeElement?.matches("aside.execution-evidence")??false'), true);
  await text(page.locator('aside.execution-evidence'), label('execution.selectedRecords'));
  await activate(page.locator('aside.execution-evidence').getByRole('button', { name: label('execution.closeEvidence'), exact: true }));

  const skill = uses(page).locator('article').filter({ hasText: fixture.skill }).first(); await skill.waitFor();
  assert.equal(await metric(page, `section[aria-label=${JSON.stringify(label('execution.uses'))}] article`, label('execution.useCount')), '3');
  await text(uses(page), label('execution.useNote')); await activate(skill.getByRole('button', { name: label('execution.useEvidence'), exact: true }));
  const records = uses(page).locator('aside.execution-evidence'); await records.locator('article').first().waitFor();
  assert.equal(await records.locator('article').count(), 3); await text(records, label('execution.useOutcome.failed'));
  assert.equal(await skill.locator('dd').first().innerText(), '3');
  await activate(records.getByRole('button', { name: label('execution.closeEvidence'), exact: true }));
  assert.equal(await skill.getByRole('button', { name: label('execution.useEvidence'), exact: true }).evaluate(button => button === document.activeElement), true);

  await details(page, 'execution.moreMetrics'); const ttft = await metric(page, '.turn[open] .execution', label('execution.nativeTtft'));
  assert.ok(ttft.startsWith('0 ms')); assert.ok(ttft.includes(label('execution.evidence')));
  for (const key of ['execution.work.addedLines', 'execution.work.removedLines'] as const)
    assert.equal(await page.locator('.work-facts dt').filter({ hasText: label(key) }).count(), 0);
  await details(page, 'execution.work.technical');
  await text(page.locator('.work-facts details'), 'missing_repository_baseline');
  await details(page, 'execution.work.technical');
  const summaryRequest = requests.find(request => request.action === 'summary' && request.privacyProfile === 'local'); assert.ok(summaryRequest);
  assert.equal(typeof summaryRequest.snapshotId, 'string'); assert.equal(typeof summaryRequest.threadId, 'string'); assert.equal(typeof summaryRequest.turnId, 'string');
  const before = String(summaryRequest.snapshotId), thread = String(summaryRequest.threadId), turn = String(summaryRequest.turnId);
  const shared = await share(page, [fixture.dir, privateBody, title, nativeThread, nativeTurn, 'synthetic-browser-command-one', 'synthetic-browser-server', 'synthetic-browser-tool']);
  assert.ok(record(shared) && record(shared.uses) && record(shared.uses.recordCount)); assert.equal(shared.uses.recordCount.value, 4);
  bound(requests, before, thread, turn, oracle.scope.sourceInstanceId); await overflow(page);

  await fixture.append(); const next = await fixture.current(); assert.notEqual(next.readView.snapshotId, before);
  assert.equal(next.uses.objects.find(object => object.kind === 'skill')?.useCount.value, 4);
  assert.equal(next.work.operationCandidates.value, 8);
  // Existing group remains readable until the explicit application of new data.
  assert.equal(await skill.locator('dd').first().innerText(), '3');
  await page.getByRole('button', { name: label('webui.applyUpdates'), exact: true }).waitFor();
  await activate(page.getByRole('button', { name: label('webui.applyUpdates'), exact: true }));
  await page.waitForFunction(() => document.querySelector('.turn[open] > summary .token-number')?.getAttribute('title') === '220 Token');
  await page.waitForFunction(({ selector, skill }) => [...document.querySelectorAll(selector)].find(article => article.textContent?.includes(skill))?.querySelector('dd')?.textContent === '4', { selector: `section[aria-label=${JSON.stringify(label('execution.uses'))}] article`, skill: fixture.skill });
  await execution(page).getByRole('button', { name: label('execution.share'), exact: true }).waitFor();
  const refreshed = await share(page, [fixture.dir, privateBody, title, nativeThread, nativeTurn]);
  assert.ok(record(refreshed) && record(refreshed.uses) && record(refreshed.uses.recordCount)); assert.equal(refreshed.uses.recordCount.value, 5);
  const latest = requests.filter(request => request.action === 'summary' && request.privacyProfile === 'local').at(-1); assert.ok(latest); assert.notEqual(latest.snapshotId, before);
  const afterIndex = requests.indexOf(latest); bound(requests.slice(afterIndex), String(latest.snapshotId), thread, turn, next.scope.sourceInstanceId);
  await comparisons(page,fixture,language,width);
  await setupJourney(page,fixture,language,width,next.scope.sourceInstanceId);
  await statisticsMonitoring(page,fixture,language,width);
  assert.deepEqual(errors, []); await overflow(page);
  return { source: 'production', language, width, journeys: ['find-return-keyboard', 'parallel-union-sum', 'three-uses-including-failure', 'zero-versus-unrecorded', 'append-explicit-refresh-share-fixed-group','period-contributions-fixed-evidence','session-family-comparison','publication-changes','setup-scoped-receipts-pagination-keyboard-reload','task-distribution-model-groups-budget-validation-save-pause-acknowledge-reload-resume'] };
}

async function statisticsMonitoring(page:Page,fixture:Fixture,language:Language,width:number) {
  const url=new URL(fixture.product);url.search=new URLSearchParams({page:'usage',allTime:'1',timezone:'UTC',project:fixture.project}).toString();
  const fragment=new URLSearchParams(url.hash.slice(1));fragment.set('lang',language);url.hash=fragment.toString();
  await page.goto(url.href);await settled(page);await details(page,'statistics.title');
  const statistics=page.locator('details').filter({hasText:label('statistics.title')}).first();
  await statistics.locator('tbody tr').first().waitFor();await text(statistics,t('statistics.population',{complete:2,total:2}));
  await statistics.getByRole('combobox',{name:label('webui.dimension'),exact:true}).selectOption('models');
  await statistics.locator('tbody tr').filter({hasText:'gpt-5.4'}).waitFor();await overflow(page);
  await details(page,'monitor.title');
  const monitor=page.locator('details').filter({hasText:label('monitor.title')}).first();
  const id=`browser-${language}-${width}`;
  await monitor.getByRole('textbox',{name:label('monitor.id'),exact:true}).fill('invalid id');
  assert.equal(await monitor.locator('form').evaluate(form=>(form as HTMLFormElement).checkValidity()),false,'Invalid plan IDs must be blocked by browser validation');
  await monitor.getByRole('textbox',{name:label('monitor.id'),exact:true}).fill(id);
  await monitor.getByRole('spinbutton',{name:label('monitor.tokens'),exact:true}).fill('100');
  assert.equal(await monitor.locator('form').evaluate(form=>(form as HTMLFormElement).checkValidity()),true);
  await monitor.getByRole('combobox',{name:label('webui.period'),exact:true}).selectOption('month');
  await activate(monitor.getByRole('button',{name:label('monitor.save'),exact:true}));
  const plan=monitor.locator('p > span').filter({hasText:id});await plan.waitFor();
  await activate(monitor.getByRole('button',{name:label('monitor.check'),exact:true}));
  const notices=monitor.locator('article').filter({hasText:id});await notices.filter({hasText:label('monitor.budget_exceeded')}).waitFor();
  assert.equal(await notices.count(),2);
  await activate(monitor.getByRole('button',{name:label('monitor.check'),exact:true}));
  assert.equal(await notices.count(),2,'Repeated checks must retain the same history without duplicate notices');
  await activate(plan.getByRole('button',{name:label('monitor.pause'),exact:true}));
  await plan.getByRole('button',{name:label('monitor.resume'),exact:true}).waitFor();
  assert.equal(await notices.count(),2,'Pausing a plan does not hide prior facts');
  await activate(notices.getByRole('button',{name:label('monitor.acknowledge'),exact:true}).first());
  await page.waitForFunction(({id,label})=>{const panel=[...document.querySelectorAll('details')].find(e=>e.querySelector('summary')?.textContent===label);return [...(panel?.querySelectorAll('article')??[])].filter(e=>e.textContent?.includes(id)).filter(e=>e.querySelector('button')).length===1;},{id,label:label('monitor.title')});
  await page.reload();await settled(page);await details(page,'monitor.title');
  await plan.getByRole('button',{name:label('monitor.resume'),exact:true}).waitFor();
  assert.equal(await notices.count(),2,'Reload preserves notification history');
  assert.equal(await notices.getByRole('button',{name:label('monitor.acknowledge'),exact:true}).count(),1,'Reload preserves acknowledgement');
  await activate(plan.getByRole('button',{name:label('monitor.resume'),exact:true}));
  await plan.getByRole('button',{name:label('monitor.pause'),exact:true}).waitFor();
  await activate(monitor.getByRole('button',{name:label('monitor.check'),exact:true}));
  assert.equal(await notices.count(),2,'Resuming the original plan must not duplicate prior notices');
  await overflow(page);
  await activate(plan.getByRole('button',{name:label('monitor.remove'),exact:true}));
  await plan.waitFor({state:'hidden'});
}

async function rulePreviews(page: Page, origin: string, language: Language): Promise<void> {
  const open = async () => { await activate(page.locator('.review-list .review-row').first()); await page.locator('dialog .review-detail').waitFor(); };
  const action = async (key: PlainKey) => {
    await activate(page.locator('dialog .review-actions').getByRole('button', { name: label(key), exact: true }));
    await page.locator('dialog .review-detail').waitFor({ state: 'hidden' });
    await open();
  };
  const latest = () => page.locator('dialog .review-related').filter({ hasText: label('optimize.assessment.latest') });
  await page.goto(origin + `/preview.html?scenario=complete&page=optimize&allTime=1&lang=${language}`);
  await open();
  for (const outcome of ['hit', 'miss', 'insufficient', 'unsupported', 'error'] as const) await text(latest(), label(`optimize.check.${outcome}`));
  await action('optimize.keep');
  await text(page.locator('dialog .review-history'), label('optimize.assessment.decisionNote'));
  await action('optimize.recheck');
  await text(latest(), label('optimize.check.hit'));
  await text(page.locator('dialog .review-history'), label('optimize.assessment.necessary'));
  await action('optimize.redisplay');
  await page.locator('dialog .review-actions').getByRole('combobox', { name: label('optimize.decisionReason'), exact: true }).selectOption('incorrect_evidence');
  await action('optimize.notApplicable');
  await text(page.locator('dialog .review-history'), label('optimize.reason.incorrectEvidence'));
  await text(latest(), label('optimize.check.hit'));
  await overflow(page);
  for (const [scenario, comparison] of [['resolved', 'comparable'], ['rule-upgraded', 'incomparable'], ['evidence-gap', 'unknown']] as const) {
    await page.goto(origin + `/preview.html?scenario=${scenario}&page=optimize&allTime=1&lang=${language}`);
    await open(); await action('optimize.recheck');
    await text(latest(), label(`optimize.assessment.comparison.${comparison}`));
    await text(latest(), label(scenario === 'resolved' ? 'optimize.check.miss' : scenario === 'evidence-gap' ? 'optimize.check.insufficient' : 'optimize.check.hit'));
    await activate(page.locator('dialog summary').filter({ hasText: label('optimize.assessment.original') }).first());
    await text(page.locator('dialog details').filter({ hasText: label('optimize.assessment.original') }).first(), label('optimize.check.hit'));
    await overflow(page);
  }
}

export async function previews(page: Page, origin: string, language: Language, width: number): Promise<Case> {
  locale.setLocale(language); await page.setViewportSize({ width, height: 900 }); const errors = pageErrors(page);
  await page.goto(origin + `/preview.html?previewModule=execution&lang=${language}`);
  await text(page.locator('#preview-controls'), label('preview.title')); await execution(page).waitFor();
  const scenario = page.getByRole('combobox', { name: label('preview.scenario'), exact: true });
  await details(page, 'execution.distribution'); await text(execution(page), label('execution.concurrent')); await overflow(page);
  const skill = uses(page).locator('article').filter({ hasText: 'SKILL.md' }).first(); await skill.waitFor();
  assert.equal(await skill.locator('dd').first().innerText(), '3');
  assert.equal(await skill.locator('dd').nth(3).innerText(), '0');
  assert.equal(await uses(page).locator('article').nth(1).locator('dd').first().innerText(), '2', 'partial MCP coverage retains the observed associated uses');
  await activate(skill.getByRole('button', { name: label('execution.useEvidence'), exact: true }));
  await uses(page).locator('aside article').first().waitFor(); await text(uses(page).locator('aside'), label('execution.useOutcome.failed'));
  await activate(uses(page).locator('aside').getByRole('button', { name: label('execution.closeEvidence'), exact: true }));

  await scenario.selectOption('missing'); await execution(page).waitFor(); await text(execution(page), label('timing.basis.notRecorded'));
  assert.equal(await page.locator('.execution-track button').count(), 0);
  await scenario.selectOption('running'); await text(execution(page), label('execution.running')); await text(execution(page), label('timing.running')); await text(execution(page), label('timing.censored'));
  await scenario.selectOption('empty');
  await waitCount(page, `section[aria-label=${JSON.stringify(label('execution.uses'))}]`, 0);
  await text(execution(page), label('execution.missing'));
  await scenario.selectOption('uses-failure'); await uses(page).locator('article').first().waitFor();
  await activate(uses(page).locator('article').first().getByRole('button', { name: label('execution.useEvidence'), exact: true }));
  await uses(page).getByRole('alert').waitFor(); await text(execution(page), '10000 ms'); assert.equal(await uses(page).locator('article dd').first().innerText(), '3');
  await scenario.selectOption('uses-expired'); await uses(page).locator('article').first().waitFor();
  await activate(uses(page).locator('article').first().getByRole('button', { name: label('execution.useEvidence'), exact: true }));
  await uses(page).getByRole('alert').waitFor();
  assert.equal(await uses(page).locator('article').first().getByRole('button', { name: label('execution.useEvidence'), exact: true }).isEnabled(), false);
  assert.equal(await execution(page).getByRole('button', { name: label('execution.share'), exact: true }).isEnabled(), false);
  await activate(execution(page).getByRole('button', { name: label('execution.refresh'), exact: true }));
  await page.waitForFunction(selector => document.querySelector<HTMLButtonElement>(selector)?.disabled === false, `section[aria-label=${JSON.stringify(label('execution.uses'))}] article button`);

  await scenario.selectOption('dense'); await uses(page).locator('article').first().waitFor();
  await waitCount(page, `section[aria-label=${JSON.stringify(label('execution.uses'))}] > article`, 50);
  assert.equal(await uses(page).locator(':scope > article').count(), 50);
  await activate(uses(page).getByRole('button', { name: label('execution.nextUseObjects'), exact: true }));
  await waitCount(page, `section[aria-label=${JSON.stringify(label('execution.uses'))}] > article`, 3);
  assert.equal(await metric(page, `section[aria-label=${JSON.stringify(label('execution.uses'))}]`, label('execution.useObjectCount')), '53');
  await activate(uses(page).getByRole('button', { name: label('execution.firstUseObjects'), exact: true }));
  await activate(uses(page).locator('article').first().getByRole('button', { name: label('execution.useEvidence'), exact: true }));
  await waitCount(page, `section[aria-label=${JSON.stringify(label('execution.uses'))}] aside article`, 200);
  assert.equal(await uses(page).locator('aside article').count(), 200);
  const firstReference = await uses(page).locator('aside article details code').first().innerText();
  await activate(uses(page).locator('aside').getByRole('button', { name: label('execution.nextEvidence'), exact: true }));
  await page.waitForFunction(({ selector, previous }) => document.querySelector(selector)?.textContent !== previous, { selector: `section[aria-label=${JSON.stringify(label('execution.uses'))}] aside article details code`, previous: firstReference });
  await waitCount(page, `section[aria-label=${JSON.stringify(label('execution.uses'))}] aside article`, 14);
  await text(uses(page).locator('aside'), '214');
  await overflow(page); await page.keyboard.press('Tab'); assert.ok(await page.evaluate<boolean>('document.activeElement !== document.body'));

  // Application preview exercises production App navigation separately from
  // real-core acceptance above.
  await page.goto(origin + `/preview.html?scenario=tasks-pages&page=threads&allTime=1&lang=${language}`);
  await page.locator('.thread-row').nth(1).waitFor(); const returning = page.locator('.thread-row').nth(1); await activate(returning); await page.locator('.turn[open]').waitFor();
  await settled(page);
  if (width <= 760) await activate(page.locator('.task-return')); else await page.goBack();
  await returning.waitFor();
  await page.waitForFunction(() => !new URL(location.href).searchParams.has('thread'));
  await settled(page);
  assert.equal(await returning.evaluate(button => button === document.activeElement), true);
  await overflow(page); await rulePreviews(page, origin, language); assert.deepEqual(errors, []);
  return { source: 'preview', language, width, journeys: ['production-App-return', 'parallel-explanation-presentation-only', 'uses-failure-expiry-refresh', 'zero-missing-running', 'dense-object-and-record-pages-keyboard', 'rule-decisions-separate-from-checks', 'rule-recheck-resolution-version-and-evidence'] };
}

async function comparisons(page:Page,fixture:Fixture,language:Language,width:number) {
 const current=await fixture.client.live!({query:{action:'threads',roots:[fixture.root],scope:{allTime:true}},mode:'cached'});
 const parent=current.result.items.find(i=>i.kind==='thread'&&i.upstreamId===nativeThread);
 const child=current.result.items.find(i=>i.kind==='thread'&&i.upstreamId==='synthetic-comparison-child');
 assert.ok(parent?.kind==='thread'&&child?.kind==='thread');
 const url=new URL(fixture.product);url.search=new URLSearchParams({page:'usage',since:fixture.date,until:fixture.date,timezone:'UTC',snapshot:current.result.snapshotRef.snapshotId}).toString();
 const fragment=new URLSearchParams(url.hash.slice(1));fragment.set('lang',language);url.hash=fragment.toString();
 await page.goto(url.href);
 await activate(page.locator('.usage-comparison summary'));await page.locator('.usage-comparison tbody tr').first().waitFor();
 await text(page.locator('.usage-comparison'),'+275 Token');await text(page.locator('.usage-comparison'),label('comparison.note'));await overflow(page);
 const inspection=page.locator('.inspection').first();await activate(inspection.locator('summary').first());
 await text(inspection,label('inspection.opportunities.title'));await text(inspection,label('inspection.opportunities.rule.risky_command'));await text(inspection,label('inspection.opportunities.label.broad_permissions'));
 await text(inspection,label('inspection.activity.title'));await text(inspection,label('inspection.activity.signal.repeated_slow_request'));await text(inspection,label('inspection.activity.advice.repeated_slow_request'));
 await activate(inspection.locator('.inspection-card').filter({hasText:label('inspection.activity.signal.repeated_slow_request')}).getByRole('button').first());await page.locator('.evidence-selected').waitFor();
 const activityProofUrl=new URL(page.url());assert.equal(activityProofUrl.searchParams.get('snapshot'),current.result.snapshotRef.snapshotId);assert.ok(activityProofUrl.searchParams.get('operation'));await overflow(page);
 await page.goto(url.href);await activate(inspection.locator('summary').first());
 await inspection.getByRole('combobox',{name:label('inspection.title')}).selectOption('resources');await text(inspection,label('inspection.limit.actual_changes_unavailable'));await inspection.locator('.inspection-card').first().waitFor();await overflow(page);
 await activate(inspection.locator('.inspection-proof button').first());await page.locator('.evidence-selected').waitFor();
 const proofUrl=new URL(page.url());assert.equal(proofUrl.searchParams.get('snapshot'),current.result.snapshotRef.snapshotId);assert.ok(proofUrl.searchParams.get('operation'));await overflow(page);
 await page.goto(url.href);await activate(page.locator('.usage-comparison summary'));await page.locator('.usage-comparison tbody tr').first().waitFor();await activate(inspection.locator('summary').first());
 await inspection.getByRole('combobox',{name:label('inspection.title')}).selectOption('context');await text(inspection,label('inspection.limit.context_metadata_unavailable'));await inspection.locator('.inspection-card').first().waitFor();await text(inspection,label('inspection.contentUnknown'));await overflow(page);
 await inspection.getByRole('combobox',{name:label('inspection.title')}).selectOption('review');await text(inspection,label('inspection.concentration'));await text(inspection,label('inspection.topTasks'));await text(inspection,label('comparison.delta'));await overflow(page);
 await activate(page.locator('.usage-comparison tbody tr td').nth(2).getByRole('button'));
 await page.locator('.thread-row').first().waitFor();await settled(page);assert.equal(new URL(page.url()).searchParams.get('snapshot'),current.result.snapshotRef.snapshotId);
 await activate(page.locator('.thread-row').filter({hasText:title}).first());
 await page.waitForFunction(id=>new URL(location.href).searchParams.get('thread')===id,parent.id);
 await page.locator('.turn[open]').waitFor();await execution(page).waitFor();await settled(page);
 const trajectory=page.locator('.task-detail .inspection').filter({hasText:label('inspection.trajectory')}).first();await activate(trajectory.locator('summary').first());await text(trajectory,label('inspection.limit.context_occupancy_unavailable'));await trajectory.locator('.inspection-card').first().waitFor();await overflow(page);
 const compare=page.locator('.task-detail details').filter({hasText:label('comparison.sessions')}).first();
 if(await compare.getAttribute('open')==null)await activate(compare.locator('summary').first());
 await compare.getByRole('combobox',{name:label('comparison.other')}).fill(child.id);
 await activate(compare.getByRole('button',{name:label('comparison.submit'),exact:true}));await compare.locator('tbody tr').first().waitFor();
 await text(compare,'-165 Token');const family=compare.getByRole('checkbox',{name:label('comparison.family')});await family.focus();await family.press('Space');
 await text(compare,'-220 Token');await text(compare,t('comparison.members',{count:2}));await overflow(page);
 await settled(page);
 if(width<=760){
  await activate(page.getByRole('button',{name:label('task.backToList'),exact:true}));
  await page.waitForFunction(()=>!new URL(location.href).searchParams.has('thread'));
  await page.locator('.thread-list').waitFor();await settled(page);
  // Returning restores keyboard focus after React commits the route. Visibility
  // alone can race that restoration and send the next Enter to the task row.
  await page.waitForFunction(()=>document.activeElement?.classList.contains('thread-row'));
 }
 const changes=page.locator('.thread-list details').filter({hasText:label('comparison.refresh')}).first();
 if(await changes.getAttribute('open')==null)await activate(changes.locator('summary'));
 await text(changes,label('comparison.refreshNote'));await text(changes,'+110 Token');
 if(width<=760)assert.equal(new URL(page.url()).searchParams.has('thread'),false,'Opening refresh changes must keep the narrow task list visible');
}

async function setupJourney(page:Page,fixture:Fixture,language:Language,width:number,source:string) {
 const requests:Record<string,unknown>[]=[];
 page.on('request',r=>{if(r.method()==='POST'&&new URL(r.url()).pathname==='/api/collection'&&requests.length<30){const body=r.postDataJSON();if(record(body))requests.push(body);}});
 const url=new URL(fixture.product);url.search=new URLSearchParams({page:'usage',allTime:'1',project:fixture.project,source}).toString();
 const fragment=new URLSearchParams(url.hash.slice(1));fragment.set('lang',language);url.hash=fragment.toString();
 await page.goto(url.href);
 const entry=page.getByRole('button',{name:label('setup.title'),exact:true}).first();await activate(entry);
 const dialog=page.getByRole('dialog',{name:label('setup.title'),exact:true});await dialog.waitFor();
 await text(dialog,label('setup.modeNote'));await text(dialog,'wombat collection mode hooks --json');
 await dialog.locator('.setup-status dd').filter({hasText:label('setup.liveMode')}).waitFor();await dialog.locator('.setup-content[aria-busy="false"]').waitFor();const copiedRequests=requests.length;const command=dialog.locator('.setup-copy').filter({hasText:'wombat collection mode hooks --json'});await activate(command.getByRole('button',{name:label('setup.copyCommand'),exact:true}));await text(command,label('setup.copied'));assert.equal(await page.evaluate<string>('navigator.clipboard.readText()'),'wombat collection mode hooks --json');assert.equal(requests.length,copiedRequests);
 await activate(dialog.locator('.setup-steps button').filter({hasText:label('setup.step.connect')}));
 await text(dialog,label('collection.received'));await text(dialog,label('setup.unchecked'));await text(dialog,label('setup.receiptScope'));
 await activate(dialog.locator('summary').filter({hasText:label('setup.events')}));
 await waitCount(page,'dialog .setup-events tbody tr',50);
 await text(dialog,label('setup.association.linked'));await text(dialog,nativeThread);
 await activate(dialog.getByRole('button',{name:label('webui.next'),exact:true}));
 await waitCount(page,'dialog .setup-events tbody tr',1);
 assert.equal(await dialog.getByRole('button',{name:label('webui.next'),exact:true}).isEnabled(),false);
 await activate(dialog.getByRole('button',{name:label('setup.firstEvents'),exact:true}));
 await waitCount(page,'dialog .setup-events tbody tr',50);await overflow(page);
 await activate(dialog.locator('.setup-steps button').filter({hasText:label('setup.step.history')}));await text(dialog,'wombat service status --json');
 await activate(dialog.locator('.setup-steps button').filter({hasText:label('setup.step.use')}));await text(dialog,label('setup.skillNote'));
 const question=dialog.locator('.setup-example .setup-copy').first();const beforeCopy=requests.length;await activate(question.getByRole('button',{name:label('setup.copyQuestion'),exact:true}));await text(question,label('setup.copied'));const copied=await page.evaluate<string>('navigator.clipboard.readText()');assert.ok(copied.includes(label('setup.exampleUsage'))&&copied.includes(fixture.project));assert.equal(requests.length,beforeCopy);
 await page.keyboard.press('Escape');await dialog.waitFor({state:'hidden'});
 assert.equal(await entry.evaluate(e=>e===document.activeElement),true);
 for(const r of requests){assert.equal(r.project,fixture.project);assert.equal(r.sourceInstanceId,source);assert.ok(!('roots' in r));assert.ok(!['configure','pause','resume'].includes(String(r.action)));}
 assert.ok(requests.filter(r=>r.action==='events').length>=3);
 await page.goto(page.url());await activate(page.getByRole('button',{name:label('setup.useWithSkill'),exact:true}));
 await text(page.getByRole('dialog'),label('setup.skillNote'));await overflow(page);await page.keyboard.press('Escape');
}
