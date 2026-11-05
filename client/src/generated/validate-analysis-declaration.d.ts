import type { AnalysisDeclaration } from './analysis-declaration.js';
export function validate(value: unknown): value is AnalysisDeclaration;
export namespace validate { let errors: readonly {instancePath:string;schemaPath:string;keyword:string;params:Record<string,unknown>;message?:string}[] | null; }
