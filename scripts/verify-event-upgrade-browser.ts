import assert from 'node:assert/strict';
import path from 'node:path';
import { fileURLToPath } from 'node:url';
import { checkBuild, sourceIdentity } from './build-identity.ts';
import { fixture, stop, type Fixture } from './event-upgrade-browser/fixture.ts';
import { options } from './event-upgrade-browser/policy.ts';
import { playwright, type Browser, type BrowserServer, type Context } from './event-upgrade-browser/playwright.ts';
import { product, previews, pageErrors, type Case, type Language } from './event-upgrade-browser/journeys.ts';

// Artifact/browser acceptance is deliberately separate from source-only repo
// checks. Run a complete build first; all source reads below are synthetic.
const repo = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '..');
const controller = new AbortController();
let server: BrowserServer | undefined, browser: Browser | undefined, data: Fixture | undefined;
let stage = 'prerequisites', fingerprint: string | undefined;
const completed: Case[] = [];
const timeout = setTimeout(() => controller.abort(new Error('Browser acceptance exceeded eight minutes')), 8 * 60_000);
const interrupted = () => controller.abort(new Error('Browser acceptance interrupted'));
process.once('SIGINT', interrupted); process.once('SIGTERM', interrupted);
const cancelBrowser = () => { server?.process().kill('SIGTERM'); };
controller.signal.addEventListener('abort', cancelBrowser, { once: true });

