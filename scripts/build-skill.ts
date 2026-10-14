import {mkdirSync, readFileSync, writeFileSync, cpSync, rmSync, mkdtempSync, renameSync} from 'node:fs';
import path from 'node:path';
import {fileURLToPath} from 'node:url';
import {execFileSync} from 'node:child_process';
import {inventory, hash} from './artifact-files.ts';

/** JSON-form YAML keeps standalone and plugin presentation on one native JSON reader. */
export function skillPresentation(file:string,reference:'$wombat:wombat'|'$wombat-collection:wombat') {
  const source:unknown=JSON.parse(readFileSync(file,'utf8'));
  const object=(value:unknown):value is Record<string,unknown>=>value!==null&&typeof value==='object'&&!Array.isArray(value);
  const view=object(source)?source.interface:null;
  if(!object(view)||typeof view.display_name!=='string'||!view.display_name.trim()||typeof view.short_description!=='string'||view.short_description.length<25||view.short_description.length>64||typeof view.default_prompt!=='string'||!view.default_prompt.trim()||view.default_prompt.match(/\$wombat(?![\w:-])/g)?.length!==1)
    throw new Error('Skill interface requires a display name, 25–64 character description and one $wombat reference');
  return {displayName:view.display_name,shortDescription:view.short_description,defaultPrompt:[view.default_prompt.replace(/\$wombat(?![\w:-])/,reference)]};
}

export function buildSkill(root:string, output=path.join(root,'dist','skill')) {
  const metadata=JSON.parse(readFileSync(path.join(root,'plugin/package.json'),'utf8'));
  const version=JSON.parse(readFileSync(path.join(root,'package.json'),'utf8')).version;
  const files=inventory(path.join(root,'plugin/skills/wombat'));
  if(!files.some(f=>f.path==='SKILL.md'))throw new Error('Missing Wombat Skill entry');
  mkdirSync(path.dirname(output),{recursive:true});
  const stage=mkdtempSync(path.join(path.dirname(output),'.skill-build-'));
  try{
    cpSync(path.join(root,'plugin/skills/wombat'),path.join(stage,'wombat'),{recursive:true});
    const source=execFileSync('git',['rev-parse','HEAD'],{cwd:root,encoding:'utf8',timeout:5000,maxBuffer:4096}).trim();
    writeFileSync(path.join(stage,'manifest.json'),JSON.stringify({format:1,name:'wombat',version,source,requiredCapabilities:metadata.requiredCapabilities,files,contentHash:hash(JSON.stringify(files))},null,2)+'\n');
    const plugin=path.join(stage,'plugin');mkdirSync(path.join(plugin,'.codex-plugin'),{recursive:true});
    cpSync(path.join(stage,'wombat'),path.join(plugin,'skills/wombat'),{recursive:true});
    cpSync(path.join(root,'LICENSE'),path.join(plugin,'LICENSE'));
    const identity={name:'wombat',version,description:metadata.description,license:metadata.license,homepage:metadata.homepage};
    writeFileSync(path.join(plugin,'wombat-runtime.json'),JSON.stringify({format:1,version,requiredCapabilities:metadata.requiredCapabilities,skillContentHash:hash(JSON.stringify(files))},null,2)+'\n');
    const presentation=skillPresentation(path.join(root,'plugin/skills/wombat/agents/openai.yaml'),'$wombat:wombat');
    writeFileSync(path.join(plugin,'plugin.json'),JSON.stringify({$schema:'https://agent-plugins.org/schemas/1.0.0/plugin.schema.json',...identity,extensions:{'com.openai':{interface:presentation}}},null,2)+'\n');
    writeFileSync(path.join(plugin,'.codex-plugin/plugin.json'),JSON.stringify({...identity,skills:'./skills/',interface:presentation},null,2)+'\n');
    const collection=path.join(stage,'collection-plugin');
    cpSync(plugin,collection,{recursive:true});
    for(const directory of ['hooks','scripts'])cpSync(path.join(root,'plugin',directory),path.join(collection,directory),{recursive:true});
    const collectionIdentity={...identity,name:'wombat-collection'};
    const collectionPresentation=skillPresentation(path.join(root,'plugin/skills/wombat/agents/openai.yaml'),'$wombat-collection:wombat');
    const collectionInterface={...collectionPresentation,displayName:collectionPresentation.displayName+' collection'};
    // Codex 0.160.1 skips lifecycle Hooks when a root manifest is present.
    // Keep this local-only package on its native compatibility manifest.
    rmSync(path.join(collection,'plugin.json'));
    writeFileSync(path.join(collection,'.codex-plugin/plugin.json'),JSON.stringify({...collectionIdentity,skills:'./skills/',hooks:'./hooks/hooks.json',interface:collectionInterface},null,2)+'\n');
    mkdirSync(path.join(stage,'.agents/plugins'),{recursive:true});
    writeFileSync(path.join(stage,'.agents/plugins/marketplace.json'),JSON.stringify({name:'wombat-local',interface:{displayName:'Wombat Local'},plugins:[{name:'wombat',source:{source:'local',path:'./plugin'},policy:{installation:'AVAILABLE',authentication:'ON_INSTALL'}},{name:'wombat-collection',source:{source:'local',path:'./collection-plugin'},policy:{installation:'AVAILABLE',authentication:'ON_INSTALL'}}]},null,2)+'\n');
    rmSync(output,{recursive:true,force:true});renameSync(stage,output);
  }finally{rmSync(stage,{recursive:true,force:true});}
}
if(process.argv[1]&&path.resolve(process.argv[1])===fileURLToPath(import.meta.url))buildSkill(path.resolve(path.dirname(fileURLToPath(import.meta.url)),'..'));
