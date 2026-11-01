// Source metadata is display data: never interpret terminal escape sequences.
export function terminalText(text: string): string {
  return text.replace(/\x1b\][^\x07\x1b]*(?:\x07|\x1b\\)/g, '')
    .replace(/\x1b\[[0-?]*[ -/]*[@-~]/g, '').replace(/[\x00-\x1f\x7f-\x9f]/g, ' ');
}

/** Preserve transcript lines while keeping terminal control data inert. */
export function terminalBlock(text:string):string {
  return text.split(/\r?\n/).map(terminalText).join('\n');
}
