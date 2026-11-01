type CommandRoute = readonly [run: (args: string[]) => Promise<number>, errorVersion: 1 | 3];

/** Five human entries; structured methods remain owned by the generated Rust schema. */
export const commandRoutes = new Map<string, CommandRoute>([
  ['check', [async args => (await import('./config-cli.js')).runConfigCli(args), 1]],
  ['system', [async args => (await import('./system-cli.js')).runSystemCli(args), 1]],
]);
export const commandNames = ['usage', 'check', 'system', 'api', 'call'];

export function firstCommandIndex(argv: readonly string[]): number {
  let index = 0;
  while (argv[index] === '--lang' || argv[index]?.startsWith('--lang=')) index += argv[index] === '--lang' ? 2 : 1;
  return index;
}
