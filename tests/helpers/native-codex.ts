import { createRequire } from 'node:module';
import { readFile, writeFile } from 'node:fs/promises';
import { connect } from 'node:net';
import path from 'node:path';

function cleanupUnconfirmed(message = 'Synthetic native processes did not exit after product cleanup') {
  return Object.assign(new Error(message), { code: 'NATIVE_CLEANUP_UNCONFIRMED' });
}

/** A PID or port can be reused; only the fixture's own random identity establishes liveness. */
function fixtureAlive(port: number, identity: string, remainingMs: number): Promise<boolean> {
  return new Promise((resolve, reject) => {
    const socket = connect(port, '127.0.0.1');
    let result = '', settled = false;
    const finish = (error?: Error) => {
      if (settled) return;
      settled = true;
      clearTimeout(timer);
      socket.destroy();
      if (error) reject(error); else resolve(result === identity);
    };
    const timer = setTimeout(() => finish(cleanupUnconfirmed('Synthetic lifecycle probe timed out')), remainingMs);
    socket.setEncoding('utf8');
    socket.on('data', chunk => {
      result += chunk;
      if (Buffer.byteLength(result) > 128) { result = ''; finish(); }
    });
    socket.once('end', () => finish());
    socket.once('close', () => finish());
    socket.once('error', error => {
      if ('code' in error && ['ECONNREFUSED', 'ECONNRESET'].includes(String(error.code))) finish();
      else finish(error);
    });
  });
}
/** Synthetic native protocol only: no login, model work, or source-file execution. */
export async function nativeCodexFixture(dir: string) {
  const binary=path.join(dir,'codex.cjs'),mode=path.join(dir,'native-mode.json'),calls=path.join(dir,'native-calls.jsonl'),lifecycle=path.join(dir,'native-lifecycle.jsonl');
  const ws=createRequire(path.resolve('client/package.json')).resolve('ws');
  await writeFile(mode,JSON.stringify({kind:'blocked'}));await writeFile(calls,'');await writeFile(lifecycle,'');
  await writeFile(binary,`const {readFileSync,appendFileSync,writeFileSync}=require('node:fs');
const mode=JSON.parse(readFileSync(${JSON.stringify(mode)},'utf8'));let accountReads=0,hookReads=0;
if(mode.permissionDenied&&!process.argv.includes('--version')){process.stderr.write('Error: listen EPERM: operation not permitted SYNTHETIC_PRIVATE_NATIVE_PATH\\n');process.exit(1);}
const identity=require('node:crypto').randomUUID();
// A peer can disconnect while cleanup stops this fixture; it does not invalidate other probes.
const monitor=require('node:net').createServer(socket=>{socket.on('error',()=>{});socket.end(identity);});
monitor.listen(0,'127.0.0.1',()=>appendFileSync(${JSON.stringify(lifecycle)},JSON.stringify({pid:process.pid,event:'spawn',port:monitor.address().port,identity,proxy:process.argv.includes('proxy')})+'\\n'));
process.once('SIGTERM',()=>process.exit(0));
process.once('exit',()=>appendFileSync(${JSON.stringify(lifecycle)},JSON.stringify({pid:process.pid,event:'exit'})+'\\n'));
if(process.argv.includes('--version')){console.log('codex-cli '+(mode.version||'0.160.0'));process.exit(0);}
if(process.argv.includes('daemon'))process.exit(0);
function reply(m){if(m.id==null)return null;appendFileSync(${JSON.stringify(calls)},JSON.stringify({method:m.method,...(m.method==='thread/queue/add'?{params:m.params}:{})})+'\\n');let result={};
if(m.method==='skills/list'){if(mode.skillsError)return {id:m.id,error:{code:-1,message:'Synthetic discovery error'}};result={data:[{cwd:m.params.cwds[0],errors:[],skills:mode.skills??[{name:'wombat',path:m.params.cwds[0]+'/.agents/skills/wombat/SKILL.md',enabled:true}]}]};}
if(m.method==='hooks/list'){
 hookReads++;
 if(mode.mutateOnHookList&&hookReads===2)writeFileSync(mode.mutateOnHookList,mode.mutatedHookText);
 if(mode.hooksError)return {id:m.id,error:{code:-1,message:'Synthetic private error'}};
 result=hookReads===2&&mode.hooksAfter?mode.hooksAfter:mode.hooks||{data:[]};
}
if(m.method==='config/read')result={config:{model:mode.configModel||'synthetic-model',model_provider:mode.provider||'openai'},origins:{}};
if(m.method==='account/read')result={requiresOpenaiAuth:true,account:{type:'chatgpt',email:'synthetic@example.invalid',planType:'pro'},workspaceRouting:{chatgptAccountId:mode.kind==='switched'&&++accountReads>1?'account-b':'account-a'}};
if(m.method==='account/rateLimits/read'){
 if(mode.kind==='unknown')return {id:m.id,error:{code:-1,message:'Synthetic private error'}};
 const reset=Math.floor(Date.now()/1000)+(mode.kind==='expired'?-1:mode.resetAfter??3600);
 result={accountId:'account-a',ordinaryUsageAllowed:mode.kind==='available',rateLimitUpsell:['blocked','foreign','expired','switched'].includes(mode.kind)?{banner_type:'selected_model_limit',blocked_model_slug:mode.kind==='foreign'?'other-model':'synthetic-model',reset_at:reset}:null,rateLimitsByLimitId:{[mode.kind==='foreign'?'base_model_inference':'codex']:{normalModelSlug:mode.kind==='foreign'?'synthetic-model':null,primary:{usedPercent:mode.kind==='low'?92:mode.kind==='available'?20:100,windowDurationMins:17,resetsAt:reset},rateLimitReachedType:['blocked','foreign','expired','switched'].includes(mode.kind)?'rate_limit_reached':null}}};
}
if(m.method==='thread/start'&&mode.skillsAfterStart)writeFileSync(${JSON.stringify(mode)},JSON.stringify({...mode,skills:mode.skillsAfterStart}));
if(m.method==='thread/start'&&mode.mutateOnStart)writeFileSync(mode.mutateOnStart,'changed after review');
if(m.method==='thread/start')result={cwd:m.params.cwd,model:mode.actualModel||'synthetic-model',modelProvider:mode.provider||'openai',thread:{id:'00000000-0000-4000-8000-000000000001',cwd:m.params.cwd}};
if(m.method==='thread/queue/add'){
 if(mode.queueBehavior==='rejected')return {id:m.id,error:{code:-1,message:'Synthetic private rejection'}};
 if(mode.queueBehavior==='disconnect')process.exit(0);
 result={queuedSubmission:{clientUserMessageId:mode.queueBehavior==='mismatch'?'different-message':m.params.clientUserMessageId,input:mode.queueSkillMissing?m.params.input.filter(i=>i.type!=='skill'):m.params.input}};
}
return {id:m.id,result};}
function respond(m,send){const result=reply(m);if(!result)return;const delay=m.method==='thread/queue/add'?mode.queueDelayMs:m.method==='thread/start'?mode.threadStartDelayMs:0;if(delay)setTimeout(()=>{appendFileSync(${JSON.stringify(lifecycle)},JSON.stringify({pid:process.pid,event:'delayedResponse',method:m.method})+'\\n');send(result);},delay);else send(result);}
if(process.argv.includes('proxy')){
 const {WebSocketServer}=require(${JSON.stringify(ws)}),{connect}=require('node:net');const server=new WebSocketServer({host:'127.0.0.1',port:0});server.on('connection',socket=>socket.on('message',row=>respond(JSON.parse(row),result=>socket.send(JSON.stringify(result)))));server.on('listening',()=>{const pipe=connect(server.address().port,'127.0.0.1');process.stdin.pipe(pipe);pipe.pipe(process.stdout);});
}else{let buffer='';process.stdin.on('data',chunk=>{buffer+=chunk;let end;while((end=buffer.indexOf('\\n'))>=0){const row=buffer.slice(0,end);buffer=buffer.slice(end+1);respond(JSON.parse(row),result=>console.log(JSON.stringify(result)));}});}
`);
  const waitForExit = async () => {
    const deadline = Date.now() + 3000;
    for (;;) {
      const remainingMs = deadline - Date.now();
      if (remainingMs <= 0) throw cleanupUnconfirmed();
      const events = (await readFile(lifecycle, 'utf8')).trim().split('\n').filter(Boolean).map(row => JSON.parse(row));
      const probeRemainingMs = deadline - Date.now();
      if (probeRemainingMs <= 0) throw cleanupUnconfirmed();
      const alive = await Promise.all(events.filter(e => e.event === 'spawn').map(e => {
        if (!Number.isInteger(e.port) || e.port < 1 || e.port > 65535 || typeof e.identity !== 'string' || !e.identity) throw new Error('Invalid synthetic lifecycle identity');
        return fixtureAlive(e.port, e.identity, probeRemainingMs);
      }));
      const running = alive.some(Boolean);
      if (!running) return;
      if (Date.now() >= deadline) throw cleanupUnconfirmed();
      await new Promise(resolve => setTimeout(resolve, 20));
    }
  };
  return {binary,mode,calls,lifecycle,waitForExit};
}
