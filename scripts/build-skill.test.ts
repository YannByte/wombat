import {test} from 'node:test';import assert from 'node:assert/strict';
import {mkdtempSync,mkdirSync,rmSync,readFileSync,existsSync,writeFileSync} from 'node:fs';import os from 'node:os';import path from 'node:path';import {fileURLToPath} from 'node:url';
import {buildSkill,skillPresentation,skillNames} from './build-skill.ts';import {inventory,hash} from './artifact-files.ts';
test('plugin and standalone resources derive from declared Skill sources with matching inventory and version',()=>{
  const root=path.resolve(path.dirname(fileURLToPath(import.meta.url)),'..'),dir=mkdtempSync(path.join(os.tmpdir(),'wombat-skill-package-'));
  try{
    buildSkill(root,path.join(dir,'skill'));const output=path.join(dir,'skill');
    const metadata=JSON.parse(readFileSync(path.join(output,'manifest.json'),'utf8'));
    assert.deepEqual(metadata.files,inventory(path.join(root,'plugin/skills/wombat')));
    assert.deepEqual(metadata.files,inventory(path.join(output,'wombat')));
    assert.deepEqual(inventory(path.join(output,'wombat')),inventory(path.join(output,'plugin/skills/wombat')));
    const names=skillNames(root);
    assert.equal(names.length,4);
    for(const name of names){
      const source=inventory(path.join(root,'plugin/skills',name));
      assert.deepEqual(inventory(path.join(output,'plugin/skills',name)),source);
      assert.deepEqual(inventory(path.join(output,'collection-plugin/skills',name)),source);
      assert.deepEqual(metadata.pluginSkills.find((s:{name:string})=>s.name===name),{name,contentHash:hash(JSON.stringify(source))});
    }
    assert.equal(metadata.pluginSkillContentHash,hash(JSON.stringify(metadata.pluginSkills)));
    const portable=JSON.parse(readFileSync(path.join(output,'plugin/plugin.json'),'utf8')),compat=JSON.parse(readFileSync(path.join(output,'plugin/.codex-plugin/plugin.json'),'utf8'));
    const entry=JSON.parse(readFileSync(path.join(root,'plugin/skills/wombat/agents/openai.yaml'),'utf8')).interface;
    assert.equal(compat.interface.shortDescription,entry.short_description);
    assert.equal(compat.interface.defaultPrompt[0],entry.default_prompt.replace('$wombat','$wombat:wombat'));
    assert.equal(compat.interface.defaultPrompt.length,names.length);
    for(const [i,name] of names.entries()){
      const prompt=JSON.parse(readFileSync(path.join(root,'plugin/skills',name,'agents/openai.yaml'),'utf8')).interface.default_prompt;
      assert.equal(compat.interface.defaultPrompt[i],prompt.replace('$'+name,'$wombat:'+name));
    }
    assert.deepEqual(portable.extensions['com.openai'].interface,compat.interface);
    assert.equal(portable.version,JSON.parse(readFileSync(path.join(root,'package.json'),'utf8')).version);assert.equal(compat.version,portable.version);
    const catalog=JSON.parse(readFileSync(path.join(output,'.agents/plugins/marketplace.json'),'utf8'));
    assert.equal(path.resolve(output,catalog.plugins[0].source.path),path.join(output,'plugin'));
    assert.ok(!JSON.stringify(portable).includes(root));
    assert.equal(portable.extensions['com.openai'].hooks,undefined);
    for(const directory of ['hooks','scripts'])assert.equal(existsSync(path.join(output,'plugin',directory)),false);
    const collected=path.join(output,'collection-plugin');
    const collectionManifest=JSON.parse(readFileSync(path.join(collected,'.codex-plugin/plugin.json'),'utf8'));
    assert.equal(collectionManifest.name,'wombat-collection');
    assert.equal(collectionManifest.hooks,'./hooks/hooks.json');
    assert.equal(collectionManifest.interface.defaultPrompt[0],entry.default_prompt.replace('$wombat','$wombat-collection:wombat'));
    assert.equal(existsSync(path.join(collected,'plugin.json')),false);
    assert.deepEqual(inventory(path.join(collected,'skills/wombat')),metadata.files);
    for(const directory of ['hooks','scripts'])assert.deepEqual(inventory(path.join(collected,directory)),inventory(path.join(root,'plugin',directory)));
    const hooks=JSON.parse(readFileSync(path.join(collected,'hooks/hooks.json'),'utf8'));
    assert.equal(Object.keys(hooks.hooks).length,10);
    for(const definitions of Object.values(hooks.hooks))for(const definition of definitions as {hooks:{async:boolean;timeout:number;command:string}[]}[]) {
      assert.equal(definition.hooks[0].async,true);assert.equal(definition.hooks[0].timeout,5);
      assert.equal(definition.hooks[0].command,'sh "${PLUGIN_ROOT}/scripts/collect.sh"');
    }
    assert.equal(catalog.plugins[1].source.path,'./collection-plugin');
    const runtime=JSON.parse(readFileSync(path.join(output,'plugin/wombat-runtime.json'),'utf8'));assert.deepEqual(runtime.requiredCapabilities,metadata.requiredCapabilities);assert.equal(runtime.skillContentHash,metadata.pluginSkillContentHash);assert.deepEqual(runtime.skills,metadata.pluginSkills);assert.equal(runtime.version,portable.version);
  }finally{rmSync(dir,{recursive:true,force:true});}
});

