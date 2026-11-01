type CommandRoute = readonly [run: (args: string[]) => Promise<number>, errorVersion: 1 | 3];

/** Five human entries; structured methods remain owned by the generated Rust schema. */
export const commandRoutes = new Map<string, CommandRoute>([
  ['history', [async args => (await import('./history-cli.js')).runHistoryCli(args), 1]],
  ['check', [async args => (await import('./project-cli.js')).runProjectCli(args), 1]],
  ['clean', [async args => (await import('./artifacts-cli.js')).runArtifactsCli(args), 1]],
  ['system', [async args => (await import('./system-cli.js')).runSystemCli(args), 1]],
]);
export const commandNames = ['history', 'usage', 'check', 'clean', 'system'];
