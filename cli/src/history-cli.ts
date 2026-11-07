import {writeHelp} from './help.js';
import path from 'node:path';
import {CoreError, parseConversationResult, type UsageClient, type UsageScope, type HistoryRead, type ConversationRequest, type ConversationResult} from '@wombat/client';
import {createNodeClient} from '@wombat/client/node';
import {t,messageOriginText} from '@wombat/client/locale';
import {dateWindow} from './calendar.js';
import {ask, canPrompt, commandLine, invalidInput, pageNumber, parseOptions, projectFilter, withCancellation} from './human-input.js';
import {terminalText,terminalBlock} from './display-text.js';
import {resultExitCode} from './exit-codes.js';
import {readJsonInput} from './json-input.js';
import {OUTPUT_BYTES} from './agent-limits.js';

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
function searchInput(terms: readonly string[]): Pick<ConversationRequest,'query'|'terms'> {
  if(terms.length>16||terms.some(term=>!term.trim()||Buffer.byteLength(term)>4096))invalidInput('search terms (1–16 nonblank terms, up to 4096 UTF-8 bytes each)');
  return terms.length===0?{}:terms.length===1?{query:terms[0]}:{terms:[...terms]};
}
type ReadTargets = NonNullable<ConversationRequest['targets']>;
type Occurrence = Omit<ConversationResult['fragments'][number],'text'>;

function historyOutput(result: ConversationResult, request: ConversationRequest, nextLocations?: ConversationRequest | null): HistoryRead {
  const targets = request.targets ?? [];
  const deferred = (result.readIssues ?? []).filter(issue=>issue.deferred).flatMap(issue=>issue.targetIndices);
  const retryRead = deferred.length ? {...request,targets:deferred.map(i=>targets[i])} : undefined;
  return {outputVersion:3,result,nextLocations,retryRead};
}

function renderFragments(result: ConversationResult, writeOccurrence?: (fragment: Occurrence) => void): void {
  const neighbors = new Map(result.neighbors?.map(body=>[body.message.evidence.eventId,body]));
  const printed = new Set<string>();
  const showNeighbors = (ids?: string[]) => {
    for (const id of ids ?? []) {
      const body = neighbors.get(id);
      if (!body || printed.has(id)) continue;
      printed.add(id);
      process.stdout.write(`${body.message.timestamp??'—'} · ${messageOriginText(body.message.origin)}\n${terminalBlock(body.text)}\n`);
    }
  };
  for(const fragment of result.fragments){
    showNeighbors(fragment.visibleContext?.before);
    process.stdout.write(`${fragment.message.timestamp??'—'} · ${messageOriginText(fragment.message.origin)}\n${terminalBlock(fragment.text)}\n`);
    writeOccurrence?.(fragment);
    showNeighbors(fragment.visibleContext?.after);
    for(const duplicate of fragment.alsoAt??[]){
      showNeighbors(duplicate.visibleContext?.before);
      process.stdout.write(`${duplicate.message.timestamp??'—'} · ${messageOriginText(duplicate.message.origin)}\n`);
      writeOccurrence?.(duplicate);
      showNeighbors(duplicate.visibleContext?.after);
    }
  }
}

async function readSearchInput(client: UsageClient, signal: AbortSignal, json: boolean, maxBytes: number, contextMessages?: number): Promise<number> {
  if (process.stdin.isTTY) invalidInput('--from - requires search JSON on stdin');
  const search = await parseConversationResult(await readJsonInput(process.stdin,signal,OUTPUT_BYTES));
  if (search.action !== 'search') invalidInput('--from - requires JSON from history search --locate');
  const targets = search.items.filter(item=>item.hasTextLocator)
    .map(item=>({evidence:item.evidence,cursor:item.readCursor,maxBytes}));
  if (!targets.length) {
    const output:HistoryRead = {outputVersion:3,result:search,nextLocations:search.nextSearch};
    process.stdout.write(json?JSON.stringify(output)+'\n':t('cli.history.noText')+'\n');
    return search.partial || !search.scanComplete ? 2 : 0;
  }
  const request:ConversationRequest = {action:'read',roots:search.roots,selection:search.selection,
    sourceScope:search.sourceScope,contextMessages,targets};
  const result=await client.conversation!(request,{signal});
  const output=historyOutput(result,request,search.nextSearch);
  if(json) process.stdout.write(JSON.stringify(output)+'\n');
  else {
    renderFragments(result,f=>process.stdout.write(f.locations.map(l=>`${terminalText(l.file)}:${l.byteStart}-${l.byteEnd}`).join('\n')+'\n'));
    for(const issue of result.readIssues??[])process.stdout.write(t('cli.history.readIssue',{id:issue.eventId,code:issue.code})+'\n');
    if(!search.scanComplete || output.retryRead)process.stdout.write(t('cli.history.more')+'\n');
  }
  return result.partial || search.partial || !search.scanComplete ? 2 : 0;
}

