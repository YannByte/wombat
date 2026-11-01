import {configExitCode} from './exit-codes.js';
import { CoreError, type ConfigRequest, type ConfigItem } from '@wombat/client';
import { createNodeClient } from '@wombat/client/node';
import { t, useBasisCount, useBasisPresentation, configEvidenceLabel, eventStatusLabel } from '@wombat/client/locale';
import path from 'node:path';
import { terminalText } from './display-text.js';

export function parseConfigArgs(argv: string[]): { request: ConfigRequest; json: boolean; help: boolean } {
  const request: ConfigRequest = { action: 'list', scope: {} };
  let json = false, help = false;
  const roots: string[] = [], projects: string[] = [], seen = new Set<string>();
  const fail = (value: string): never => { throw new CoreError('INVALID_ARGUMENT', t('cli.config.invalid', { value })); };
  for (let i = 0; i < argv.length; i++) {
    const [name, inline] = argv[i].split(/=(.*)/s);
    if (name === '--help' || name === '-h') { help = true; continue; }
    if (name === '--all-time') { if(inline!==undefined || request.scope!.allTime) fail(argv[i]); request.scope!.allTime=true; continue; }
    if (name === '--json') { if (inline !== undefined || json) fail(argv[i]); json = true; continue; }
    if (!['--root','--project-root','--project','--since','--until','--timezone','--agent','--source','--thread','--kind','--observation','--search','--sort','--item','--action','--offset','--limit','--read-view','--snapshot'].includes(name)) fail(argv[i]);
    if (seen.has(name) && !['--root','--project-root'].includes(name)) fail(name);
    seen.add(name);
    const value = inline ?? argv[++i];
    if (!value || value.startsWith('--')) fail(name);
    const scope = request.scope!;
    switch (name) {
      case '--root': roots.push(path.resolve(value)); break;
      case '--project-root': projects.push(path.resolve(value)); break;
      case '--project': scope.project = path.resolve(value); break;
      case '--since': scope.since = value; break;
      case '--until': scope.until = value; break;
      case '--timezone': scope.timezone = value; break;
      case '--agent': scope.agentKind = value; break;
      case '--source': scope.sourceInstanceId = value; break;
      case '--thread': scope.threadId = value; break;
      case '--kind': if (!['rule','skill','mcp','hook'].includes(value)) fail(value); request.kind = value as ConfigRequest['kind']; break;
      case '--observation': if (!['used','loaded_only','unknown'].includes(value)) fail(value); request.observation = value as ConfigRequest['observation']; break;
      case '--sort': if (!['tokens','activity','size','name','content_tokens','characters','recent'].includes(value)) fail(value); request.sort = value as ConfigRequest['sort']; break;
      case '--action': if (!['list','detail','evidence','related_scopes','capabilities'].includes(value)) fail(value); request.action = value as ConfigRequest['action']; break;
      case '--item': request.itemId = value; break;
      case '--search': request.search = value; break;
      case '--read-view': request.readView = value; break;
      case '--snapshot': request.snapshotId = value; break;
      case '--offset': case '--limit': {
        const n = Number(value);
        if (!/^\d+$/.test(value) || !Number.isSafeInteger(n) || (name === '--limit' && (n < 1 || n > 200))) fail(value);
        request[name === '--limit' ? 'limit' : 'offset'] = n; break;
      }
    }
  }
  if(request.scope!.allTime && (request.scope!.since || request.scope!.until)) fail('--all-time');
  if (roots.length) request.roots = roots;
  request.projectRoots = projects.length ? projects : [process.cwd()];
  return { request, json, help };
}

export async function runConfigCli(argv: string[]): Promise<number> {
  if(argv[0]==='cleanup')return(await import('./artifacts-cli.js')).runArtifactsCli(argv.slice(1));
  if (argv[0] !== 'inventory') return (await import('./optimize-cli.js')).runOptimizeCli(argv);
  const { request, json, help } = parseConfigArgs(argv.slice(1));
  if (help) { process.stdout.write(t('cli.config.help')); return 0; }
  const controller = new AbortController(), stop = () => controller.abort();
  process.once('SIGINT', stop); process.once('SIGTERM', stop);
  try {
    const result = await createNodeClient().config!(request, { signal: controller.signal });
    if (json) process.stdout.write(JSON.stringify(result) + '\n');
    else {
      process.stdout.write(t('config.scopeNote') + '\n');
      if(result.extensionActivity){const a=result.extensionActivity;process.stdout.write(t('config.activity.counts',{used:a.observedUse,quiet:a.noObservedUse,unknown:a.unavailable})+'\n');for(const item of a.items)if(item.noObservedUseDays!=null)process.stdout.write(`${terminalText(result.items.find(i=>i.id===item.itemId)?.name??item.itemId)} · ${t('config.activity.days',{days:item.noObservedUseDays})}\n`);process.stdout.write(t('config.activity.note')+'\n');}
      for (const item of result.items) {
        process.stdout.write(`${item.kind}\t${terminalText(item.name)}\t${t(`config.${item.observation}`)}\t${terminalText(item.path)}\n`);
        if (item.kind !== 'hook') {
          for (const line of configUseBasisLines(item)) process.stdout.write(`  ${line}\n`);
        }
      }
      for (const context of result.hookRegistry.contexts) for (const hook of context.registrations.filter(h => result.items.some(i => i.id === h.itemId))) {
        const plugin = hook.pluginId ? '\t' + t('config.hookPlugin', { name: terminalText(hook.pluginId) }) : '';
        process.stdout.write(`${terminalText(context.project)}\t${terminalText(hook.itemId)}\t${t(hook.enabled ? 'config.hookEnabled' : 'config.hookDisabled')}\t${t(`config.hookTrust.${hook.trust}`)}${plugin}\n`);
      }
      for (const row of result.evidence) process.stdout.write(`${row.timestamp ?? '—'}\t${row.threadId}\t${configEvidenceLabel(row.eventType)}\t${eventStatusLabel(row.outcome)}\n`);
      for (const row of result.relatedScopes) process.stdout.write(`${terminalText(row.project ?? '—')}\t${row.evidenceCount}\n`);
      process.stdout.write(t('config.readVersion', { version: result.readView ?? '—' }) + '\n');
      process.stdout.write(t('config.coverageNote') + '\n');
    }
    return configExitCode(result);
  } finally { process.off('SIGINT', stop); process.off('SIGTERM', stop); }
}

export function configUseBasisLines(item: ConfigItem): string[] {
  const basis = useBasisPresentation(item.useBasis);
  const count = useBasisCount(item.kind === 'rule' ? item.counts.fileReads : item.usageCount, item.useBasis);
  return [...(count == null ? [] : [`${t(item.kind === 'rule' ? 'useBasis.ruleLoadOrRead' : 'useBasis.objectUse')}: ${count}`]), basis.summary, ...basis.notes, ...basis.details].map(terminalText);
}
