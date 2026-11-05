import type { Description } from './agent-description.js';
export function validate(value: unknown): value is Description;
export namespace validate { let errors: readonly {instancePath:string;schemaPath:string;keyword:string;params:Record<string,unknown>;message?:string}[] | null; }
