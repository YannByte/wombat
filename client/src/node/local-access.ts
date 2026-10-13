import { StringDecoder } from 'node:string_decoder';
import { CoreError } from '../errors.js';

/** Recognize local OS denials without returning native diagnostic text or paths. */
export function isLocalAccessDenied(error: unknown): boolean {
  return error !== null && typeof error === 'object' && 'code' in error &&
    (error.code === 'EPERM' || error.code === 'EACCES');
}

export function localAccessError(): CoreError {
  return new CoreError('LOCAL_ACCESS_DENIED', 'Local process or service access was denied');
}

/** Retain at most one short diagnostic line; logs are never commands or grants. */
export class NativeDiagnostics {
  private readonly decoder = new StringDecoder('utf8');
  private tail = '';
  private truncated = false;
  private denied = false;

  observe(error: unknown): void {
    this.denied ||= isLocalAccessDenied(error);
  }

  clear(): void {
    this.denied = false;
    this.tail = '';
    this.truncated = false;
  }

  write(chunk: Buffer): void {
    const lines = this.decoder.write(chunk).split('\n');
    for (let i = 0; i < lines.length; i++) {
      if (!this.truncated) {
        this.tail += lines[i];
        if (this.tail.length > 2048) {
          this.tail = '';
          this.truncated = true;
        } else {
          // Node's syscall diagnostic, or the equivalent native OS diagnostic.
          this.denied ||= /^Error: (?:(?:listen|connect|bind|spawn) (?:EPERM|EACCES)\b|(?:EPERM|EACCES): .*\b(?:open|listen|connect|bind|spawn)\b)/.test(this.tail) ||
            /^Error: .*\b(?:bind|listen|connect|spawn)\b.*(?:Operation not permitted \(os error 1\)|Permission denied \(os error 13\))/.test(this.tail);
        }
      }
      if (i < lines.length - 1) {
        this.tail = '';
        this.truncated = false;
      }
    }
  }

  error(code: string, message: string): CoreError {
    return this.denied ? localAccessError() : new CoreError(code, message);
  }
}
