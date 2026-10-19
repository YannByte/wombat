# Use the Codex plugin

[中文](plugin.md) | English

Install a compatible local Codex first. Use the [installation guide](installation.en.md) to install Wombat and its plugin.

## Select an invocation

Open your project in Codex. Start a new conversation. Use the invocation discovered for that project:

- `$wombat:wombat` for the base plugin.
- `$wombat-collection:wombat` for the collection plugin.
- `$wombat` for a standalone Skill copy.

If multiple instances exist, select one explicitly. Installation does not scan logs or prove data readiness.

The plugin provides these entries. Use the task entry that matches the request:

| Skill | Task | Example request |
|---|---|---|
| `wombat` | Direct measurements, budgets and setup | “How many Tokens did I use today?” |
| `wombat-daily-review` | Daily or periodic usage review | “Review today’s usage and suggest one useful improvement.” |
| `wombat-task-retro` | Investigate one selected task | “Why did this task take so much effort? Show the original evidence.” |
| `wombat-config-review` | Review configuration and recheck authorized changes | “Review this project’s Skills and point to specific files.” |

For example, use `$wombat:wombat-daily-review` in the base plugin or `$wombat-collection:wombat-daily-review` in the collection plugin. The same namespace rule applies to the other task entries. The standalone copy provides the general `$wombat` entry only. Answers follow the user’s language; separate language installations are unnecessary.

Reviews start read-only. A report does not schedule future reports, and a configuration finding does not authorize an edit. After an authorized edit, configuration review rechecks the original suggestion; it does not equate smaller instructions with verified savings.

## Optional collection

Historical log analysis works without the collection plugin. For native observations, open **Setup and collection** in Web. See the [collection commands](cli.en.md) for status and pause/resume.

Plugin installation, collection mode, native Hook trust, and event receipt are separate steps. Review collection declarations in Codex `/hooks` before trusting them. Receipts do not add token accounting or prove complete coverage. The POSIX bridge is implemented; Windows Hook collection remains unverified.

## Update and remove

To update the runtime and plugin, rerun the installation command with its plugin option. `wombat update` updates only the runtime.

Codex manages plugin removal. Remove the selected base plugin:

```sh
codex plugin remove wombat@wombat-local
```

For the collection plugin, run:

```sh
codex plugin remove wombat-collection@wombat-local
```

Before switching from a standalone copy, run:

```sh
wombat skill uninstall --json
```

Uninstall removes only unchanged managed copies. Modified copies require manual review. Plugin removal preserves product data and other Hook declarations.

## Limits

Wombat queries and checks make no model calls. Codex conversations and authorized work use model tokens. Handoff acceptance does not prove execution or resolution; recheck the original issue after changes.

Local plugin discovery and removal were verified on Codex 0.160.0 on macOS. That evidence does not establish other platforms or complete conversational coverage. No public marketplace submission is claimed. See [plugin development](../development/plugin.en.md) for local trials and verification scope.
