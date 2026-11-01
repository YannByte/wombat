import {writeHelp} from './help.js';
import path from 'node:path';
import {CoreError, type UsageClient, type UsageScope, type HistoryRead, type ConversationRequest} from '@wombat/client';
import {createNodeClient} from '@wombat/client/node';
import {t,messageOriginText} from '@wombat/client/locale';
import {dateWindow} from './calendar.js';
import {ask, canPrompt, commandLine, invalidInput, pageNumber, parseOptions, withCancellation} from './human-input.js';
import {terminalText,terminalBlock} from './display-text.js';
import {resultExitCode} from './exit-codes.js';

type RecordChoice={id:string;upstreamId?:string;title?:string|null;project?:string|null;lastActivityAt?:string|null};
export async function resolveRecord(client: UsageClient, record: string, scope: UsageScope, roots: string[] | undefined, args: readonly string[], signal: AbortSignal, snapshot?: string, allowExactOutsideWindow=false): Promise<{thread:RecordChoice;snapshotId:string}> {
  const snapshotId = snapshot ?? (await client.live!({query:{action:'usage',scope,roots,limit:1,compact:true},mode:'auto'},{signal})).result.snapshotRef.snapshotId;
  const response=await client.conversation!({action:'records',snapshotId,roots,selection:selection(scope),query:record,limit:200},{signal});
  const items:RecordChoice[]=response.items.flatMap(i=>i.evidence.threadId?[{id:i.evidence.threadId,upstreamId:i.record?.upstreamId,title:i.title,project:i.project,lastActivityAt:i.record?.lastActivityAt}]:[]);
  if(!items.length&&allowExactOutsideWindow){
    const outside=await client.conversation!({action:'records',snapshotId,roots,selection:{...selection(scope),since:undefined,until:undefined,includeUndated:true},query:record,limit:200},{signal});
    const exact=outside.items.find(i=>i.evidence.threadId===record||i.record?.upstreamId===record);
    if(exact?.evidence.threadId)return {thread:{id:exact.evidence.threadId,upstreamId:exact.record?.upstreamId,title:exact.title,project:exact.project,lastActivityAt:exact.record?.lastActivityAt},snapshotId};
  }
  const exact=items.filter(i=>i.id===record||i.upstreamId===record);
  if(exact.length===1)return {thread:exact[0],snapshotId};
  if(items.length===1&&response.scanComplete&&!response.partial)return {thread:items[0],snapshotId};
  if(!items.length)throw new CoreError(response.scanComplete&&!response.partial?'NOT_FOUND':'SYNC_PENDING',t('cli.history.noMatch'));
  const candidates=items.slice(0,20).map((i,n)=>`${n+1}. ${terminalText(i.title??i.id)} · ${terminalText(i.project??'—')} · ${i.lastActivityAt??'—'} · ${i.id}`).join('\n');
  if(!canPrompt(args))throw new CoreError('AMBIGUOUS_RECORD',t('cli.history.ambiguous')+'\n'+candidates);
  process.stderr.write(candidates+'\n');
  const choice=await ask(t('cli.history.choose'),signal);
  if(!choice)throw new CoreError('CANCELLED',t('common.cancelled'));
  return {thread:items[pageNumber(choice,0,1,Math.min(20,items.length))-1],snapshotId};
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
    const p = parseOptions(args,['--project','--since','--until','--timezone','--source','--root','--limit','--offset','--turn','--find','--snapshot','--cursor','--event','--event-offset','--event-source','--event-turn','--max-bytes'],['--all-time'],['--root']);
    if(p.help){writeHelp('history',argv);return 0;}
    if (p.positionals.length !== (action === 'list' ? 0 : 1)) invalidInput(action);
    if (action !== 'read' && ['--find','--event','--event-offset','--event-source','--event-turn','--max-bytes'].some(k=>p.values.has(k))) invalidInput(action);
    if(['list','search'].includes(action)&&p.values.has('--turn'))invalidInput('--turn');
    if(action==='show'&&p.values.has('--cursor'))invalidInput('--cursor');
    if(p.values.has('--event')&&p.values.has('--find')||!p.values.has('--event')&&['--event-source','--event-turn','--event-offset'].some(k=>p.values.has(k)))invalidInput('read selection');
    if (action !== 'list' && p.values.has('--offset')) invalidInput('--offset');
    if (action === 'read' && p.values.has('--limit')) invalidInput('--limit');
    const explicitDates = p.values.has('--since') || p.values.has('--until') || p.flags.has('--all-time');
    const scope: UsageScope = {...dateWindow({since:p.get('--since'),until:p.get('--until'),timezone:p.get('--timezone'),allTime:p.flags.has('--all-time') || undefined},'recent'),...(p.get('--project')?{project:path.resolve(p.get('--project')!)}:{}),...(p.get('--source')?{sourceInstanceId:p.get('--source')}: {})};
    const roots = p.all('--root').length?p.all('--root').map(r=>path.resolve(r)):undefined;
    const client = createNodeClient({automaticPrices:false}), options = {signal};
    const limit = pageNumber(p.get('--limit'),20,1,200);
    if (action === 'list') {
      const snapshotId=p.get('--snapshot')??(await client.live!({query:{action:'usage',roots,scope,limit:1,compact:true},mode:'auto'},options)).result.snapshotRef.snapshotId;
      const r=await client.conversation!({action:'records',snapshotId,roots,selection:selection(scope),limit,offset:pageNumber(p.get('--offset'),0),cursor:p.get('--cursor')},options);
      process.stdout.write(p.json?JSON.stringify(r)+'\n':r.items.map(i=>`${terminalText(i.title??i.evidence.threadId??'—')}\t${terminalText(i.project??'—')}\t${i.record?.lastActivityAt??'—'}\t${i.evidence.threadId}`).join('\n')+'\n'+t(r.scanComplete?'cli.history.complete':'cli.history.more')+'\n');
      if(!p.json&&r.nextCursor)process.stdout.write(commandLine(['history',...scopeArgs(scope,roots),'--snapshot',r.snapshotId,'--cursor',r.nextCursor,'--offset',String(pageNumber(p.get('--offset'),0)),'--limit',String(limit)])+'\n');
      return r.partial?2:0;
    }
    if (action === 'search') {
      const initial = (await client.live!({query:{action:'threads',roots,scope,limit:1,compact:true,snapshotId:p.get('--snapshot')},mode:'auto'},options)).result;
      const r = await client.conversation!({action:'search',snapshotId:initial.snapshotRef.snapshotId,roots,selection:selection(scope),query:p.positionals[0],limit,cursor:p.get('--cursor')},options);
      process.stdout.write(p.json?JSON.stringify(r)+'\n':r.items.map(i=>`${terminalText(i.title ?? i.evidence.threadId ?? '—')}\t${i.timestamp ?? '—'}\t${i.evidence.threadId}\t${i.evidence.turnId ?? '—'}\t${i.evidence.eventId}`).join('\n')+'\n'+t(r.scanComplete?'cli.history.complete':'cli.history.more')+'\n');
      if(!p.json&&r.nextCursor)process.stdout.write(commandLine(['history','search',p.positionals[0],...scopeArgs(scope,roots),'--snapshot',r.snapshotId,'--cursor',r.nextCursor,'--limit',String(limit)])+'\n');
      return r.partial?2:0;
    }
    const chosen = await resolveRecord(client,p.positionals[0],scope,roots,argv,signal,p.get('--snapshot'),!explicitDates);
    const retainedScope: UsageScope = explicitDates ? scope : {...scope,since:undefined,until:undefined,allTime:true};
    const snapshotId = p.get('--snapshot') ?? chosen.snapshotId;
    if (action === 'show') {
      const r = (await client.live!({query:{action:p.get('--turn')?'steps':'turns',threadId:chosen.thread.id,turnId:p.get('--turn'),scope:retainedScope,snapshotId,limit,compact:true},mode:'cached'},options)).result;
      process.stdout.write(p.json?JSON.stringify(r)+'\n':terminalText(chosen.thread.title ?? chosen.thread.id)+'\n'+r.items.map(i=>i.kind==='turn'?`${i.id}\t${i.ordinal ?? '—'}\t${i.startedAt ?? '—'}\t${i.status}`:i.kind==='operation'?`${i.id}\t${terminalText(i.name)}\t${i.status}`:'').filter(Boolean).join('\n')+'\n');
      return resultExitCode(r);
    }
    const selected = {...selection(retainedScope),threads:[chosen.thread.id],turns:p.get('--turn')?[p.get('--turn')!]:[]};
    let evidence:NonNullable<ConversationRequest['targets']>[number]['evidence']|undefined, matchCursor:string|null|undefined, nextLocations:ConversationRequest|null=null;
    if (p.get('--event')) {
      if (!p.get('--event-source') || !p.values.has('--event-offset')) invalidInput('--event');
      evidence = {eventId:p.get('--event')!,sourceInstanceId:p.get('--event-source')!,threadId:chosen.thread.id,turnId:p.get('--event-turn'),eventOffset:pageNumber(p.get('--event-offset'),0)};
    } else {
      const locationQuery:ConversationRequest={action:p.get('--find')?'search':'locate',snapshotId,roots,selection:selected,query:p.get('--find'),limit:1,cursor:p.get('--cursor')};
      const locations=await client.conversation!(locationQuery,options);
      const item=locations.items.find(i=>i.hasTextLocator);evidence=item?.evidence;matchCursor=item?.readCursor;
      if(locations.nextCursor)nextLocations={...locationQuery,cursor:locations.nextCursor};
      if (!evidence) {process.stdout.write(p.json?JSON.stringify({outputVersion:1,result:locations,nextLocations} satisfies HistoryRead)+'\n':t('cli.history.noText')+'\n');return locations.partial?2:0;}
    }
    const r = await client.conversation!({action:'read',snapshotId,roots,selection:selected,targets:[{evidence,cursor:p.get('--event')?p.get('--cursor'):matchCursor,maxBytes:pageNumber(p.get('--max-bytes'),8192,1,65536),before:1,after:1}]},options);
    const output:HistoryRead={outputVersion:1,result:r,nextLocations};
    if(p.json)process.stdout.write(JSON.stringify(output)+'\n');
    else{
      for(const f of r.fragments){
        process.stdout.write(`${f.message.timestamp??'—'} · ${messageOriginText(f.message.origin)}\n${terminalBlock(f.text)}\n`+f.locations.map(l=>`${terminalText(l.file)}:${l.byteStart}-${l.byteEnd}`).join('\n')+'\n');
        if(f.nextCursor){
          const args=['history','read',chosen.thread.id,...scopeArgs(retainedScope,roots),'--snapshot',r.snapshotId,'--event',f.evidence.eventId,'--event-source',f.evidence.sourceInstanceId,'--event-offset',String(f.evidence.eventOffset),'--cursor',f.nextCursor];
          if(p.get('--turn'))args.push('--turn',p.get('--turn')!);
          if(f.evidence.turnId)args.push('--event-turn',f.evidence.turnId);
          process.stdout.write(commandLine(args)+'\n');
        }
      }
      if(nextLocations){const args=['history','read',chosen.thread.id,...scopeArgs(retainedScope,roots),'--snapshot',r.snapshotId,'--cursor',nextLocations.cursor!];if(p.get('--find'))args.push('--find',p.get('--find')!);if(p.get('--turn'))args.push('--turn',p.get('--turn')!);process.stdout.write(commandLine(args)+'\n');}
    }
    return r.partial?2:0;
  });
}
