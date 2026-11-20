import { constants } from 'node:fs';
import { randomUUID } from 'node:crypto';
import { mkdir, mkdtemp, open, realpath, rm, symlink, writeFile } from 'node:fs/promises';
import { createServer } from 'node:http';
import { tmpdir } from 'node:os';
import path from 'node:path';
import { CoreError } from '../../errors.js';
import { isLocalAccessDenied, localAccessError } from '../local-access.js';
import { nativeVersion } from './process.js';
import { runTrialSession } from './trial-session.js';

const profile = 'wombat-trial-probe';
const provider = 'wombat_probe';
const model = 'gpt-5.4';
const probePrompt = 'Run the synthetic Wombat trial and report the result.';
const artifact = 'result.txt';
const artifactLimit = 4096;
const snapshot = '2\n5\n';
const correctTotal = '7\n';
const cases = [
  { name: 'baseline', program: '/usr/bin/awk \'END { print NR }\' "$1" > result.txt\n', expected: '2\n' },
  { name: 'candidate', program: '/usr/bin/awk \'{ total += $1 } END { print total }\' "$1" > result.txt\n', expected: correctTotal },
] as const;

const probe = `
set -eu
input=$1
source=$2
neighbor=$3
neighbor_inputs=$4
home=$5
port=$6
export TMPDIR="$PWD/temp"
test "$(/bin/cat "$input/numbers.txt")" = "$(/usr/bin/printf '2\\n5')"
/usr/bin/touch "$TMPDIR/own-file"
for location in "$source" "$neighbor" "$neighbor_inputs" "$home"; do
  if /bin/cat "$location/canary" >/dev/null 2>&1; then exit 31; fi
  if /usr/bin/touch "$location/write-probe" 2>/dev/null; then exit 32; fi
done
if /bin/cat control-link >/dev/null 2>&1; then exit 33; fi
if ( /usr/bin/printf 'unexpected write\\n' > control-link ) 2>/dev/null; then exit 36; fi
if /usr/bin/touch "$input/write-probe" 2>/dev/null; then exit 34; fi
test -x /usr/bin/nc
if /usr/bin/nc -z -w 1 127.0.0.1 "$port" >/dev/null 2>&1; then exit 35; fi
/bin/sh harness.sh "$input/numbers.txt"
/usr/bin/printf 'TRIAL_PROBE_PASSED\\n'
`;

function fileRule(location: string, access: 'read' | 'write') {
  return `${JSON.stringify(location)} = ${JSON.stringify(access)}`;
}

export async function checkTrialProbeArtifact(workspace: string, expected: string) {
  try {
    // O_NONBLOCK prevents a named pipe from blocking before the file-type check.
    const file = await open(path.join(workspace, artifact), constants.O_RDONLY | constants.O_NOFOLLOW | constants.O_NONBLOCK);
    try {
      const metadata = await file.stat();
      const expectedBytes = Buffer.from(expected);
      if (metadata.isFile() && metadata.nlink === 1 && metadata.size === expectedBytes.length && metadata.size <= artifactLimit) {
        const buffer = Buffer.alloc(metadata.size + 1);
        const { bytesRead } = await file.read(buffer);
        if (bytesRead === expectedBytes.length && buffer.subarray(0, bytesRead).equals(expectedBytes)) return;
      }
    } finally {
      await file.close();
    }
  } catch (error) {
    if (!(error instanceof Error && 'code' in error && (error.code === 'ENOENT' || error.code === 'ELOOP' || error.code === 'EISDIR'))) throw error;
  }
  throw new CoreError('TRIAL_PROBE_ARTIFACT', 'Trial did not produce the expected regular file');
}

function passedToolCheck(body: string): boolean {
  let request: unknown;
  try { request = JSON.parse(body); } catch { return false; }
  return !!request && typeof request === 'object' && 'input' in request && Array.isArray(request.input) && request.input.some((item: unknown) =>
    !!item && typeof item === 'object' && 'type' in item && item.type === 'function_call_output' &&
    'call_id' in item && item.call_id === 'call_wombat' && 'output' in item && JSON.stringify(item.output).includes('TRIAL_PROBE_PASSED'));
}

