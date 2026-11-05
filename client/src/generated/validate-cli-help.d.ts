import type { Help } from './cli-help.js';
export function validate(value: unknown): value is Help;
export namespace validate { let errors: readonly {instancePath:string;schemaPath:string;keyword:string;params:Record<string,unknown>;message?:string}[] | null; }
