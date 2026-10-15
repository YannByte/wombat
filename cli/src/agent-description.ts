import {
  CoreError,
  agentRequestSchema,
  agentResultSchemas,
  type AgentDescription,
} from '@wombat/client';
import metadata from '../package.json' with { type: 'json' };
import { INPUT_BYTES, INPUT_TIMEOUT_MS, OUTPUT_BYTES } from './agent-limits.js';
const object = (v: unknown): v is Record<string, unknown> =>
  !!v && typeof v === 'object' && !Array.isArray(v);
const invalid = () => new CoreError('INVALID_ARGUMENT', 'Invalid Agent request');
function variants(): Record<string, unknown>[] {
  const list = agentRequestSchema.oneOf;
  if (!Array.isArray(list) || !list.every(object))
    throw new CoreError('PROTOCOL_ERROR', 'Invalid bundled API schema');
  return list;
}
function methodOf(branch: Record<string, unknown>): string {
  const properties = branch.properties,
    method = object(properties) ? properties.method : null;
  const name = object(method) ? method.const : null;
  if (typeof name !== 'string') throw new CoreError('PROTOCOL_ERROR', 'Invalid bundled API method');
  return name;
}
/** Keep only definitions reachable from the selected method; the Rust schema is authoritative. */
function methodSchema(branch: Record<string, unknown>): Record<string, unknown> {
  const definitions = agentRequestSchema.definitions;
  if (!object(definitions)) throw new CoreError('PROTOCOL_ERROR', 'Missing API definitions');
  const selected: Record<string, unknown> = {};
  const visit = (value: unknown): void => {
    if (Array.isArray(value)) {
      value.forEach(visit);
      return;
    }
    if (!object(value)) return;
    if (typeof value.$ref === 'string' && value.$ref.startsWith('#/definitions/')) {
      const name = value.$ref.slice('#/definitions/'.length);
      if (!Object.hasOwn(selected, name)) {
        if (!Object.hasOwn(definitions, name))
          throw new CoreError('PROTOCOL_ERROR', 'Missing API definition');
        selected[name] = definitions[name];
        visit(definitions[name]);
      }
    }
    Object.values(value).forEach(visit);
  };
  visit(branch);
  return { $schema: agentRequestSchema.$schema, ...branch, definitions: selected };
}
export function describeAgentApi(method?: string, includeOutput = false): AgentDescription {
  const branches = variants(),
    methods = branches.map(methodOf);
  const branch = method ? branches.find((b) => methodOf(b) === method) : undefined;
  if (method && !branch) throw invalid();
  return {
    outputVersion: 1,
    runtimeVersion: metadata.version,
    methods,
    methodDescriptions: Object.fromEntries(
      branches.map((b) => [
        methodOf(b),
        typeof b.description === 'string' ? b.description : methodOf(b),
      ]),
    ),
    selectedMethod: method ?? null,
    inputSchema: branch ? methodSchema(branch) : null,
    outputSchema: includeOutput && method ? agentResultSchemas[method] : null,
    stdinBytes: INPUT_BYTES,
    stdinTimeoutMs: INPUT_TIMEOUT_MS,
    stdoutBytes: OUTPUT_BYTES,
  };
}