async function startTrialProvider() {
  const route = `/${randomUUID()}/v1`;
  let trial: { workspace: string; command: string; requests: number } | undefined;
  const server = createServer(async (request, response) => {
    if (request.method !== 'POST' || request.url !== `${route}/responses` || !trial || trial.requests >= 2) {
      response.writeHead(404).end();
      return;
    }
    const active = trial;
    let bytes = 0;
    const chunks: Buffer[] = [];
    try {
      for await (const chunk of request) {
        bytes += chunk.length;
        if (bytes > 512 * 1024) { response.writeHead(413).end(); return; }
        chunks.push(chunk);
      }
    } catch { return; } // The native process may disconnect after cancellation.
    if (active !== trial || active.requests >= 2) { response.writeHead(409).end(); return; }
    const body = Buffer.concat(chunks).toString('utf8');
    const first = active.requests === 0;
    if (!(first ? body.includes(probePrompt) : passedToolCheck(body))) {
      response.writeHead(422).end();
      return;
    }
    active.requests++;
    const output = first ? {
      type: 'function_call', id: 'fc_wombat', call_id: 'call_wombat', name: 'exec_command', status: 'completed',
      arguments: JSON.stringify({ cmd: active.command, workdir: active.workspace, login: false }),
    } : {
      type: 'message', id: 'msg_wombat', role: 'assistant', status: 'completed',
      content: [{ type: 'output_text', text: 'Synthetic trial completed.', annotations: [] }],
    };
    const id = `resp_wombat_${active.requests}`;
    const event = (type: string, value: object) => `event: ${type}\ndata: ${JSON.stringify({ type, ...value })}\n\n`;
    response.writeHead(200, { 'content-type': 'text/event-stream' });
    response.end([
      event('response.created', { response: { id, output: [] } }),
      event('response.output_item.done', { item: output, output_index: 0 }),
      event('response.completed', { response: { id, output: [output], usage: { input_tokens: 11, output_tokens: 2, total_tokens: 13 } } }),
    ].join(''));
  });
  await new Promise<void>((resolve, reject) => {
    server.once('error', reject);
    server.listen(0, '127.0.0.1', () => {
      server.off('error', reject);
      resolve();
    });
  });
  const address = server.address();
  if (!address || typeof address === 'string') throw new CoreError('TRIAL_PROBE_LISTENER', 'Local control listener has no TCP address');
  return {
    url: `http://127.0.0.1:${address.port}${route}`, port: address.port,
    prepare(workspace: string, args: string[]) {
      const command = args.map(part => `'${part.replaceAll("'", "'\\''")}'`).join(' ');
      trial = { workspace, command, requests: 0 };
    },
    verify() {
      if (trial?.requests !== 2) throw new CoreError('TRIAL_PROBE_PROTOCOL', 'Trial did not complete the synthetic provider exchange');
      trial = undefined;
    },
    close() {
      return new Promise<void>((resolve, reject) => {
        server.close(error => error ? reject(error) : resolve());
        server.closeAllConnections();
      });
    },
  };
}

