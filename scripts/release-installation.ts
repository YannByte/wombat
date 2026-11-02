/** Installers install a foreground CLI; no resident process or automatic startup. */
export function installSourceArgs(version: string, baseUrl: string, publicDownload: boolean, windows: boolean): string[] {
  if (!publicDownload) return [...(windows ? ['-Version', version, '-BaseUrl', baseUrl] : ['--version', version, '--base-url', baseUrl])];
  return [...(version.includes('-') ? (windows ? ['-Version', version] : ['--version', version]) : [])];
}
