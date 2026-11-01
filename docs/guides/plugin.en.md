# Use the Codex plugin

[中文](plugin.md) | English

Follow the [installation guide](installation.en.md) to install Wombat and a compatible local Codex. Invoke `$wombat:wombat` in a new project conversation.

## Select evidence by task

Use Wombat to resume work, investigate an obstacle, verify delivery, review a period or improve a project mechanism. Ask directly for usage or costs when that is all you need. No project initialization or mandatory evidence sweep is required.

Checks start read-only. A suggestion identifies the problem, evidence location, owner file, smallest change and recheck criterion. Codex owns authorized edits, tests, task creation and recovery. Wombat supplies facts and evidence; a static recheck does not establish later adoption or savings.

## Update and remove

Rerun the installation command with its plugin option to update the runtime and plugin. `wombat system update` updates only the runtime. Codex manages the plugin:

```sh
codex plugin remove wombat@wombat-local
```

Removal preserves product data. Wombat is delivered as a Codex plugin, without automatic Hook registration or standalone Skill installation.

## Optional observations and limits

Log queries require no Hooks. To inspect native Hook registrations, explicitly run `wombat system doctor --plugin --hooks --project /path/to/project --json`. User-managed event receipt is described in the [CLI guide](cli.en.md); receipts add no Tokens and do not establish complete coverage.

Wombat queries make no model calls. Codex conversations and authorized work use model tokens. Discovery, runtime compatibility, data readiness and conversation quality need separate verification. Cross-platform and complete native event coverage remain limited; see [development](../development/plugin.en.md).
