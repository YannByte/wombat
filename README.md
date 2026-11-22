<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="assets/wombat-logo-dark.svg">
    <img src="assets/wombat-logo-light.svg" alt="Wombat" width="80" height="56">
  </picture>
</p>

# Wombat · Improve your Codex harness

English | [中文](README.zh-CN.md)

Wombat is a local Codex plugin that uses your work history to find changes worth making. When Codex gets stuck, skips a check, or needs repeated corrections, look up the original conversation and inspect the relevant project instructions, skills, tool settings, and verification steps. Together they make up your Codex harness.

Wombat provides evidence and check results. Choose what to change, then let Codex make the edit and repeat the check with the same scope and rules.

## Install Wombat

**Stable: [`v0.9.0`](https://github.com/YannByte/wombat/releases/tag/v0.9.0).**

Install a version of Codex that supports native plugins and provides the local `codex` command. Wombat's local analysis needs no separate API key or development tools.

Install Wombat and its plugin on macOS or Linux.

```sh
curl -fsSL https://raw.githubusercontent.com/YannByte/wombat/main/scripts/install/install.sh | sh -s -- --plugin
```

Install in Windows PowerShell.

```powershell
& ([scriptblock]::Create((irm https://raw.githubusercontent.com/YannByte/wombat/main/scripts/install/install.ps1))) -Plugin
```

## Start with a task that keeps going wrong

Open a project you have worked on in Codex. Start a new conversation and ask:

> $wombat:wombat Review this project's Codex history for the last 7 days. Find one task that failed repeatedly or needed several corrections. Quote the original message, inspect the related project instructions and verification steps, and suggest one specific change and how to check it.

**Example project.** Two release tasks stopped at the installation check. In turn 18 of the most recent conversation, you said, “We still can't find the installation checklist. Hold the release.” The current `AGENTS.md` still links to the removed `docs/release-checklist.md`. The same installation steps are now in `docs/install-checks.md`. The suggested fix is to update that one link and run the same check again.

After reviewing the evidence, you can say:

> Update only the checklist link. Run the original check again and tell me what still needs verification.

**Example result.** Codex updated the link. The original reference check passed, and the current checklist opens. Installing the latest version still needs a separate check.

If the selected dates have no usable records, extend the date range or ask Wombat to inspect the current project instructions first. If the plugin does not appear, see the [installation guide](docs/guides/installation.en.md).

## Use Wombat to improve your harness

| What you want to solve | How Wombat helps |
|---|---|
| Find a task worth reviewing | Start with failures, retries, usage, or changes in elapsed time. Find the related task and original conversation. |
| Diagnose a harness problem | Review how a task ran. Inspect project instructions, skills, MCP, hooks, and project verification steps. Check the evidence for configuration and actual use. |
| Choose and make a change | Review the evidence, scope, and verification steps. Choose what to change or keep, then let Codex make the authorized project changes. Preview cleanup candidates and confirm specific items before deletion. Deletion is permanent. |
| Check whether the change worked | Repeat the check for the original problem or cleanup items with the same scope and rules. See which tests ran and what still needs verification. Assess the longer-term effect on later tasks with comparable workloads and acceptance criteria. |
| Use what you learned next time | Recover past decisions, resume unfinished work, or add findings with sources to project documentation. |

On Apple silicon Macs, run the same real task with four versions of the project instructions. Apply a reviewed change and check that a new session loads it. [Real-task trials](docs/guides/plugin.en.md#harness-trial)

Find example prompts and expected results for each step. [Plugin guide](docs/guides/plugin.en.md#harness-workflow)

## Manage usage, installation, and integration

- **Manage usage.** View Token usage, estimated costs, available account allowance records, and budget alerts. [CLI usage and budgets](docs/guides/cli.en.md#usage)
- **Maintain the local installation.** Check the runtime and plugin discovery. To use only the CLI, omit `--plugin` or `-Plugin` from the install command. [Installation and troubleshooting](docs/guides/installation.en.md)
- **Connect scripts.** Query in JSON or use the structured interface. Dedicated methods cover directory access, optional collection, and display preferences. [Automation interface](docs/guides/cli.en.md#automation)

## Platforms and privacy

Release packages support macOS arm64/x64, Linux glibc arm64/x64, and Windows x64. Codex artifact cleanup currently targets macOS layouts.

CLI analysis stays local, sends no logs, and makes no model calls. Evidence selected by the skill enters the current Codex conversation and consumes Tokens. See [privacy](docs/reference/privacy.en.md).

## Update

Rerun the installation command above to update Wombat and its Codex plugin. Keep any options for a specific version or custom installation directory.

See the [installation guide](docs/guides/installation.en.md) for update options and runtime-only updates.

## Help and contribution

When you [report a problem or request](https://github.com/YannByte/wombat/issues), describe what happened and the tools you used. You can omit private logs. Use the [security policy](.github/SECURITY.md) to report security problems. See [Contributing](.github/CONTRIBUTING.md) for source development.

## License

[MIT](LICENSE). See [third-party notices](licenses/THIRD_PARTY_NOTICES.md) for dependency licenses.
