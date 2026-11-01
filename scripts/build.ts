import {spawnSync} from 'node:child_process';
import {readFileSync,writeFileSync} from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {checkBuild} from './build-identity.ts';
import {hash,inventory} from './artifact-files.ts';
import {toolCommand} from './run-tool.ts';

const root=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'..');
export function buildRevision(directory:string):string|null {
  const r=spawnSync('git',['rev-parse','HEAD'],{cwd:directory,encoding:'utf8',timeout:10000,maxBuffer:4096});
  return r.status===0?r.stdout.trim():null;
}
export function buildContext(env:NodeJS.ProcessEnv=process.env):string {
  const keys=[...new Set([...Object.keys(env).filter(k=>/^CARGO_(?:BUILD_|PROFILE_|TARGET_)/.test(k)),
    'WOMBAT_CARGO','CARGO_HOME','CARGO_TARGET_DIR','CARGO_ENCODED_RUSTFLAGS','CARGO_ENCODED_RUSTDOCFLAGS','CARGO_INCREMENTAL',
    'RUSTFLAGS','RUSTC','RUSTC_WRAPPER','RUSTC_WORKSPACE_WRAPPER','RUSTDOCFLAGS','RUSTUP_TOOLCHAIN',
    'MACOSX_DEPLOYMENT_TARGET','CC','CFLAGS','CXX','CXXFLAGS'])].sort();
  return hash(JSON.stringify({node:process.version,platform:process.platform,arch:process.arch,
    env:Object.fromEntries(keys.map(k=>[k,env[k]??null]))}));
}
export function moduleOutputs(directory:string):unknown {
  return ['client/dist','cli/dist'].map(p=>({path:p,files:inventory(path.join(directory,p))}));
}
export function reusableBuild(directory:string,context=buildContext()):boolean {
  try {
    checkBuild(directory);
    const cached=JSON.parse(readFileSync(path.join(directory,'dist/modules-build.json'),'utf8'));
    return cached.format===1&&cached.revision===buildRevision(directory)&&cached.context===context&&JSON.stringify(cached.modules)===JSON.stringify(moduleOutputs(directory));
  }catch{return false;}
}
export function build(directory=root):void {
  if(reusableBuild(directory)){console.log('Build unchanged; verified bundled and module outputs reused.');return;}
  const stages:[string,string[]][]=[
    [process.execPath,['scripts/build-core.mjs']],
    ['corepack',['pnpm','--filter','@wombat/client','build']],
    ['corepack',['pnpm','--filter','@wombat/cli','build']],
    [process.execPath,['scripts/build-cli.mjs']],
    [process.execPath,['scripts/build-skill.ts']],
    [process.execPath,['scripts/record-build.ts']],
  ];
  for(const [program,args]of stages){
    const result=spawnSync(...toolCommand(program,args),{cwd:directory,stdio:'inherit',timeout:900000});
    if(result.error||result.status!==0)throw new Error(`Build stopped: ${result.error?.message??`exit ${result.status}`}`);
  }
  writeFileSync(path.join(directory,'dist/modules-build.json'),JSON.stringify({format:1,revision:buildRevision(directory),context:buildContext(),modules:moduleOutputs(directory)})+'\n');
}
if(process.argv[1]&&path.resolve(process.argv[1])===fileURLToPath(import.meta.url)){
  try{build();}catch(error){console.error(error instanceof Error?error.message:error);process.exitCode=1;}
}
