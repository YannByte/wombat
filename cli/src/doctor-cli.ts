import {execFile} from 'node:child_process';
import {promisify} from 'node:util';
import {parseOptions,withCancellation} from './human-input.js';
import {writeHelp} from './help.js';
import {setupExitCode} from './exit-codes.js';
import {createNodeClient} from '@wombat/client/node';
import type {SetupRequest,SetupResult} from '@wombat/client';
import {spawnSync} from 'node:child_process';
import {accessSync, constants, existsSync, readFileSync, statSync} from 'node:fs';
import os from 'node:os';
import path from 'node:path';
import {CoreError} from '@wombat/client';
import {resolveCoreBinary} from '@wombat/client/node';
import {t} from '@wombat/client/locale';
import packageMetadata from '../package.json' with {type: 'json'};

export type DoctorStatus = 'pass' | 'warning' | 'fail';
export interface DoctorCheck {id: string; status: DoctorStatus; message: string; repair?: string}
export interface DoctorResult {outputVersion: 1; action: 'doctor'; status: DoctorStatus; version: string; checks: DoctorCheck[]; scannedSources: false; networkUsed: false}

function readableDirectory(directory: string): boolean {
  try {
    accessSync(directory, constants.R_OK | constants.X_OK);
    return statSync(directory).isDirectory();
  } catch {
    return false;
  }
}
function commandOnPath(name: string, envPath: string): string | undefined {
  const names = process.platform === 'win32' ? [`${name}.cmd`, `${name}.exe`, name] : [name];
  for (const directory of envPath.split(path.delimiter).filter(Boolean)) for (const candidate of names) {
    const file = path.join(directory, candidate);
    try {
      if (!statSync(file).isFile()) continue;
      if (process.platform !== 'win32') accessSync(file, constants.X_OK);
      return path.resolve(file);
    } catch {
      // Keep searching when a matching file cannot be executed.
    }
  }
  return undefined;
}
function resultStatus(checks: DoctorCheck[]): DoctorStatus {
  return checks.some(check => check.status === 'fail') ? 'fail' : checks.some(check => check.status === 'warning') ? 'warning' : 'pass';
}

export function parseDoctorArgs(argv: string[]) {
  const p=parseOptions(argv,['--project','--root'],['--hooks'],['--root']);
  if(p.positionals.length)throw new CoreError('INVALID_ARGUMENT',t('cli.doctor.invalid',{value:p.positionals[0]}));
  const request:SetupRequest={project:path.resolve(p.get('--project')??process.cwd()),includeHooks:p.flags.has('--hooks'),...(p.all('--root').length?{roots:p.all('--root').map(r=>path.resolve(r))}:{})};
  return {request,json:p.json,help:p.help};
}

