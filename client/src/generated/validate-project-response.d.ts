import type { Response } from './project-response.js';
export function validate(value: unknown): value is Response;
export namespace validate { let errors: readonly {instancePath:string;schemaPath:string;keyword:string;params:Record<string,unknown>;message?:string}[] | null; }
