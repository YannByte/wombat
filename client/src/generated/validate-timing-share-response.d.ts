import type { ShareResponse } from './timing-share-response.js';
export function validate(value: unknown): value is ShareResponse;
export namespace validate { let errors: readonly {instancePath:string;schemaPath:string;keyword:string;params:Record<string,unknown>;message?:string}[] | null; }