export function collectDoctor(options: {entryFile?: string; env?: NodeJS.ProcessEnv; resolveCore?: () => string; runCore?: (file: string) => {status: number | null; stdout: string; stderr: string}} = {}): DoctorResult {
  const env = options.env ?? process.env, checks: DoctorCheck[] = [];
  const entryFile = path.resolve(options.entryFile ?? process.argv[1] ?? '');
  const payload = path.dirname(path.dirname(entryFile));
  const releaseFile = path.join(payload, 'release.json');
  let managedRoot: string | undefined;
  if (existsSync(releaseFile)) {
    try {
      const release = JSON.parse(readFileSync(releaseFile, 'utf8')) as {version?: unknown};
      const versions = path.dirname(payload), root = path.dirname(versions), pointer = path.join(root, 'current.txt');
      if (path.basename(versions) === 'versions' && existsSync(pointer) && readFileSync(pointer, 'utf8').trim() === path.basename(payload)
        && release.version === packageMetadata.version) {
        managedRoot = root; checks.push({id: 'installation', status: 'pass', message: t('cli.doctor.managed')});
      } else checks.push({id: 'installation', status: 'warning', message: t('cli.doctor.unmanaged'), repair: t('cli.doctor.reinstall')});
    } catch { checks.push({id: 'installation', status: 'fail', message: t('cli.doctor.invalidRelease'), repair: t('cli.doctor.reinstall')}); }
  } else checks.push({id: 'installation', status: 'warning', message: t('cli.doctor.source'), repair: t('cli.doctor.reinstall')});

  const located = commandOnPath('wombat', env.PATH ?? '');
  if (!managedRoot) checks.push({id: 'path', status: located ? 'pass' : 'warning', message: located ? t('cli.doctor.pathReady') : t('cli.doctor.pathMissing'), ...(located ? {} : {repair: t('cli.doctor.pathRepair')})});
  else {
    const prefix = path.dirname(path.dirname(managedRoot)), expected = path.join(prefix, 'bin', process.platform === 'win32' ? 'wombat.cmd' : 'wombat');
    const match = Boolean(located) && path.resolve(located!).toLowerCase() === path.resolve(expected).toLowerCase();
    checks.push({id: 'path', status: match ? 'pass' : 'fail', message: match ? t('cli.doctor.pathReady') : t('cli.doctor.pathWrong'), ...(match ? {} : {repair: t('cli.doctor.pathRepair')})});
  }

  checks.push({id: 'runtime', status: 'pass', message: t('cli.doctor.runtime', {version: process.versions.node})});
  try {
    const core = (options.resolveCore ?? resolveCoreBinary)();
    const run = options.runCore ?? (file => {
      const script = /\.[cm]js$/.test(file);
      const result = spawnSync(script ? process.execPath : file, script ? [file, '--version'] : ['--version'], {encoding: 'utf8', timeout: 15_000, maxBuffer: 64 * 1024, windowsHide: true});
      if (result.error) throw result.error;
      return {status: result.status, stdout: result.stdout, stderr: result.stderr};
    });
    const result = run(core), expected = `wombat-core ${packageMetadata.version}`;
    if (result.status !== 0 || result.stdout.trim() !== expected) throw new Error(result.stderr || result.stdout || t('cli.doctor.coreFailed'));
    checks.push({id: 'core', status: 'pass', message: t('cli.doctor.coreReady', {version: packageMetadata.version})});
  } catch (error) { checks.push({id: 'core', status: 'fail', message: error instanceof Error ? error.message : String(error), repair: t('cli.doctor.reinstall')}); }

  const codexHome = path.resolve(env.CODEX_HOME ?? path.join(os.homedir(), '.codex'));
  checks.push(readableDirectory(codexHome)
    ? {id: 'codex-source', status: 'pass', message: t('cli.doctor.codexReady')}
    : {id: 'codex-source', status: 'warning', message: t('cli.doctor.codexMissing'), repair: t('cli.doctor.codexRepair')});
  return {outputVersion: 1, action: 'doctor', status: resultStatus(checks), version: packageMetadata.version, checks, scannedSources: false, networkUsed: false};
}

export async function runDoctorCli(argv:string[]):Promise<number>{
  const {request,json,help}=parseDoctorArgs(argv);
  if(help){writeHelp('system',['doctor',...argv]);return 0;}
  return withCancellation(async signal=>{
    let probe:{status:number|null;stdout:string;stderr:string}|undefined, failure:unknown;
    try{
      const file=resolveCoreBinary(),script=/\.[cm]js$/.test(file);
      const result=await promisify(execFile)(script?process.execPath:file,script?[file,'--version']:['--version'],{encoding:'utf8',timeout:15000,maxBuffer:65536,windowsHide:true,signal});
      probe={status:0,stdout:result.stdout,stderr:result.stderr};
    }catch(error){failure=error;}
    if(signal.aborted)throw new CoreError('CANCELLED',t('common.cancelled'));
    const local=collectDoctor({runCore:()=>{if(failure)throw failure;return probe!;}});
    const result:SetupResult={...await createNodeClient({automaticPrices:false}).setup!(request,{signal}),installationChecks:local.checks};
    if(json)process.stdout.write(JSON.stringify(result)+'\n');
    else{
      for(const check of local.checks)process.stdout.write(`${t(`cli.doctor.${check.status}`)}\t${check.id}\t${check.message}${check.repair?'\n  '+check.repair:''}\n`);
      process.stdout.write(t('setup.summary',{version:result.nativeVersion??'—',skill:t(`skill.${result.discovery.status==='selection_changed'?'unavailable':result.discovery.status}`)})+'\n');
      for(const check of result.runtimeChecks??[])process.stdout.write(t('setup.runtimeCheck',{version:check.pluginVersion??'—',status:t(`setup.runtime.${check.status}`)})+'\n');
      for(const code of result.errorCodes)process.stdout.write(t('setup.errorLine',{code})+'\n');
    }
    return local.status==='fail'?1:local.status==='warning'?2:setupExitCode(result);
  });
}