async function closeContext(context: Context) {
  let timer: ReturnType<typeof setTimeout> | undefined;
  try { await Promise.race([context.close(), new Promise<never>((_, reject) => { timer = setTimeout(() => reject(new Error('Browser context cleanup timed out')), 4000); })]); }
  finally { clearTimeout(timer); }
}
async function journey(source: 'production' | 'preview', language: Language, width: number) {
  assert.ok(browser && data); controller.signal.throwIfAborted();
  const context = await browser.newContext({ viewport: { width, height: 900 } });
  let page: Awaited<ReturnType<Context['newPage']>> | undefined;
  const failedResponses: { path: string; status: number }[] = [];
  try {
    await context.grantPermissions(['clipboard-read', 'clipboard-write'], { origin: new URL(data.product).origin });
    page = await context.newPage(); page.setDefaultTimeout(15_000); page.setDefaultNavigationTimeout(15_000);
    page.on('response', response => { if (response.status() >= 400 && failedResponses.length < 20) failedResponses.push({ path: new URL(response.url()).pathname, status: response.status() }); });
    completed.push(source === 'production' ? await product(page, data, language, width) : await previews(page, data.preview, language, width));
  } catch (error) {
    // Only this runner's synthetic DOM is recorded, bounded and external in the
    // unified verifier's stage log. Never capture screenshots or personal data.
    if (page) try { console.error(JSON.stringify({ source, language, width, failedResponses, failedDom: (await page.locator('body').innerText()).slice(0, 16_000), focus: await page.evaluate<string>('document.activeElement?.outerHTML?.slice(0,1000)??""'), layout: await page.evaluate<unknown>(`({width:innerWidth,scrollWidth:document.documentElement.scrollWidth,offenders:[...document.querySelectorAll('body *')].map(e=>({tag:e.tagName,class:e.className,text:e.textContent?.slice(0,80),left:e.getBoundingClientRect().left,right:e.getBoundingClientRect().right,width:e.getBoundingClientRect().width})).filter(e=>e.right>innerWidth+1||e.left< -1).slice(0,15)})`) })); } catch { /* Preserve the original browser failure. */ }
    throw error;
  } finally { await closeContext(context); }
}
async function minimumWidth(language: Language) {
  assert.ok(browser && data); controller.signal.throwIfAborted();
  const context = await browser.newContext({ viewport: { width: 320, height: 900 } });
  let page: Awaited<ReturnType<Context['newPage']>> | undefined;
  try {
    page = await context.newPage(); page.setDefaultTimeout(15_000); page.setDefaultNavigationTimeout(15_000);
    const errors = pageErrors(page);
    await page.goto(data.preview + `/preview.html?previewModule=execution&lang=${language}`);
    await page.locator('.execution-summary strong').filter({ hasText: '10000 ms' }).waitFor();
    await page.locator('section article code').filter({ hasText: '/synthetic/wombat/skills/review/SKILL.md' }).first().waitFor();
    assert.equal(await page.evaluate<boolean>('document.documentElement.scrollWidth > innerWidth + 1'), false, 'Minimum-width horizontal overflow');
    assert.deepEqual(errors, []);
    completed.push({ source: 'preview', language, width: 320, journeys: ['minimum-width-long-names-no-horizontal-overflow'] });
  } catch (error) {
    if (page) try { console.error(JSON.stringify({ source: 'preview', language, width: 320, layout: await page.evaluate<unknown>(`({width:innerWidth,scrollWidth:document.documentElement.scrollWidth,offenders:[...document.querySelectorAll('body *')].map(e=>({tag:e.tagName,class:e.className,text:e.textContent?.slice(0,80),left:e.getBoundingClientRect().left,right:e.getBoundingClientRect().right,width:e.getBoundingClientRect().width})).filter(e=>e.right>innerWidth+1||e.left< -1).slice(0,15)})`) })); } catch { /* Preserve the original failure. */ }
    throw error;
  } finally { await closeContext(context); }
}
async function cleanup() {
  clearTimeout(timeout); process.removeListener('SIGINT', interrupted); process.removeListener('SIGTERM', interrupted);
  controller.signal.removeEventListener('abort', cancelBrowser);
  // Every owned resource is attempted even if another close fails. Kill fallback
  // bounds browser shutdown; Playwright performs its own profile cleanup.
  const browserClose = async () => {
    let timer: ReturnType<typeof setTimeout> | undefined;
    try { await Promise.race([Promise.all([browser?.close(), server?.close()]), new Promise<never>((_, reject) => { timer = setTimeout(() => reject(new Error('Browser cleanup timed out')), 4000); })]); }
    finally { clearTimeout(timer); await stop(server?.process()); }
  };
  const results = await Promise.allSettled([browserClose(), data?.close()]);
  const failed = results.find(result => result.status === 'rejected'); if (failed?.status === 'rejected') throw failed.reason;
}
let failure: unknown;
try {
  const config = options(process.argv.slice(2), process.env);
  checkBuild(repo); fingerprint = sourceIdentity(repo);
  const api = await playwright(config.module);
  stage = 'synthetic-core-and-preview-startup'; data = await fixture(repo, controller.signal);
  stage = 'browser-startup';
  server = await api.chromium.launchServer({ headless: true, executablePath: config.executable, timeout: 15_000 });
  browser = await api.chromium.connect(server.wsEndpoint(), { timeout: 10_000 });
  for (const language of ['zh', 'en'] as const) for (const width of [320, 390, 768, 1440]) {
    stage = `production-${language}-${width}`; await journey('production', language, width);
    if (width !== 320) { stage = `preview-${language}-${width}`; await journey('preview', language, width); }
  }
  for (const language of ['zh', 'en'] as const) { stage = `preview-${language}-320`; await minimumWidth(language); }
  stage = 'final-build-identity'; checkBuild(repo); assert.equal(sourceIdentity(repo), fingerprint);
} catch (error) { failure = error; }
finally { try { await cleanup(); } catch (error) { failure ??= error; stage = 'cleanup'; } }
console.log(JSON.stringify({ format: 1, status: failure ? 'failed' : 'passed', stage, sourceSha256: fingerprint, platform: `${process.platform}-${process.arch}`,
  syntheticOnly: true, screenshots: false, production: completed.filter(item => item.source === 'production'), preview: completed.filter(item => item.source === 'preview'),
  boundaries: ['Production uses the built core, Node client, and local HTTP host with isolated native-format fixtures.', 'Preview uses the production components with synthetic typed transports; it does not establish core accuracy.', 'No personal source data, other operating systems, screenshots, or visual styling acceptance.'],
  error: failure instanceof Error ? failure.message.slice(0, 2000) : failure === undefined ? undefined : String(failure).slice(0, 2000),
}, null, 2));
if (failure) process.exitCode = 1;
