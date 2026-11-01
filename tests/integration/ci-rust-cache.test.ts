import assert from 'node:assert/strict';
import test from 'node:test';
import {mkdtempSync,mkdirSync,readFileSync,writeFileSync,copyFileSync,cpSync,globSync,rmSync,statSync,utimesSync,existsSync} from 'node:fs';
import {spawnSync} from 'node:child_process';
import path from 'node:path';
import os from 'node:os';
import {rustWorkspacePaths} from '../../scripts/ci-rust-policy.js';

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
    writeFileSync(path.join(dir,'core/Cargo.toml'),'[package]\nname="wombat-core"\nversion="0.1.0"\nedition="2021"\n');
    const source=path.join(dir,'core/src/main.rs'),body='fn main() { println!("cache probe"); }\n';writeFileSync(source,body);
    writeFileSync(path.join(dir,'core/src/lib.rs'),'pub fn marker() -> u8 { 1 }\n');
    copyFileSync(new URL('../../rust-toolchain.toml',import.meta.url),path.join(dir,'rust-toolchain.toml'));
    const helper=path.join(dir,'restore-times.sh');copyFileSync(new URL('../../.github/actions/rust/restore-times.sh',import.meta.url),helper);
    run('git',['init','--quiet']);run(cargo,['generate-lockfile','--offline','--manifest-path','core/Cargo.toml']);run('git',['add','core/src','core/Cargo.toml','core/Cargo.lock']);
    const fingerprint=path.join(dir,'fingerprint.sh'),output=path.join(dir,'fingerprint-output');
    copyFileSync(new URL('../../.github/actions/rust/fingerprint.sh',import.meta.url),fingerprint);
    const context=(extra:NodeJS.ProcessEnv={})=>{
      writeFileSync(output,'');run('bash',[fingerprint],{ImageVersion:'synthetic-1',GITHUB_OUTPUT:output,PATH:path.dirname(cargo)+path.delimiter+process.env.PATH,...extra});
      const value=readFileSync(output,'utf8');assert.match(value,/^key=[0-9a-f]{40}\n$/);return value;
    };
    const baseline=context();assert.equal(context(),baseline);
    assert.notEqual(context({ImageVersion:'synthetic-2'}),baseline);
    assert.notEqual(context({RUSTFLAGS:'--cfg synthetic_cache_probe'}),baseline);
    assert.notEqual(context({CARGO_PROFILE_RELEASE_LTO:'false'}),baseline);
    const missingCompiler=spawnSync('bash',[fingerprint],{cwd:dir,env:{...env,ImageVersion:'synthetic-1',GITHUB_OUTPUT:output,RUSTC:path.join(dir,'missing-compiler')},encoding:'utf8',timeout:5000,maxBuffer:65536});
    assert.notEqual(missingCompiler.status,0,'Unknown compiler identity must fail before cache reuse');
    const build=()=>run(cargo,['build','--locked','--offline','--verbose','--manifest-path','core/Cargo.toml']);
    assert.match(build().stderr,/Compiling wombat-core/);
    const newer=new Date(Date.now()+10000);utimesSync(source,newer,newer);
    run('bash',[helper],{CI_RUST_CACHE_HIT:'false'});assert.ok(statSync(source).mtimeMs>Date.now());
    assert.match(build().stderr,/Compiling wombat-core/,'A dependency-only cache must not suppress source validation');
    run('bash',[helper],{CI_RUST_CACHE_HIT:'true'});assert.ok(statSync(source).mtimeMs<1000);assert.equal(readFileSync(source,'utf8'),body);
    const reused=build();assert.doesNotMatch(reused.stderr,/Compiling wombat-core/);assert.match(reused.stderr,/Fresh wombat-core/);
    const binary=path.join(dir,'core/target/debug',process.platform==='win32'?'wombat-core.exe':'wombat-core');
    assert.equal(run(binary,[]).stdout.trim(),'cache probe');
    const release=()=>run(cargo,['build','--release','--locked','--offline','--verbose','--manifest-path','core/Cargo.toml']);
    assert.match(release().stderr,/Compiling wombat-core/);
    // Exercise the actual configured cache paths, including library metadata and
    // Windows' unhashed executable, rather than merely testing timestamp changes.
    const paths=globSync(rustWorkspacePaths(readFileSync(new URL('../../.github/actions/rust/action.yml',import.meta.url),'utf8')),{cwd:dir});
    assert.ok(paths.length);const saved=path.join(dir,'saved-workspace');
    for(const p of paths){mkdirSync(path.dirname(path.join(saved,p)),{recursive:true});cpSync(path.join(dir,p),path.join(saved,p),{recursive:true,preserveTimestamps:true});}
    rmSync(path.join(dir,'core/target'),{recursive:true,force:true});
    for(const p of paths){mkdirSync(path.dirname(path.join(dir,p)),{recursive:true});cpSync(path.join(saved,p),path.join(dir,p),{recursive:true,preserveTimestamps:true});}
    run('bash',[helper],{CI_RUST_CACHE_HIT:'true'});
    for(const result of [build(),release()]){
      assert.match(result.stderr,/Fresh wombat-core/);assert.doesNotMatch(result.stderr,/Compiling wombat-core/,'The real cache paths omitted a required compiler output');
    }
    writeFileSync(source,'fn main() { println!("changed source"); }\n');
    run('bash',[helper],{CI_RUST_CACHE_HIT:'false'});assert.match(build().stderr,/Compiling wombat-core/);assert.equal(run(binary,[]).stdout.trim(),'changed source');
  }finally{rmSync(dir,{recursive:true,force:true});}
});