export async function runHistoryCli(argv: readonly string[]): Promise<number> {
  return withCancellation(async signal=>{
    const args = [...argv], action = args[0] && !args[0].startsWith('-') ? args.shift()! : 'list';
    if (!['list','search','show','read'].includes(action)) invalidInput(action);
    const p = parseOptions(args,['--project','--since','--until','--timezone','--source','--root','--limit','--offset','--turn','--find','--cursor','--event','--event-offset','--event-source','--event-turn','--max-bytes','--operation','--source-scope','--from','--context','--speaker'],['--all-time','--tools','--fixed','--complete','--locate'],['--root','--find']);
    if(p.help){writeHelp('history',argv);return 0;}
    const maxBytes = pageNumber(p.get('--max-bytes'),8192,1,65536);
    const contextMessages = p.values.has('--context') ? pageNumber(p.get('--context'),1,1,5) : undefined;
    if (p.values.has('--from')) {
      if (action!=='read' || p.get('--from')!=='-' || p.positionals.length ||
          [...p.values.keys()].some(k=>!['--from','--max-bytes','--context'].includes(k)) ||
          [...p.flags].some(k=>!['--json','--no-input'].includes(k))) invalidInput('--from -');
      return readSearchInput(createNodeClient({automaticPrices:false}),signal,p.json,maxBytes,contextMessages);
    }
    const locate = p.flags.has('--locate');
    if (locate && action !== 'search') invalidInput('--locate');
    const direct = action === 'search' && !locate;
    const complete = p.flags.has('--complete');
    if (complete && action !== 'search') invalidInput('--complete');
    if (contextMessages && (!direct && action !== 'read' || p.values.has('--operation'))) invalidInput('--context');
    const speaker=p.get('--speaker');
    if(speaker && (action!=='search'||!['user','assistant'].includes(speaker)))invalidInput('--speaker');
    if(p.flags.has('--tools')&&action!=='read')invalidInput('--tools');
    if (action==='search'?p.positionals.length===0:p.positionals.length!==(action==='list'?0:1)) invalidInput(action);
    if (action !== 'read' && ['--find','--event','--event-offset','--event-source','--event-turn','--operation'].some(k=>p.values.has(k))) invalidInput(action);
    if (p.values.has('--max-bytes') && action !== 'read' && !direct) invalidInput('--max-bytes');
    if(['list','search'].includes(action)&&p.values.has('--turn'))invalidInput('--turn');
    if(action==='show'&&p.values.has('--cursor'))invalidInput('--cursor');
    if(p.values.has('--event')&&p.values.has('--find')||!p.values.has('--event')&&['--event-source','--event-turn','--event-offset'].some(k=>p.values.has(k)))invalidInput('read selection');
    if (action !== 'list' && p.values.has('--offset')) invalidInput('--offset');
    if(p.values.has('--operation')&&!p.get('--turn'))invalidInput('--operation requires --turn');
    if (p.values.has('--event') && p.values.has('--limit')) invalidInput('--limit');
    const fixedScope = p.flags.has('--fixed');
    let sourceScope = p.get('--source-scope');
    if ((fixedScope || sourceScope) && !['search','read'].includes(action)) invalidInput('--fixed / --source-scope');
    if (fixedScope && p.get('--event') && !sourceScope) invalidInput('--source-scope');
    const findTerms=p.all('--find');
    const search=searchInput(action==='search'?p.positionals:findTerms);
    const explicitDates = p.values.has('--since') || p.values.has('--until') || p.flags.has('--all-time');
    const scope: UsageScope = {...dateWindow({since:p.get('--since'),until:p.get('--until'),timezone:p.get('--timezone'),allTime:p.flags.has('--all-time') || undefined},'recent'),...(p.get('--project')?{project:projectFilter(p.get('--project')!)}:{}),...(p.get('--source')?{sourceInstanceId:p.get('--source')}: {})};
    const roots = p.all('--root').length?p.all('--root').map(r=>path.resolve(r)):undefined;
    const client = createNodeClient({automaticPrices:false}), options = {signal};
    const limit = action==='read'?pageNumber(p.get('--limit'),p.get('--operation')?4:1,1,20):pageNumber(p.get('--limit'),complete?200:20,1,200);
    if (action === 'list') {
      const r=await client.conversation!({action:'records',roots,selection:selection(scope),limit,offset:pageNumber(p.get('--offset'),0),cursor:p.get('--cursor')},options);
      process.stdout.write(p.json?JSON.stringify(r)+'\n':r.items.map(i=>`${terminalText(i.title??i.evidence.threadId??'—')}\t${terminalText(i.project??'—')}\t${i.record?.lastActivityAt??'—'}\t${i.evidence.threadId}`).join('\n')+'\n'+t(r.scanComplete?'cli.history.complete':'cli.history.more')+'\n');
      if(!p.json&&r.nextCursor)process.stdout.write(commandLine(['history',...scopeArgs(scope,roots),'--cursor',r.nextCursor,'--offset',String(pageNumber(p.get('--offset'),0)),'--limit',String(limit)])+'\n');
      return r.partial?2:0;
    }
    if (action === 'search') {
      const origins = speaker === 'user' ? ['user_input','user_unclassified'] as const :
        speaker === 'assistant' ? ['assistant_visible'] as const : [];
      const r = await client.conversation!({action:direct?'source_search':'search',excerptBytes:direct?maxBytes:undefined,contextMessages,roots,selection:{...selection(scope),origins:[...origins]},...search,limit,cursor:p.get('--cursor'),fixedScope,sourceScope,complete},options);
      if (p.json) process.stdout.write(JSON.stringify(r)+'\n');
      else {
        if (direct) {
          let currentFile = -1;
          const shown = new Set<number>();
          const showNeighbors = (indices: number[] = []) => {
            for (const index of indices) {
              if (shown.has(index)) continue;
              shown.add(index);
              const n = r.sourceNeighbors![index];
              process.stdout.write(`${n.timestamp ?? '—'} · ${messageOriginText(n.origin)} · ${n.byteStart}-${n.byteEnd}\n${terminalBlock(n.text)}\n\n`);
            }
          };
          for (const hit of r.sourceHits ?? []) {
            if (currentFile !== hit.fileIndex) {
              currentFile = hit.fileIndex;
              const file = r.sourceFiles![currentFile];
              process.stdout.write(`${terminalText(file.file)} · ${file.threadId}\n`);
            }
            showNeighbors(hit.context?.before);
            process.stdout.write(`${hit.timestamp ?? '—'} · ${messageOriginText(hit.origin)} · ${hit.byteStart}-${hit.byteEnd}\n${terminalBlock(hit.text)}\n\n`);
            showNeighbors(hit.context?.after);
          }
        } else {
          for (const item of r.items) {
            process.stdout.write(`${terminalText(item.title ?? item.evidence.threadId ?? '—')}\t${item.timestamp ?? '—'}\t${item.evidence.threadId}\t${item.evidence.turnId ?? '—'}\t${item.evidence.eventId}\t${terminalText(item.matchedTerm ?? '—')}\n`);
          }
        }
        process.stdout.write(t(r.scanComplete?'cli.history.complete':'cli.history.more')+'\n');
      }
      if(!p.json&&r.nextCursor)process.stdout.write(commandLine(['history','search',...scopeArgs(scope,roots),...(locate?['--locate']:['--max-bytes',String(maxBytes)]),...(fixedScope?['--fixed']:[]),...(complete?['--complete']:[]),...(contextMessages?['--context',String(contextMessages)]:[]),...(speaker?['--speaker',speaker]:[]),...(r.sourceScope?['--source-scope',r.sourceScope]:[]),'--cursor',r.nextCursor,'--limit',String(limit),'--',...p.positionals])+'\n');
      return r.partial?2:0;
    }
    const chosen = await resolveRecord(client,p.positionals[0],scope,roots,argv,signal);
    const retainedScope: UsageScope = explicitDates ? scope : {...scope,since:undefined,until:undefined,allTime:true};
    if (action === 'show') {
      const r = (await client.live!({query:{action:p.get('--turn')?'steps':'turns',threadId:chosen.thread.id,turnId:p.get('--turn'),scope:retainedScope,limit,compact:true},mode:'cached'},options)).result;
      process.stdout.write(p.json?JSON.stringify(r)+'\n':terminalText(chosen.thread.title ?? chosen.thread.id)+'\n'+r.items.map(i=>i.kind==='turn'?`${i.id}\t${i.ordinal ?? '—'}\t${i.startedAt ?? '—'}\t${i.status}`:i.kind==='operation'?`${i.id}\t${terminalText(i.name)}\t${i.status}`:'').filter(Boolean).join('\n')+'\n');
      return resultExitCode(r);
    }
    const selected = {...selection(retainedScope),operationId:p.get('--operation'),includeToolOutput:p.flags.has('--tools')||p.values.has('--operation'),threads:[chosen.thread.id],turns:p.get('--turn')?[p.get('--turn')!]:[]};
    const readArgs=['history','read',chosen.thread.id,...scopeArgs(retainedScope,roots),'--max-bytes',String(maxBytes)];
    if(p.flags.has('--tools'))readArgs.push('--tools');
    if(p.get('--operation'))readArgs.push('--operation',p.get('--operation')!);
    if(p.get('--turn'))readArgs.push('--turn',p.get('--turn')!);
    const scopeFlag=()=>sourceScope?['--source-scope',sourceScope]:[];
    const continueLocations=(cursor:string)=>process.stdout.write(commandLine([...readArgs,...scopeFlag(),...(fixedScope?['--fixed']:[]),...(contextMessages?['--context',String(contextMessages)]:[]),'--cursor',cursor,'--limit',String(limit),...findTerms.flatMap(term=>['--find',term])])+'\n');
    let evidence:NonNullable<ConversationRequest['targets']>[number]['evidence']|undefined, nextLocations:ConversationRequest|null=null;
    let located:ReadTargets=[];
    if (p.get('--event')) {
      if (!p.get('--event-source') || !p.values.has('--event-offset')) invalidInput('--event');
      evidence = {eventId:p.get('--event')!,sourceInstanceId:p.get('--event-source')!,threadId:chosen.thread.id,turnId:p.get('--event-turn'),eventOffset:pageNumber(p.get('--event-offset'),0)};
    } else {
      const locationQuery:ConversationRequest={action:findTerms.length?'search':'locate',roots,selection:selected,...search,limit,cursor:p.get('--cursor'),fixedScope,sourceScope};
      const locations=await client.conversation!(locationQuery,options);
      sourceScope=locations.sourceScope??undefined;
      located=locations.items.filter(i=>i.hasTextLocator).map(i=>({evidence:i.evidence,cursor:i.readCursor}));
      const item=locations.items.find(i=>i.hasTextLocator);evidence=item?.evidence;
      if(locations.nextCursor)nextLocations={...locationQuery,cursor:locations.nextCursor,sourceScope};
      if (!evidence) {
        process.stdout.write(p.json?JSON.stringify({outputVersion:3,result:locations,nextLocations} satisfies HistoryRead)+'\n':t('cli.history.noText')+'\n');
        if(!p.json&&locations.nextCursor)continueLocations(locations.nextCursor);
        return locations.partial?2:0;
      }
    }
    const targets=(p.get('--event')?[{evidence,cursor:p.get('--cursor')}]:located)
      .map(target=>({...target,maxBytes,...(contextMessages?{}:{before:1,after:1})}));
    const request:ConversationRequest={action:'read',roots,selection:selected,sourceScope,targets,contextMessages};
    const r = await client.conversation!(request,options);
    const output=historyOutput(r,request,nextLocations);
    if(p.json)process.stdout.write(JSON.stringify(output)+'\n');
    else{
      const writeTarget=(evidence:typeof targets[number]['evidence'],cursor?:string|null)=>{
        const args=[...readArgs,...scopeFlag(),'--event',evidence.eventId,'--event-source',evidence.sourceInstanceId,'--event-offset',String(evidence.eventOffset)];
        if(cursor)args.push('--cursor',cursor);
        if(evidence.turnId)args.push('--event-turn',evidence.turnId);
        process.stdout.write(commandLine(args)+'\n');
      };
      const writeOccurrence=(f:Omit<ConversationResult['fragments'][number],'text'>)=>{
        process.stdout.write(f.locations.map(l=>`${terminalText(l.file)}:${l.byteStart}-${l.byteEnd}`).join('\n')+'\n');
        if(f.nextCursor)writeTarget(f.evidence,f.nextCursor);
      };
      renderFragments(r,writeOccurrence);
      for(const issue of r.readIssues??[]){
        process.stdout.write(t('cli.history.readIssue',{id:issue.eventId,code:issue.code})+'\n');
        if(issue.deferred){
          const target=targets[issue.targetIndices[0]];
          writeTarget(target.evidence,target.cursor);
        }
      }
      if(nextLocations?.cursor)continueLocations(nextLocations.cursor);
    }
    return r.partial?2:0;
  });
}
