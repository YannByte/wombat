import path from 'node:path';
import { CoreError } from '../../errors.js';
import type { ConfigRequest, QueryOptions } from '../../client.js';
import { invokeOperation, type CoreProcessOptions } from '../core.js';
import { connectCodex } from './rpc.js';
import { nativeVersion, type CodexOptions } from './process.js';

type Skill = { name: string; description: string; path: string; enabled: boolean; scope: string };
type Context = { cwd: string; complete: boolean; skills: Skill[] };
type Capture = { nativeVersion: string | null; checkedAt: string | null; errorCode: string | null; contexts: Context[] };
const object = (v: unknown): v is Record<string, unknown> => !!v && typeof v === 'object' && !Array.isArray(v);
const text = (v: unknown, max: number): v is string => typeof v === 'string' && v.length > 0 && Buffer.byteLength(v) <= max;

/** Read-only native listing; no model session, MCP startup or configuration mutation. */
export async function captureSkills(request: Pick<ConfigRequest, 'roots' | 'projectRoots'>, query: QueryOptions, options: CoreProcessOptions & CodexOptions): Promise<Capture> {
  const empty = (errorCode: string): Capture => ({ nativeVersion: null, checkedAt: null, errorCode, contexts: [] });
  const deadline = new AbortController(), timer = setTimeout(() => deadline.abort(), 5000);
  const signal = query.signal ? AbortSignal.any([query.signal, deadline.signal]) : deadline.signal;
  const q = { ...query, signal };
  let rpc: Awaited<ReturnType<typeof connectCodex>> | undefined;
  try {
    // Reuse the core's authorized source/project resolution; another source home must
    // never inherit enablement facts from this machine's active Codex installation.
    const context = await invokeOperation('native_hook_context', request, q, { ...options, maxResponseBytes: 1024 * 1024 });
    if (!object(context) || typeof context.nativeHomeSelected !== 'boolean' || !Array.isArray(context.projects) || !context.projects.every(p => text(p, 4096))) return empty('nativeSkillProtocolError');
    if (!context.nativeHomeSelected) return empty('nativeSkillHomeNotSelected');
    if (!context.projects.length || context.projects.length > 64) return empty('nativeSkillProjectUnavailable');
    const version = await nativeVersion(q, options);
    rpc = await connectCodex(q, options);
    const response = await rpc.request('skills/list', { cwds: context.projects, forceReload: true });
    if (!object(response) || !Array.isArray(response.data) || response.data.length > 64) return empty('nativeSkillProtocolError');
    const contexts: Context[] = [], seen = new Set<string>();
    let count = 0;
    for (const raw of response.data) {
      if (!object(raw) || !text(raw.cwd, 4096) || !context.projects.includes(raw.cwd) || seen.has(raw.cwd) || !Array.isArray(raw.skills) || !Array.isArray(raw.errors)) return empty('nativeSkillProtocolError');
      seen.add(raw.cwd);
      const entry: Context = { cwd: raw.cwd, complete: raw.errors.length === 0, skills: [] };
      const paths = new Map<string, Skill[]>();
      for (const s of raw.skills) {
        if (++count > 4096) return empty('nativeSkillResourceLimit');
        if (!object(s) || !text(s.name, 1024) || !text(s.description, 65536) || !text(s.path, 4096) || !path.isAbsolute(s.path) || path.basename(s.path) !== 'SKILL.md' || typeof s.enabled !== 'boolean' || !text(s.scope, 128)) { entry.complete = false; continue; }
        const skill = { name: s.name, description: s.description, path: s.path, enabled: s.enabled, scope: s.scope };
        const list = paths.get(s.path) ?? []; list.push(skill); paths.set(s.path, list);
      }
      for (const list of paths.values()) {
        if (list.some(s => JSON.stringify(s) !== JSON.stringify(list[0]))) entry.complete = false;
        else entry.skills.push(list[0]);
      }
      contexts.push(entry);
    }
    const result: Capture = { nativeVersion: version, checkedAt: new Date().toISOString(), errorCode: contexts.length !== context.projects.length || contexts.some(c => !c.complete) ? 'nativeSkillListPartial' : null, contexts };
    return Buffer.byteLength(JSON.stringify(result)) <= 768 * 1024 ? result : empty('nativeSkillResourceLimit');
  } catch (error) {
    if (query.signal?.aborted) throw new CoreError('CANCELLED', 'Cancelled');
    return empty(error instanceof CoreError ? error.code : 'nativeSkillUnavailable');
  } finally { clearTimeout(timer); rpc?.close(); }
}
