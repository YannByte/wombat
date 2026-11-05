import type { Discovery } from './skill-discovery.js';
export function validate(value: unknown): value is Discovery;
export namespace validate { let errors: readonly {instancePath:string;schemaPath:string;keyword:string;params:Record<string,unknown>;message?:string}[] | null; }
