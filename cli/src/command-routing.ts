type CommandRoute = readonly [run: (args: string[]) => Promise<number>, errorVersion: 1 | 3];

/** Human CLI routes stay lazy; Agent methods remain owned by the generated Rust schema. */
export const commandRoutes = new Map<string, CommandRoute>([
  ['service', [async args => (await import('./service-cli.js')).runServiceCli(args), 3]],
  ['monitor', [async args => (await import('./monitor-cli.js')).runMonitorCli(args), 3]],
  ['setup', [async args => (await import('./setup-cli.js')).runSetupCli(args), 1]],
  ['collection', [async args => (await import('./collection-cli.js')).runCollectionCli(args), 1]],
  ['hook', [async args => (await import('./collection-cli.js')).runHookCli(args), 1]],
  ['account', [async args => (await import('./account-cli.js')).runAccountCli(args), 1]],
  ['directories', [async args => (await import('./directories-cli.js')).runDirectoriesCli(args), 3]],
  ['prices', [async args => (await import('./prices-cli.js')).runPricingCli(args), 1]],
  ['optimize', [async args => (await import('./config-cli.js')).runConfigCli(args), 1]],
  ['skill', [async args => (await import('./skill-cli.js')).runSkillCli(args), 1]],
  ['update', [async args => (await import('./update-cli.js')).runUpdateCli(args), 1]],
  ['doctor', [async args => (await import('./doctor-cli.js')).runDoctorCli(args), 1]],
]);
export const usageActions = new Set(['refresh', 'usage', 'threads', 'turns', 'steps', 'compare', 'investigate', 'trajectory', 'resources', 'review', 'context', 'statistics']);
export const commandNames = [...usageActions, ...commandRoutes.keys(), 'timing', 'api', 'call'];

/** Find a command without loading command implementations or presentation code. */
export function firstCommandIndex(argv: readonly string[]): number {
  let index = 0;
  while (argv[index] === "--lang" || argv[index]?.startsWith("--lang=")) {
    index += argv[index] === "--lang" ? 2 : 1;
  }
  return index;
}
