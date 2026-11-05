import type { LocalResponse } from './timing-local-response.js';
export function validate(value: unknown): value is LocalResponse;
export namespace validate { let errors: readonly {instancePath:string;schemaPath:string;keyword:string;params:Record<string,unknown>;message?:string}[] | null; }
