# Use the Codex plugin

[中文](plugin.md) | English

Install a compatible local Codex first. Use the [installation guide](installation.en.md) to install Wombat and its plugin.

## Select an invocation

Open your project in Codex. Start a new conversation. Use the invocation discovered for that project:

- `$wombat:wombat` for the base plugin.
- `$wombat-collection:wombat` for the collection plugin.
- `$wombat` for a standalone Skill copy.

If multiple instances exist, select one explicitly. Installation does not scan logs or prove data readiness.

One main Skill loads a workflow for resuming work, investigating an obstacle, verifying delivery, reviewing a period or improving a project mechanism. Simple usage and cost questions query the CLI directly. The Skill does not initialize the project or collect every evidence type before answering.

Reviews start read-only. Each supported improvement names the observed problem, evidence, exact owner, smallest change and original check to rerun. Codex performs authorized edits and checks. Missing use records cannot justify deletion. A static recheck does not prove later adoption or savings.

## Optional collection

Historical log analysis works without the collection plugin. Native observation collection is optional and explicit. See the [collection commands](cli.en.md) for status and pause/resume.

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
