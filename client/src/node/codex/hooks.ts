import { createHash } from 'node:crypto';
import { constants } from 'node:fs';
import { open, realpath, stat } from 'node:fs/promises';
import path from 'node:path';
import { isDeepStrictEqual } from 'node:util';
import type { ConfigRequest, QueryOptions } from '../../client.js';
import { CoreError } from '../../errors.js';
import { invokeOperation, type CoreProcessOptions } from '../core.js';
import { connectCodex } from './rpc.js';
import { nativeVersion, type CodexOptions } from './process.js';

type Context = { projects: string[]; roots: string[]; nativeHomeSelected: boolean; hasDeclarations: boolean };
type Capture = { nativeVersion: string | null; checkedAt: string | null; contexts: CapturedContext[] };
type CapturedContext = { cwd: string; complete: boolean; hooks: CapturedHook[] };
type CapturedHook = { key: string; eventName: string; sourcePath: string; sourceHash: string; currentHash: string; enabled: boolean; trustStatus: string; handlerType: string; source: string; pluginId: string | null; command: string | null };
const object = (v: unknown): v is Record<string, unknown> => !!v && typeof v === 'object' && !Array.isArray(v);
const text = (v: unknown, max = 4096): v is string => typeof v === 'string' && v.length > 0 && v.length <= max;
const within = (file: string, root: string) => { const p = path.relative(root, file); return p === '' || (!path.isAbsolute(p) && p !== '..' && !p.startsWith('..' + path.sep)); };

/** Expanded plugin commands cross only the private kernel channel; never the public DTO or cache. */
export async function captureHooks(request: Pick<ConfigRequest, 'roots' | 'projectRoots'>, query: QueryOptions, options: CoreProcessOptions & CodexOptions): Promise<Capture> {
  const empty: Capture = { nativeVersion: null, checkedAt: null, contexts: [] };
  const deadline = new AbortController(), timer = setTimeout(() => deadline.abort(), 5000);
  const signal = query.signal ? AbortSignal.any([query.signal, deadline.signal]) : deadline.signal;
  const q = { ...query, signal };
  let rpc: Awaited<ReturnType<typeof connectCodex>> | undefined;
  try {
    const context = await invokeOperation('native_hook_context', request, q, { ...options, maxResponseBytes: 1024 * 1024 }) as Context;
    if (!context.hasDeclarations || !context.nativeHomeSelected || !context.projects.length || context.projects.length > 64) return empty;
    const version = await nativeVersion(q, options);
    rpc = await connectCodex(q, options);
    const checkedAt = new Date().toISOString();
    const before = await rpc.request('hooks/list', { cwds: context.projects });
    if (!object(before) || !Array.isArray(before.data) || before.data.length > 64) return empty;
    const hashes = new Map<string, string | null>(); let total = 0, count = 0;
    const hashFile = async (file: string): Promise<string | null> => {
      if (hashes.has(file)) return hashes.get(file)!;
      hashes.set(file, null);
      if (signal.aborted) throw new CoreError('CANCELLED', 'Cancelled');
      let handle: Awaited<ReturnType<typeof open>> | undefined;
      try {
        const canonical = await realpath(file);
        if (canonical !== file || !context.roots.some(root => within(canonical, root))) return null;
        if (!(await stat(file)).isFile()) return null;
        handle = await open(file, constants.O_RDONLY | (constants.O_NOFOLLOW ?? 0) | (constants.O_NONBLOCK ?? 0));
        const metadata = await handle.stat();
        if (!metadata.isFile() || metadata.size > 10 * 1024 * 1024 || total + metadata.size > 32 * 1024 * 1024) return null;
        total += metadata.size;
        const hash = createHash('sha256'), buffer = Buffer.alloc(64 * 1024); let bytes = 0;
        for (;;) {
          if (signal.aborted) throw new CoreError('CANCELLED', 'Cancelled');
          const read = await handle.read(buffer, 0, Math.min(buffer.length, metadata.size + 1 - bytes), null);
          if (!read.bytesRead) break;
          bytes += read.bytesRead; if (bytes > metadata.size) return null;
          hash.update(buffer.subarray(0, read.bytesRead));
        }
        if (bytes !== metadata.size || await realpath(file) !== canonical) return null;
        const value = hash.digest('hex'); hashes.set(file, value); return value;
      } catch { return null; } finally { await handle?.close(); }
    };
    const contexts: CapturedContext[] = [], seen = new Set<string>();
    for (const entry of before.data) {
      if (!object(entry) || !text(entry.cwd) || !context.projects.includes(entry.cwd) || seen.has(entry.cwd)) return empty;
      seen.add(entry.cwd);
      const current: CapturedContext = { cwd: entry.cwd, complete: Array.isArray(entry.warnings) && !entry.warnings.length && Array.isArray(entry.errors) && !entry.errors.length, hooks: [] };
      contexts.push(current);
      if (!Array.isArray(entry.hooks)) { current.complete = false; continue; }
      count += entry.hooks.length; if (count > 512) return empty;
      const keys = new Map<string, number>();
      for (const hook of entry.hooks) if (object(hook) && text(hook.key)) keys.set(hook.key, (keys.get(hook.key) ?? 0) + 1);
      for (const hook of entry.hooks) {
        if (!object(hook) || !text(hook.key) || keys.get(hook.key) !== 1 || !text(hook.eventName, 64) || !text(hook.sourcePath) || !text(hook.currentHash, 256) || typeof hook.enabled !== 'boolean'
          || !text(hook.trustStatus) || !['managed', 'untrusted', 'trusted', 'modified'].includes(hook.trustStatus)
          || !text(hook.handlerType) || !['command', 'mcpTool', 'prompt', 'agent'].includes(hook.handlerType) || !text(hook.source, 128) || !['system','user','project','mdm','sessionFlags','plugin','cloudRequirements','cloudManagedConfig','legacyManagedConfigFile','legacyManagedConfigMdm','unknown'].includes(hook.source)
          || (hook.source === 'plugin' ? !text(hook.pluginId, 256) : hook.pluginId != null)) { current.complete = false; continue; }
        const sourceHash = await hashFile(hook.sourcePath);
        if (!sourceHash) { current.complete = false; continue; }
        const command = hook.source === 'plugin' && hook.handlerType === 'command' && typeof hook.command === 'string' && Buffer.byteLength(hook.command) <= 16_384 ? hook.command : null;
        current.hooks.push({ key: hook.key, eventName: hook.eventName, sourcePath: hook.sourcePath, sourceHash, currentHash: hook.currentHash, enabled: hook.enabled, trustStatus: hook.trustStatus, handlerType: hook.handlerType, source: hook.source, pluginId: hook.source === 'plugin' ? hook.pluginId as string : null, command });
      }
    }
    const after = await rpc.request('hooks/list', { cwds: context.projects });
    // Includes matcher/command/timeout/plugin metadata, not just enabled and trust bits.
    if (!isDeepStrictEqual(before, after) || signal.aborted) return empty;
    const result = { nativeVersion: version, checkedAt, contexts };
    return Buffer.byteLength(JSON.stringify(result)) <= 768 * 1024 ? result : empty;
  } catch (error) {
    if (query.signal?.aborted) throw new CoreError('CANCELLED', 'Cancelled');
    return empty;
  } finally { clearTimeout(timer); rpc?.close(); }
}
