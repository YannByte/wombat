import type { ErrorOutput } from './agent-error.js';
export function validate(value: unknown): value is ErrorOutput;
export namespace validate { let errors: readonly {instancePath:string;schemaPath:string;keyword:string;params:Record<string,unknown>;message?:string}[] | null; }
