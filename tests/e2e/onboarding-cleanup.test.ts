import assert from 'node:assert/strict';
import {spawn} from 'node:child_process';
import {once} from 'node:events';
import {mkdtemp,mkdir,copyFile,chmod,writeFile,stat,rm} from 'node:fs/promises';
import os from 'node:os';
import path from 'node:path';
import {setTimeout as delay} from 'node:timers/promises';
import {test} from 'node:test';
import {cleanupOnboardingCores} from '../../scripts/onboarding-cleanup.ts';
import {terminateTree} from '../../scripts/verify-e2e-helpers.ts';

test('failed onboarding cleans a real detached core before any Web URL was received',{skip:process.platform==='win32'},async()=>{
  const work=await mkdtemp(path.join(os.tmpdir(),'wombat-onboarding-failure-'));
  const prefix=path.join(work,"安装 '"),data=path.join(work,'data'),home=path.join(work,'source');
  const installed=path.join(prefix,'lib/wombat'),id='0.3.0-synthetic',core=path.join(installed,'versions',id,'lib/wombat-core');
  let service:ReturnType<typeof spawn>|undefined;
  try {
    await mkdir(path.dirname(core),{recursive:true});await mkdir(home);
    await copyFile(path.resolve('dist/wombat-core'),core);await chmod(core,0o755);
    await writeFile(path.join(installed,'current.txt'),id+'\n');
    service=spawn(core,['--serve-usage'],{detached:true,stdio:'ignore',env:{...process.env,CODEX_HOME:home,WOMBAT_DATA_HOME:data,WOMBAT_AUTO_PRICES:'0'}});
    await once(service,'spawn');
    const until=Date.now()+10_000;
    for(;;) {
      assert.equal(service.exitCode,null,'Owned core exited before readiness');
      try{await stat(path.join(data,'live-v2/service-v2.lock'));break;}catch(error){if(!error||typeof error!=='object'||!('code' in error)||error.code!=='ENOENT')throw error;}
      assert.ok(Date.now()<until,'Owned core readiness timed out');await delay(25);
    }
    const closed=once(service,'close');
    await cleanupOnboardingCores(prefix,[data]);await closed;
    assert.ok(service.exitCode!==null||service.signalCode!==null);
  } finally {
    if(service)assert.ok(await terminateTree(service),'Test-owned process must stop');
    await rm(work,{recursive:true,force:true});
  }
});
