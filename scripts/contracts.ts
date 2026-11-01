import {spawnSync} from 'node:child_process';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {checkBuild} from './build-identity.ts';

const root=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'..');
const args=process.argv.slice(2);
if(args.some(a=>a!=='--check'))throw new Error('Only --check is supported');
let binary=path.join(root,'dist',process.platform==='win32'?'wombat-core.exe':'wombat-core');
try{checkBuild(root);}catch{
  const result=spawnSync(process.execPath,['scripts/build-core.mjs','--check'],{cwd:root,stdio:'inherit',timeout:900000});
  if(result.error||result.status!==0)throw result.error??new Error(`Contract build failed: ${result.status}`);
  binary=path.join(root,'core/target/debug',process.platform==='win32'?'wombat-core.exe':'wombat-core');
}
const result=spawnSync(process.execPath,['scripts/generate-usage-contracts.mjs',...args],{cwd:root,env:{...process.env,WOMBAT_SCHEMA_CORE:binary},stdio:'inherit',timeout:120000});
if(result.error||result.status!==0)throw result.error??new Error(`Contract verification failed: ${result.status}`);
