import type { Request } from './directories-request.js';
export function validate(value: unknown): value is Request;
export namespace validate { let errors: readonly {instancePath:string;schemaPath:string;keyword:string;params:Record<string,unknown>;message?:string}[] | null; }
