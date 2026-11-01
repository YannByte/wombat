import assert from 'node:assert/strict';
import test from 'node:test';
import {mkdtempSync,mkdirSync,readFileSync,writeFileSync,rmSync} from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import {spawnSync} from 'node:child_process';
import {buildContext,buildRevision,moduleOutputs,reusableBuild} from './build.ts';
import {builtFiles,sourceIdentity} from './build-identity.ts';
test('unchanged builds reuse complete outputs; changed inputs, environment and missing modules rebuild',()=>{
  const root=mkdtempSync(path.join(os.tmpdir(),'wombat-build-reuse-'));
  try{
    for(const p of ['dist','client/dist','cli/dist','core/src'])mkdirSync(path.join(root,p),{recursive:true});
    writeFileSync(path.join(root,'.gitignore'),'dist/\n');
    const git=spawnSync('git',['init','--quiet'],{cwd:root,encoding:'utf8',timeout:10000});assert.equal(git.status,0,git.stderr);
    writeFileSync(path.join(root,'core/src/lib.rs'),'// Synthetic source\n');
    for(const file of ['dist/wombat.js','client/dist/index.js','cli/dist/index.js'])writeFileSync(path.join(root,file),'// Synthetic output\n');
    const receipt=()=>{writeFileSync(path.join(root,'dist/build.json'),JSON.stringify({format:1,target:process.platform+'-'+process.arch,sourceSha256:sourceIdentity(root),files:builtFiles(root)}));writeFileSync(path.join(root,'dist/modules-build.json'),JSON.stringify({format:1,revision:buildRevision(root),context:buildContext(),modules:moduleOutputs(root)}));};
    receipt();assert.equal(reusableBuild(root),true);assert.equal(reusableBuild(root,'different environment'),false);
    const file=path.join(root,'dist/modules-build.json');const changed=JSON.parse(readFileSync(file,'utf8'));
    changed.revision='different revision';writeFileSync(file,JSON.stringify(changed));assert.equal(reusableBuild(root),false);receipt();
    writeFileSync(path.join(root,'core/src/lib.rs'),'// Changed source\n');assert.equal(reusableBuild(root),false);
    receipt();writeFileSync(path.join(root,'client/dist/index.js'),'// Changed module\n');assert.equal(reusableBuild(root),false);
    receipt();rmSync(path.join(root,'cli/dist/index.js'));assert.equal(reusableBuild(root),false);
    assert.notEqual(buildContext({RUSTFLAGS:'-C opt-level=1'}),buildContext({}));
    assert.notEqual(buildContext({CARGO_PROFILE_RELEASE_LTO:'false'}),buildContext({}));
  }finally{rmSync(root,{recursive:true,force:true});}
});
