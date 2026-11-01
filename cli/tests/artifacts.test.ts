import test from 'node:test';
import assert from 'node:assert/strict';
import path from 'node:path';
import {parseArtifactsArgs,formatArtifactsText} from '../src/artifacts-cli.js';
import type {ArtifactsResult} from '@wombat/client';
test('artifact scope stays explicit and recheck cannot change the original scope',()=>{
  const p=parseArtifactsArgs(['--project','fixture','--path','target','--path','/selected','--keep','dist','--wombat-temp','--json']);
  assert.deepEqual(p.request,{action:'inspect',projectRoot:path.resolve('fixture'),paths:['target','/selected'],keepPaths:['dist'],productTemporary:true,codex:false});
  assert.deepEqual(parseArtifactsArgs(['verify','--read-view','v','--baseline','b','--item','i']).request,{action:'recheck',readView:'v',baselineId:'b',selection:['i']});
  for(const args of [['verify'],['--delete'],['--path'],['--json=false'],['--wombat-temp=1'],['--project','a','--project','b'],['verify','--read-view','v','--baseline','b','--path','target']])assert.throws(()=>parseArtifactsArgs(args));
});
test('human artifact output keeps source text inert and avoids internal identifiers',()=>{
 const result:ArtifactsResult={outputVersion:2,policyVersion:2,action:'inspect',checkedAt:'2026-10-31T00:00:00Z',expiresAt:'2026-10-31T00:10:00Z',scope:{projectRoot:'/synthetic',paths:[],keepPaths:[],productTemporary:false,codex:false},summary:{candidates:0,review:1,protected:0,unknown:0,absent:0,observedLogicalBytes:10,complete:true},items:[{id:'private-item-id',owner:null,kind:'selected_temporary',regeneration:'unknown',regenerationVersion:null,useState:'unknown',path:'a\u001b[31m',assessment:'review',reasons:['task_ownership_required'],observation:{physicalIdentity:'physical',fingerprint:'fingerprint',logicalBytes:10,allocatedBytes:null,files:1,complete:true},recheck:'gone',codexEvidence:null,removal:null}],partial:false,readView:'private-view',baselineId:'private-baseline'};
 const text=formatArtifactsText(result);assert.doesNotMatch(text,/\u001b|private-item|private-view|private-baseline|undefined/);assert.ok(text.includes('10'));
});
test('Codex apply requires original explicit IDs and exposes no path or force override',()=>{
 assert.equal(parseArtifactsArgs(['--apply','--codex']).guided,true);
 const q=parseArtifactsArgs(['--codex','--keep','/selected','--json']);assert.equal(q.request.action,'inspect');if(q.request.action==='inspect')assert.equal(q.request.codex,true);
 assert.deepEqual(parseArtifactsArgs(['--apply','--read-view','v','--baseline','b','--item','i']).request,{action:'apply',readView:'v',baselineId:'b',selection:['i']});
 for(const args of [['--apply'],['--apply','--read-view','v','--baseline','b'],['--codex','--wombat-temp'],['--apply','--read-view','v','--baseline','b','--item','i','--force'],['--apply','--read-view','v','--baseline','b','--path','/arbitrary']])assert.throws(()=>parseArtifactsArgs(args));
});
