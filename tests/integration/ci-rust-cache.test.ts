import assert from 'node:assert/strict';
import test from 'node:test';
import {mkdtempSync,mkdirSync,readFileSync,writeFileSync,copyFileSync,rmSync,statSync,utimesSync,existsSync} from 'node:fs';
import {spawnSync} from 'node:child_process';
import path from 'node:path';
import os from 'node:os';

test('exact-source cache normalization avoids checkout-driven recompilation while fallback restores stay dirty',()=>{
  const dir=mkdtempSync(path.join(os.tmpdir(),'wombat-rust-cache-'));
  const localCargo=path.join(os.homedir(),'.cargo/bin',process.platform==='win32'?'cargo.exe':'cargo');
  const cargo=process.env.WOMBAT_CARGO??(existsSync(localCargo)?localCargo:'cargo');
  const env={...process.env,CARGO_TARGET_DIR:path.join(dir,'core/target')};
  const run=(program:string,args:string[],extra:NodeJS.ProcessEnv={})=>{
    const r=spawnSync(program,args,{cwd:dir,env:{...env,...extra},encoding:'utf8',timeout:30000,maxBuffer:1024*1024});assert.ifError(r.error);assert.equal(r.status,0,r.stderr+r.stdout);return r;
  };
  try{
    mkdirSync(path.join(dir,'core/src'),{recursive:true});
    writeFileSync(path.join(dir,'core/Cargo.toml'),'[package]\nname="ci-cache-probe"\nversion="0.1.0"\nedition="2021"\n');
    const source=path.join(dir,'core/src/main.rs'),body='fn main() { println!("cache probe"); }\n';writeFileSync(source,body);
    copyFileSync(new URL('../../rust-toolchain.toml',import.meta.url),path.join(dir,'rust-toolchain.toml'));
    const helper=path.join(dir,'restore-times.sh');copyFileSync(new URL('../../.github/actions/rust/restore-times.sh',import.meta.url),helper);
    run('git',['init','--quiet']);run(cargo,['generate-lockfile','--offline','--manifest-path','core/Cargo.toml']);run('git',['add','core/src/main.rs','core/Cargo.toml','core/Cargo.lock']);
    const build=()=>run(cargo,['build','--locked','--offline','--verbose','--manifest-path','core/Cargo.toml']);
    assert.match(build().stderr,/Compiling ci-cache-probe/);
    const newer=new Date(Date.now()+10000);utimesSync(source,newer,newer);
    run('bash',[helper],{CI_RUST_CACHE_HIT:'false'});assert.ok(statSync(source).mtimeMs>Date.now());
    assert.match(build().stderr,/Compiling ci-cache-probe/,'A dependency-only cache must not suppress source validation');
    run('bash',[helper],{CI_RUST_CACHE_HIT:'true'});assert.ok(statSync(source).mtimeMs<1000);assert.equal(readFileSync(source,'utf8'),body);
    const reused=build();assert.doesNotMatch(reused.stderr,/Compiling ci-cache-probe/);assert.match(reused.stderr,/Fresh ci-cache-probe/);
    const binary=path.join(dir,'core/target/debug',process.platform==='win32'?'ci-cache-probe.exe':'ci-cache-probe');
    assert.equal(run(binary,[]).stdout.trim(),'cache probe');
    writeFileSync(source,'fn main() { println!("changed source"); }\n');
    run('bash',[helper],{CI_RUST_CACHE_HIT:'false'});assert.match(build().stderr,/Compiling ci-cache-probe/);assert.equal(run(binary,[]).stdout.trim(),'changed source');
  }finally{rmSync(dir,{recursive:true,force:true});}
});
