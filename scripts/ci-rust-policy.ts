import {load} from 'js-yaml';

type Mapping=Record<string,unknown>;
const mapping=(value:unknown):value is Mapping=>typeof value==='object'&&value!==null&&!Array.isArray(value);
function steps(document:unknown):Mapping[]{
  if(!mapping(document))return [];
  const lists:unknown[]=[];
  if(mapping(document.runs))lists.push(document.runs.steps);
  if(mapping(document.jobs))for(const job of Object.values(document.jobs))if(mapping(job))lists.push(job.steps);
  return lists.flatMap(list=>Array.isArray(list)?list.filter(mapping):[]);
}
export function rustWorkspacePaths(source:string):string[]{
  const workspace=steps(load(source)).find(step=>step.id==='workspace');
  if(!workspace||!mapping(workspace.with)||typeof workspace.with.path!=='string')throw new Error('Missing literal workspace cache paths');
  return workspace.with.path.trim().split(/\r?\n/).map(p=>p.trim());
}
/** Swatinem selects shared-key instead of key; its hit alone cannot prove source identity. */
export function rustCachePolicyErrors(sources:Record<string,string>):string[]{
  const errors:string[]=[];
  for(const [file,source]of Object.entries(sources)){
    let list:Mapping[];try{list=steps(load(source));}catch{errors.push(`${file}: invalid Action YAML`);continue;}
    for(const step of list)if(typeof step.uses==='string'&&step.uses.startsWith('Swatinem/rust-cache@')&&mapping(step.with)){
      if(step.with['shared-key']&&step.with.key)errors.push(`${file}: shared-key overrides key; do not combine them`);
    }
    if(file.replaceAll('\\','/')!=='.github/actions/rust/action.yml')continue;
    const dependency=list.findIndex(s=>s.id==='dependencies'),workspace=list.findIndex(s=>s.id==='workspace');
    const dep=list[dependency],work=list[workspace];
    if(!dep||!mapping(dep.with)||dep.with['cache-workspace-crates']!==false)errors.push(`${file}: dependency cache must exclude workspace crates`);
    if(workspace<=dependency||!work||typeof work.uses!=='string'||!work.uses.startsWith('actions/cache@')||!mapping(work.with)){
      errors.push(`${file}: restore the official workspace cache after dependencies`);continue;
    }
    const key=work.with.key;
    const inputs=['runner.os','runner.arch','inputs.key','steps.environment.outputs.key','core/src/**','core/prices/**','core/tests/**','core/build.rs','core/Cargo.toml','core/Cargo.lock','rust-toolchain.toml','scripts/build-core.mjs','.cargo/config*','core/.cargo/config*','.github/actions/rust/**','.github/workflows/ci.yml'];
    if(typeof key!=='string'||!key.includes('hashFiles(')||inputs.some(input=>!key.includes(input)))errors.push(`${file}: workspace cache must bind source, manifests, toolchain and compiler environment`);
    if(work.with['restore-keys'])errors.push(`${file}: workspace outputs require an exact key without fallback restores`);
    const reuse=list.find(s=>mapping(s.env)&&s.env.CI_RUST_CACHE_HIT!==undefined);
    if(!reuse||!mapping(reuse.env)||typeof reuse.env.CI_RUST_CACHE_HIT!=='string'||!/^\$\{\{\s*steps\.workspace\.outputs\.cache-hit\s*\}\}$/.test(reuse.env.CI_RUST_CACHE_HIT))errors.push(`${file}: timestamp reuse must use the exact workspace hit`);
    try{if(rustWorkspacePaths(source).some(p=>!p.startsWith('core/target/')||!/(?:wombat-core|wombat_core)/.test(p)))errors.push(`${file}: cache only current-project compiler outputs`);}catch(error){errors.push(`${file}: ${String(error)}`);}
  }
  return errors;
}
