import assert from 'node:assert/strict';
import test from 'node:test';
import {readFileSync} from 'node:fs';
import {load,dump} from 'js-yaml';
import {rustCachePolicyErrors,rustWorkspacePaths} from './ci-rust-policy.ts';

const file='.github/actions/rust/action.yml';
const source=readFileSync(new URL('../'+file,import.meta.url),'utf8');
type Fixture={runs:{steps:Array<{id?:string;uses?:string;with?:Record<string,unknown>;env?:Record<string,unknown>}>}};
test('Rust cache policy rejects ignored source keys, unbound reuse and cleanup-order regressions',()=>{
  assert.deepEqual(rustCachePolicyErrors({[file]:source}),[]);
  assert.deepEqual(rustCachePolicyErrors({[file.replaceAll('/','\\')]:source}),[]);
  assert.ok(rustWorkspacePaths(source).some(p=>p.includes('/deps/wombat_core')));
  for(const mutate of [
    (d:Fixture)=>{d.runs.steps.find(s=>s.id==='dependencies')!.with!.key='ignored-source';},
    (d:Fixture)=>{d.runs.steps.find(s=>s.id==='dependencies')!.with!['cache-workspace-crates']=true;},
    (d:Fixture)=>{d.runs.steps.find(s=>s.id==='workspace')!.with!.key='unbound';},
    (d:Fixture)=>{d.runs.steps.find(s=>s.id==='workspace')!.with!['restore-keys']='rust-workspace-v1-';},
    (d:Fixture)=>{const w=d.runs.steps.find(s=>s.id==='workspace')!;w.with!.key=String(w.with!.key).replace("'core/Cargo.toml', ",'');},
    (d:Fixture)=>{d.runs.steps.find(s=>s.env?.CI_RUST_CACHE_HIT)!.env!.CI_RUST_CACHE_HIT='${{ steps.dependencies.outputs.cache-hit }}';},
    (d:Fixture)=>{const w=d.runs.steps.findIndex(s=>s.id==='workspace');d.runs.steps.unshift(d.runs.steps.splice(w,1)[0]);},
    (d:Fixture)=>{d.runs.steps.find(s=>s.id==='workspace')!.with!.path='core/src/**';},
  ]){
    // This controlled fixture is the same parsed metadata used by the real source check.
    const document=load(source) as Fixture;mutate(document);
    assert.ok(rustCachePolicyErrors({[file]:dump(document)}).length);
    assert.ok(rustCachePolicyErrors({[file.replaceAll('/','\\')]:dump(document)}).length);
  }
});
