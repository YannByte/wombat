import assert from 'node:assert/strict';
import {execFileSync} from 'node:child_process';
import {chmodSync, readdirSync, existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, symlinkSync, writeFileSync} from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import test from 'node:test';

const installer = readFileSync(new URL('./install/install.sh', import.meta.url), 'utf8');
const windowsInstaller = readFileSync(new URL('./install/install.ps1', import.meta.url), 'utf8');
const functionMatch = installer.match(/(configure_path\(\) \{[\s\S]*?\n\})\nconfigure_path\n/);
const functionSource = functionMatch?.[1];
assert(functionSource, 'install.sh must expose the tested configure_path function');
test('POSIX installer retries every download error over HTTP/1.1', () => {
  assert.match(installer, /curl -fL --http1\.1 --retry 3 --retry-all-errors --retry-delay 2 --connect-timeout 15/);
  assert.equal(installer.match(/^download "\$base\//gm)?.length, 2);
});

test('installers reject the removed browser option without creating an installation', () => {
  assert.doesNotMatch(installer, /--open\)/);
  assert.doesNotMatch(windowsInstaller, /\[switch\]\$Open/);
});

function exercise(home: string, shell: string, modifyPath: boolean, prefix = path.join(home, '.local')): string {
  const script = `${functionSource}\nconfigure_path\nprintf 'ACTIVE_PATH=%s\\n' "$PATH"\n`;
  const file = path.join(home, 'path-test.sh'); writeFileSync(file, script);
  return execFileSync('sh', [file], {encoding: 'utf8', env: {
    HOME: home, SHELL: shell, PATH: '/usr/bin:/bin', bin_dir: path.join(prefix, 'bin'), prefix,
    modify_path: modifyPath ? '1' : '0',
  }});
}

test('POSIX installer adds the default bin directory once to the active shell profile', {skip: process.platform === 'win32'}, t => {
  const home = mkdtempSync(path.join(os.tmpdir(), 'wombat-install-path-'));
  t.after(() => rmSync(home, {recursive: true, force: true}));
  const output = exercise(home, '/bin/zsh', true); exercise(home, '/bin/zsh', true);
  assert.ok(output.includes(`ACTIVE_PATH=${path.join(home, '.local', 'bin')}:`));
  const profile = readFileSync(path.join(home, '.zshrc'), 'utf8');
  assert.equal(profile.match(/# Wombat PATH/g)?.length, 1);
  assert.match(profile, /export PATH="\$HOME\/\.local\/bin:\$PATH"/);
});

test('POSIX installer respects path opt-out and does not edit profiles for custom prefixes', {skip: process.platform === 'win32'}, t => {
  const home = mkdtempSync(path.join(os.tmpdir(), 'wombat-install-path-'));
  t.after(() => rmSync(home, {recursive: true, force: true}));
  assert.match(exercise(home, '/bin/bash', false), /Add .* to PATH/);
  assert.match(exercise(home, '/bin/bash', true, path.join(home, 'custom')), /Add .* to PATH/);
  assert.throws(() => readFileSync(path.join(home, '.bashrc'), 'utf8'), /ENOENT/);
});

const pluginSource = installer.match(/(install_plugin\(\) \{[\s\S]*?\n\})\ninstall_plugin\n/)?.[1];
assert(pluginSource, 'install.sh must expose the tested install_plugin function');
test('installer rejects the removed service option before changing files',()=>{
 const work=mkdtempSync(path.join(os.tmpdir(),'wombat-service-option-'));
 try {assert.throws(()=>execFileSync('sh',[path.resolve('scripts/install/install.sh'),'--service','--prefix',work],{encoding:'utf8',timeout:5000,stdio:'pipe'}));assert.deepEqual(readdirSync(work),[]);}
 finally {rmSync(work,{recursive:true,force:true});}
});
test('plugin-only reuses a managed custom-prefix runtime without downloading or editing profiles',{skip:process.platform==='win32'},t=>{
  const root=mkdtempSync(path.join(os.tmpdir(),'wombat-plugin-resume-'));t.after(()=>rmSync(root,{recursive:true,force:true}));
  const prefix=path.join(root,'custom & 安装'),installRoot=path.join(prefix,'lib/wombat'),id='1.2.3-'+ 'a'.repeat(12)+'-'+ 'b'.repeat(12),destination=path.join(installRoot,'versions',id);
  mkdirSync(path.join(destination,'runtime'),{recursive:true});symlinkSync(process.execPath,path.join(destination,'runtime/node'));
  mkdirSync(path.join(destination,'lib/skill/.agents/plugins'),{recursive:true});
  mkdirSync(path.join(destination,'lib/skill/plugin'),{recursive:true});
  writeFileSync(path.join(installRoot,'.managed-by-wombat'),'managed GitHub installation');writeFileSync(path.join(installRoot,'current.txt'),id+'\n');
  writeFileSync(path.join(destination,'release.json'),JSON.stringify({format:1,version:'1.2.3',source:'a'.repeat(40),sourceSha256:'b'.repeat(64)}));
  writeFileSync(path.join(destination,'lib/skill/.agents/plugins/marketplace.json'),JSON.stringify({name:'wombat-local',plugins:['wombat'].map(name=>({name,source:{source:'local',path:'./plugin'}}))}));
  writeFileSync(path.join(destination,'lib/skill/plugin/wombat-runtime.json'),JSON.stringify({format:1,version:'1.2.3',skillContentHash:'a'.repeat(64)}));
  const installed=path.join(root,'installed'),native=path.join(root,'codex.mjs');
  writeFileSync(native,`const a=process.argv.slice(2);console.log(JSON.stringify(a[1]==='list'?{installed:[]}:a[2]==='list'?{marketplaces:[]}:{pluginId:a[2],installedPath:process.env.INSTALLED}));`);
  writeFileSync(path.join(destination,'lib/wombat.js'),`const p=process.env.INSTALLED+'/skills/wombat/SKILL.md';console.log(JSON.stringify({outputVersion:1,discovery:{status:'available',instances:[{enabled:true,name:'wombat:wombat',path:p}]},runtimeChecks:[{path:p,status:'compatible',pluginVersion:'1.2.3',skillContentHash:'a'.repeat(64)}]}));`);
  const env={...process.env,HOME:root,WOMBAT_CODEX_BIN:native,INSTALLED:installed};
  const output=execFileSync('sh',[new URL('./install/install.sh',import.meta.url).pathname,'--plugin-only','--prefix',prefix],{env,encoding:'utf8',stdio:['ignore','pipe','pipe']});
  assert.match(output,/Reusing the current/);assert.match(output,/Installed and verified wombat/);assert.equal(existsSync(path.join(root,'.zshrc')),false);
  assert.equal(readFileSync(path.join(installRoot,'current.txt'),'utf8'),id+'\n');
  for(const pointer of ['../outside','..','']){
    writeFileSync(path.join(installRoot,'current.txt'),pointer+'\n');
    assert.throws(()=>execFileSync('sh',[new URL('./install/install.sh',import.meta.url).pathname,'--plugin-only','--prefix',prefix],{env,stdio:'pipe'}));
  }
  assert.match(windowsInstaller,/\[switch\]\$PluginOnly/);
});
