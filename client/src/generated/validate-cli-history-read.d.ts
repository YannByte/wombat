import type { HistoryRead } from './cli-history-read.js';
export function validate(value: unknown): value is HistoryRead;
export namespace validate { let errors: readonly {instancePath:string;schemaPath:string;keyword:string;params:Record<string,unknown>;message?:string}[] | null; }