test('shipped Skill catalog rejects missing, duplicate and unsafe names',()=>{
  const root=mkdtempSync(path.join(os.tmpdir(),'wombat-skill-catalog-'));
  try {
    mkdirSync(path.join(root,'plugin'));
    const check=(skills:unknown)=>{writeFileSync(path.join(root,'plugin/package.json'),JSON.stringify({skills}));return skillNames(root);};
    assert.deepEqual(check(['wombat','wombat-task-retro']),['wombat','wombat-task-retro']);
    for(const invalid of [undefined,{},[],['wombat','wombat'],['other','wombat'],['wombat','../other'],['wombat','UPPER'],['wombat',42],['wombat','x'.repeat(65)],Array.from({length:33},(_,i)=>i?'skill-'+i:'wombat')])assert.throws(()=>check(invalid));
  }finally{rmSync(root,{recursive:true,force:true});}
});

test('Skill presentation follows source changes and rejects unusable invocation metadata',()=>{
  const dir=mkdtempSync(path.join(os.tmpdir(),'wombat-skill-interface-')),file=path.join(dir,'openai.yaml');
  const view={display_name:'Synthetic',short_description:'Understand selected synthetic usage',default_prompt:'Use $wombat to answer the selected question.'};
  const read=(change:Record<string,unknown>={})=>{writeFileSync(file,JSON.stringify({interface:{...view,...change}}));return skillPresentation(file,'$wombat:wombat');};
  try {
    assert.equal(read().defaultPrompt[0],'Use $wombat:wombat to answer the selected question.');
    assert.equal(read({default_prompt:'Use $wombat to inspect timing.'}).defaultPrompt[0],'Use $wombat:wombat to inspect timing.');
    assert.equal(skillPresentation(file,'$wombat-collection:wombat').defaultPrompt[0],'Use $wombat-collection:wombat to inspect timing.');
    assert.equal(read({default_prompt:'Compare $wombat-collection:wombat, then use $wombat.'}).defaultPrompt[0],'Compare $wombat-collection:wombat, then use $wombat:wombat.');
    for(const change of [{display_name:''},{short_description:'short'},{short_description:'x'.repeat(65)},{default_prompt:'Inspect usage.'},{default_prompt:'Use $wombat:wombat.'},{default_prompt:'Use $wombat and $wombat.'},{default_prompt:5}])assert.throws(()=>read(change));
    writeFileSync(file,JSON.stringify({interface:null}));assert.throws(()=>skillPresentation(file,'$wombat:wombat'));
    writeFileSync(file,'{broken');assert.throws(()=>skillPresentation(file,'$wombat:wombat'));
    writeFileSync(file,JSON.stringify({interface:{...view,default_prompt:'Use $wombat-task-retro to inspect the selected task.'}}));
    assert.equal(skillPresentation(file,'$wombat:wombat-task-retro').defaultPrompt[0],'Use $wombat:wombat-task-retro to inspect the selected task.');
    assert.throws(()=>skillPresentation(file,'$wombat:wombat-daily-review'));
    assert.throws(()=>skillPresentation(file,'$wombat:../../other'));
  }finally{rmSync(dir,{recursive:true,force:true});}
});
