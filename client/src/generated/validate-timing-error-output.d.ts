import type { TimingErrorOutput } from './timing-error-output.js';
export function validate(value: unknown): value is TimingErrorOutput;
export namespace validate { let errors: readonly {instancePath:string;schemaPath:string;keyword:string;params:Record<string,unknown>;message?:string}[] | null; }
