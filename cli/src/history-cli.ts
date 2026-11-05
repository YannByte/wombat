import {writeHelp} from './help.js';
import path from 'node:path';
import {CoreError, type UsageClient, type UsageScope, type HistoryRead, type ConversationRequest} from '@wombat/client';
import {createNodeClient} from '@wombat/client/node';
import {t,messageOriginText} from '@wombat/client/locale';
import {dateWindow} from './calendar.js';
import {ask, canPrompt, commandLine, invalidInput, pageNumber, parseOptions, projectFilter, withCancellation} from './human-input.js';
import {terminalText,terminalBlock} from './display-text.js';
import {resultExitCode} from './exit-codes.js';

type RecordChoice={id:string;upstreamId?:string;title?:string|null;project?:string|null;lastActivityAt?:string|null};
export async function resolveRecord(client: Pick<UsageClient,'live'|'conversation'>, record: string, scope: UsageScope, roots: string[] | undefined, args: readonly string[], signal: AbortSignal): Promise<{thread:RecordChoice;snapshotId:string}> {
  // Identity resolution is independent of dated activity; subsequent reads retain the requested window.
  const identity=await client.conversation!({action:'resolve',roots,selection:selection(scope),query:record},{signal});
  if(identity.record)return {thread:identity.record,snapshotId:identity.snapshotId};
  const response=await client.conversation!({action:'records',roots,selection:selection(scope),query:record,limit:200},{signal});
  const items:RecordChoice[]=response.items.flatMap(i=>i.evidence.threadId?[{id:i.evidence.threadId,upstreamId:i.record?.upstreamId,title:i.title,project:i.project,lastActivityAt:i.record?.lastActivityAt}]:[]);
  const exact=items.filter(i=>i.id===record||i.upstreamId===record);
  if(exact.length===1)return {thread:exact[0],snapshotId:response.snapshotId};
  if(items.length===1&&response.scanComplete&&!response.partial)return {thread:items[0],snapshotId:response.snapshotId};
  if(!items.length){
    if(!response.scanComplete)throw new CoreError('SCAN_INCOMPLETE',t('cli.history.scanIncomplete'),{next:{method:'conversation',params:{action:'records',roots,selection:selection(scope),query:record,limit:200,cursor:response.nextCursor}}});
    throw new CoreError(response.partial?'SYNC_PENDING':'NOT_FOUND',t('cli.history.noMatch'));
  }
  const candidates=items.slice(0,20).map((i,n)=>`${n+1}. ${terminalText(i.title??i.id)} · ${terminalText(i.project??'—')} · ${i.lastActivityAt??'—'} · ${i.id}`).join('\n');
  if(!canPrompt(args))throw new CoreError('AMBIGUOUS_RECORD',t('cli.history.ambiguous')+'\n'+candidates);
  process.stderr.write(candidates+'\n');
  const choice=await ask(t('cli.history.choose'),signal);
  if(!choice)throw new CoreError('CANCELLED',t('common.cancelled'));
  return {thread:items[pageNumber(choice,0,1,Math.min(20,items.length))-1],snapshotId:response.snapshotId};
}
function scopeArgs(scope:UsageScope,roots:string[]|undefined):string[]{
  const args=['--timezone',scope.timezone??'UTC',...(roots??[]).flatMap(r=>['--root',r])];
  if(scope.project)args.push('--project',scope.project);
  if(scope.sourceInstanceId)args.push('--source',scope.sourceInstanceId);
  if(scope.allTime)args.push('--all-time');else if(scope.since&&scope.until)args.push('--since',scope.since,'--until',scope.until);
  return args;
}
function selection(scope: UsageScope): NonNullable<ConversationRequest['selection']> {
  return {since:scope.since,until:scope.until,timezone:scope.timezone,includeUndated:scope.allTime ?? scope.undated ?? false,projects:scope.project?[scope.project]:[],sources:scope.sourceInstanceId?[scope.sourceInstanceId]:[]};
}
export async function runHistoryCli(argv: readonly string[]): Promise<number> {
  return withCancellation(async signal=>{
    const args = [...argv], action = args[0] && !args[0].startsWith('-') ? args.shift()! : 'list';
    if (!['list','search','show','read'].includes(action)) invalidInput(action);
    const p = parseOptions(args,['--project','--since','--until','--timezone','--source','--root','--limit','--offset','--turn','--find','--cursor','--event','--event-offset','--event-source','--event-turn','--max-bytes'],['--all-time','--tools'],['--root']);
    if(p.help){writeHelp('history',argv);return 0;}
    if(p.flags.has('--tools')&&action!=='read')invalidInput('--tools');
    if (p.positionals.length !== (action === 'list' ? 0 : 1)) invalidInput(action);
    if (action !== 'read' && ['--find','--event','--event-offset','--event-source','--event-turn','--max-bytes'].some(k=>p.values.has(k))) invalidInput(action);
    if(['list','search'].includes(action)&&p.values.has('--turn'))invalidInput('--turn');
    if(action==='show'&&p.values.has('--cursor'))invalidInput('--cursor');
    if(p.values.has('--event')&&p.values.has('--find')||!p.values.has('--event')&&['--event-source','--event-turn','--event-offset'].some(k=>p.values.has(k)))invalidInput('read selection');
    if (action !== 'list' && p.values.has('--offset')) invalidInput('--offset');
    if (action === 'read' && p.values.has('--limit')) invalidInput('--limit');
    const explicitDates = p.values.has('--since') || p.values.has('--until') || p.flags.has('--all-time');
    const scope: UsageScope = {...dateWindow({since:p.get('--since'),until:p.get('--until'),timezone:p.get('--timezone'),allTime:p.flags.has('--all-time') || undefined},'recent'),...(p.get('--project')?{project:projectFilter(p.get('--project')!)}:{}),...(p.get('--source')?{sourceInstanceId:p.get('--source')}: {})};
    const roots = p.all('--root').length?p.all('--root').map(r=>path.resolve(r)):undefined;
    const client = createNodeClient({automaticPrices:false}), options = {signal};
    const limit = pageNumber(p.get('--limit'),20,1,200);
    if (action === 'list') {
      const r=await client.conversation!({action:'records',roots,selection:selection(scope),limit,offset:pageNumber(p.get('--offset'),0),cursor:p.get('--cursor')},options);
      process.stdout.write(p.json?JSON.stringify(r)+'\n':r.items.map(i=>`${terminalText(i.title??i.evidence.threadId??'—')}\t${terminalText(i.project??'—')}\t${i.record?.lastActivityAt??'—'}\t${i.evidence.threadId}`).join('\n')+'\n'+t(r.scanComplete?'cli.history.complete':'cli.history.more')+'\n');
      if(!p.json&&r.nextCursor)process.stdout.write(commandLine(['history',...scopeArgs(scope,roots),'--cursor',r.nextCursor,'--offset',String(pageNumber(p.get('--offset'),0)),'--limit',String(limit)])+'\n');
      return r.partial?2:0;
    }
    if (action === 'search') {
      const r = await client.conversation!({action:'search',roots,selection:selection(scope),query:p.positionals[0],limit,cursor:p.get('--cursor')},options);
      process.stdout.write(p.json?JSON.stringify(r)+'\n':r.items.map(i=>`${terminalText(i.title ?? i.evidence.threadId ?? '—')}\t${i.timestamp ?? '—'}\t${i.evidence.threadId}\t${i.evidence.turnId ?? '—'}\t${i.evidence.eventId}`).join('\n')+'\n'+t(r.scanComplete?'cli.history.complete':'cli.history.more')+'\n');
      if(!p.json&&r.nextCursor)process.stdout.write(commandLine(['history','search',p.positionals[0],...scopeArgs(scope,roots),'--cursor',r.nextCursor,'--limit',String(limit)])+'\n');
      return r.partial?2:0;
    }
    const chosen = await resolveRecord(client,p.positionals[0],scope,roots,argv,signal);
    const retainedScope: UsageScope = explicitDates ? scope : {...scope,since:undefined,until:undefined,allTime:true};
    if (action === 'show') {
      const r = (await client.live!({query:{action:p.get('--turn')?'steps':'turns',threadId:chosen.thread.id,turnId:p.get('--turn'),scope:retainedScope,limit,compact:true},mode:'cached'},options)).result;
      process.stdout.write(p.json?JSON.stringify(r)+'\n':terminalText(chosen.thread.title ?? chosen.thread.id)+'\n'+r.items.map(i=>i.kind==='turn'?`${i.id}\t${i.ordinal ?? '—'}\t${i.startedAt ?? '—'}\t${i.status}`:i.kind==='operation'?`${i.id}\t${terminalText(i.name)}\t${i.status}`:'').filter(Boolean).join('\n')+'\n');
      return resultExitCode(r);
    }
    const selected = {...selection(retainedScope),includeToolOutput:p.flags.has('--tools'),threads:[chosen.thread.id],turns:p.get('--turn')?[p.get('--turn')!]:[]};
    let evidence:NonNullable<ConversationRequest['targets']>[number]['evidence']|undefined, matchCursor:string|null|undefined, nextLocations:ConversationRequest|null=null;
    if (p.get('--event')) {
      if (!p.get('--event-source') || !p.values.has('--event-offset')) invalidInput('--event');
      evidence = {eventId:p.get('--event')!,sourceInstanceId:p.get('--event-source')!,threadId:chosen.thread.id,turnId:p.get('--event-turn'),eventOffset:pageNumber(p.get('--event-offset'),0)};
    } else {
      const locationQuery:ConversationRequest={action:p.get('--find')?'search':'locate',roots,selection:selected,query:p.get('--find'),limit:1,cursor:p.get('--cursor')};
      const locations=await client.conversation!(locationQuery,options);
      const item=locations.items.find(i=>i.hasTextLocator);evidence=item?.evidence;matchCursor=item?.readCursor;
      if(locations.nextCursor)nextLocations={...locationQuery,cursor:locations.nextCursor};
      if (!evidence) {process.stdout.write(p.json?JSON.stringify({outputVersion:2,result:locations,nextLocations} satisfies HistoryRead)+'\n':t('cli.history.noText')+'\n');return locations.partial?2:0;}
    }
    const r = await client.conversation!({action:'read',roots,selection:selected,targets:[{evidence,cursor:p.get('--event')?p.get('--cursor'):matchCursor,maxBytes:pageNumber(p.get('--max-bytes'),8192,1,65536),before:1,after:1}]},options);
    const output:HistoryRead={outputVersion:2,result:r,nextLocations};
    if(p.json)process.stdout.write(JSON.stringify(output)+'\n');
    else{
      for(const f of r.fragments){
        process.stdout.write(`${f.message.timestamp??'—'} · ${messageOriginText(f.message.origin)}\n${terminalBlock(f.text)}\n`+f.locations.map(l=>`${terminalText(l.file)}:${l.byteStart}-${l.byteEnd}`).join('\n')+'\n');
        if(f.nextCursor){
          const args=['history','read',chosen.thread.id,...scopeArgs(retainedScope,roots),'--event',f.evidence.eventId,'--event-source',f.evidence.sourceInstanceId,'--event-offset',String(f.evidence.eventOffset),'--cursor',f.nextCursor];
          if(p.flags.has('--tools'))args.push('--tools');
          if(p.get('--turn'))args.push('--turn',p.get('--turn')!);
          if(f.evidence.turnId)args.push('--event-turn',f.evidence.turnId);
          process.stdout.write(commandLine(args)+'\n');
        }
      }
      if(nextLocations){const args=['history','read',chosen.thread.id,...scopeArgs(retainedScope,roots),'--cursor',nextLocations.cursor!];if(p.flags.has('--tools'))args.push('--tools');if(p.get('--find'))args.push('--find',p.get('--find')!);if(p.get('--turn'))args.push('--turn',p.get('--turn')!);process.stdout.write(commandLine(args)+'\n');}
    }
    return r.partial?2:0;
  });
}
