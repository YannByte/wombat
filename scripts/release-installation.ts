/** Disposable acceptance must not register services outside its temporary prefix. */
export function installSourceArgs(version: string, baseUrl: string, publicDownload: boolean, windows: boolean): string[] {
  const isolated = windows ? ['-NoService'] : ['--no-service'];
  if (!publicDownload) return [...(windows ? ['-Version', version, '-BaseUrl', baseUrl] : ['--version', version, '--base-url', baseUrl]), ...isolated];
  return [...(version.includes('-') ? (windows ? ['-Version', version] : ['--version', version]) : []), ...isolated];
}
