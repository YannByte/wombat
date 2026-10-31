import assert from 'node:assert/strict';
import {execFileSync} from 'node:child_process';
import {chmodSync, existsSync, mkdirSync, mkdtempSync, readFileSync, rmSync, symlinkSync, writeFileSync} from 'node:fs';
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
const serviceSource=installer.match(/(start_service\(\) \{[\s\S]*?\n\})\nstart_service\n/)?.[1];
assert(serviceSource,'install.sh must expose the tested start_service function');
test('optional resident registration precedes plugin checks and respects defaults and reports failure',{skip:process.platform==='win32'},t=>{
  const root=mkdtempSync(path.join(os.tmpdir(),'wombat-install-service-'));t.after(()=>rmSync(root,{recursive:true,force:true}));
  const destination=path.join(root,'version'),launcher=path.join(root,'wombat'),marker=path.join(root,'calls'),script=path.join(root,'test.sh');
  mkdirSync(path.join(destination,'runtime'),{recursive:true});symlinkSync(process.execPath,path.join(destination,'runtime/node'));
  writeFileSync(launcher,'#!/bin/sh\nprintf "%s\\n" "$*" >> "$MARKER"\nexit "${FAIL_SERVICE:-0}"\n');chmodSync(launcher,0o755);
  writeFileSync(script,'set -eu\n'+serviceSource+'\nstart_service\n');
  assert.match(installer, /^start_service=0$/m);
  assert.match(installer, /--service\) start_service=1/);
  assert.match(windowsInstaller, /\[switch\]\$Service/);
  const env={...process.env,launcher,destination,MARKER:marker,start_service:'1'};
  execFileSync('sh',[script],{env,stdio:'pipe'});assert.equal(readFileSync(marker,'utf8'),'service enable --json\n');
  rmSync(marker);execFileSync('sh',[script],{env:{...env,start_service:'0'},stdio:'pipe'});assert(!existsSync(marker));
  assert.throws(()=>execFileSync('sh',[script],{env:{...env,FAIL_SERVICE:'17'},stdio:'pipe'}));
  assert(installer.indexOf('\nstart_service\n')<installer.indexOf('\ninstall_plugin\n'));
  assert(windowsInstaller.indexOf('if ($Service)')<windowsInstaller.indexOf('if ($Plugin)'));
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
