# Wombat user plugin

[中文](plugin.md) | English

`plugin/skills/wombat/` is the single user Skill, with task references loaded as needed. Development workflows stay in `.agents/skills/`. `plugin/package.json` and `agents/openai.yaml` own metadata; the root manifest alone owns the version. The main Skill’s [shared rules](../../plugin/skills/wombat/SKILL.md) favor clear, concise Chinese and English without strict dictionary matching.

## Build and install

```sh
corepack pnpm build
codex plugin marketplace add ./dist/skill
codex plugin add wombat@wombat-local
```

Build produces only `dist/skill/plugin` and one local marketplace entry. Archives use `lib/skill`. `wombat-runtime.json` binds content hashes, version and required capabilities. There is no standalone Skill copy or collection-plugin resource package. Codex manages installation, updates and removal. `wombat system update` updates only the runtime; see [installation](../guides/installation.en.md) for complete updates.

## Verification boundaries

`corepack pnpm verify:skill-native -- --output-dir /tmp/wombat-native --codex-bin /absolute/path/to/codex` uses a fresh profile to verify single-plugin installation, discovery, runtime compatibility and data-preserving removal. Default setup does not probe Hooks; explicit `--hooks` checks optional registration. Plugin installation does not establish indexed data or event coverage.

After a build, `corepack pnpm verify:skill-conversation -- --output-dir /tmp/wombat-conversations --agent-bin /absolute/path/to/codex --language both` runs model conversations when authorized. `--scenario` selects usage, pricing, timing, statistics, monitor, account, daily-review, task-retro or config-review; `all` runs every scenario. Each case starts with fresh data and installation paths; only its follow-up reuses state. The configuration case authorizes only the selected description edit and rechecks the original readView/baselineId. Reports retain usage and facts; a human still reviews advice quality. Do not claim model advice was verified without running this check.
