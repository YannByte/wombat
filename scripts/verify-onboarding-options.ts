import path from 'node:path';

/** Native event commands may present the shell argument inside another JSON string. */
export function commandMentionsExecutable(command: string, executable: string): boolean {
  const quoted = "'"+executable.replaceAll("'", "'\\''")+"'";
  return [executable, quoted, JSON.stringify(quoted).slice(1, -1)].some(value => command.includes(value));
}

export function onboardingOptions(args: string[]) {
  const values = new Map<string, string>();
  const allowed = ['output-dir', 'release-dir', 'codex-bin', 'agent-bin', 'agent-skill'];
  for (let i = 0; i < args.length; i++) {
    if (args[i] === '--' && i === 0) continue;
    const key = args[i].slice(2), value = args[++i];
    if (!allowed.includes(key) || !args[i - 1].startsWith('--') || !value || value.startsWith('--') || values.has(key))
      throw new Error('Expected unique --option VALUE arguments for onboarding verification');
    values.set(key, value);
  }
  const required = (key: string) => {
    const value = values.get(key);
    if (!value || !path.isAbsolute(value)) throw new Error(`--${key} requires an absolute path`);
    return value;
  };
  const optional = (key: string) => values.has(key) ? required(key) : undefined;
  const agentBin = optional('agent-bin'), agentSkill = values.get('agent-skill');
  if (Boolean(agentBin) !== Boolean(agentSkill) || agentSkill && !/^wombat:wombat$/.test(agentSkill))
    throw new Error('Supply both --agent-bin and --agent-skill with an installed Wombat invocation');
  return {output: required('output-dir'), release: required('release-dir'), codex: required('codex-bin'),
    agentBin, agentSkill};
}