/** Qualifies the same-session execution path against a synthetic local provider. */
export async function verifyHarnessTrial(codexBinaryPath: string, signal: AbortSignal) {
  if (process.platform !== 'darwin' || process.arch !== 'arm64') {
    throw new CoreError('TRIAL_PROBE_PLATFORM', 'The trial command boundary is currently verified on macOS arm64');
  }
  if (!path.isAbsolute(codexBinaryPath)) throw new CoreError('TRIAL_PROBE_BINARY', 'An absolute Codex executable path is required');
  if (signal.aborted) throw new CoreError('CANCELLED', 'Cancelled');
  const root = await mkdtemp(path.join(await realpath(tmpdir()), 'wombat-trial-'));
  const runs: { variant: typeof cases[number]['name']; sessionCheck: 'passed'; artifact: string; artifactCheck: 'passed'; taskCheck: 'passed' | 'failed' }[] = [];
  let phase = 'setup';
  let listener: Awaited<ReturnType<typeof startTrialProvider>> | undefined;
  try {
    const source = path.join(root, 'source');
    const codexHome = path.join(root, 'codex');
    const variants = cases.map(test => ({ ...test, workspace: path.join(root, test.name), inputs: path.join(root, `${test.name}-inputs`) }));
    for (const directory of [source, codexHome, ...variants.flatMap(variant => [variant.workspace, variant.inputs])]) {
      await mkdir(directory, { mode: 0o700 });
      await writeFile(path.join(directory, 'canary'), 'synthetic canary\n', { mode: 0o600 });
    }
    for (const variant of variants) {
      await mkdir(path.join(variant.workspace, 'temp'), { mode: 0o700 });
      await writeFile(path.join(variant.inputs, 'numbers.txt'), snapshot, { mode: 0o600 });
      await writeFile(path.join(variant.workspace, 'harness.sh'), variant.program, { mode: 0o600 });
      await symlink(path.join(source, 'canary'), path.join(variant.workspace, 'control-link'));
    }

    phase = 'codex-version';
    const options = { codexBinaryPath, codexHome, codexTimeoutMs: 15_000 };
    const env = { HOME: root, PATH: '/usr/bin:/bin:/usr/sbin:/sbin', TMPDIR: root };
    const version = await nativeVersion({ signal }, options, { cwd: root, env });

    phase = 'loopback-control';
    listener = await startTrialProvider();
    for (const [index, variant] of variants.entries()) {
      phase = variant.name;
      const rules = [
        `default_permissions = "${profile}"`,
        `model_provider = "${provider}"`,
        `model = "${model}"`,
        'project_doc_max_bytes = 0',
        'web_search = "disabled"',
        'allow_login_shell = false',
        'shell_environment_policy.inherit = "none"',
        `[model_providers.${provider}]`,
        'name = "Wombat local probe"',
        `base_url = ${JSON.stringify(listener.url)}`,
        'wire_api = "responses"',
        '[features]',
        'apps = false',
        'browser_use = false',
        'computer_use = false',
        'hooks = false',
        'multi_agent = false',
        'plugins = false',
        'shell_snapshot = false',
        `[permissions.${profile}.filesystem]`,
        '":minimal" = "read"',
        fileRule(variant.workspace, 'write'),
        fileRule(variant.inputs, 'read'),
        `[permissions.${profile}.network]`,
        'enabled = false',
      ];
      await writeFile(path.join(codexHome, 'config.toml'), rules.join('\n') + '\n', { mode: 0o600 });
      const sibling = variants[1 - index];
      const invocation = { cwd: variant.workspace, env };
      const session = { workspace: variant.workspace, profile, provider, model };
      if (index === 0) {
        phase = 'unselected-instructions';
        const outsideInstructions = path.join(codexHome, 'AGENTS.md');
        await writeFile(outsideInstructions, 'Synthetic instruction outside the trial inputs.\n', { mode: 0o600 });
        try {
          await runTrialSession(session, { signal }, options, invocation, probePrompt);
          throw new CoreError('TRIAL_PROBE_PROTOCOL', 'Trial accepted an unselected instruction file');
        } catch (error) {
          if (!(error instanceof CoreError && error.code === 'TRIAL_SESSION_INSTRUCTIONS')) throw error;
        } finally {
          await rm(outsideInstructions);
        }
      }
      phase = `${variant.name}-trial`;
      listener.prepare(variant.workspace, [
        '/bin/sh', '-c', probe, 'wombat', variant.inputs, source, sibling.workspace, sibling.inputs, codexHome, String(listener.port),
      ]);
      await runTrialSession(session, { signal }, options, invocation, probePrompt);
      listener.verify();
      await checkTrialProbeArtifact(variant.workspace, variant.expected);
      runs.push({ variant: variant.name, sessionCheck: 'passed', artifact, artifactCheck: 'passed', taskCheck: variant.expected === correctTotal ? 'passed' : 'failed' });
    }
    return { status: 'passed', scope: 'same_session_synthetic_trial', nativeVersion: version, unselectedInstructionsCheck: 'passed', runs };
  } catch (error) {
    const cause = isLocalAccessDenied(error) ? localAccessError() : error;
    if (cause instanceof CoreError) throw new CoreError(cause.code, cause.message, { phase, runs });
    throw error;
  } finally {
    try {
      await listener?.close();
    } finally {
      await rm(root, { recursive: true, force: true, maxRetries: 3 });
    }
  }
}
