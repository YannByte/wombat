<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="assets/wombat-logo-dark.svg">
    <img src="assets/wombat-logo-light.svg" alt="Wombat" width="80" height="56">
  </picture>
</p>

# Wombat for Codex project usage and conversation history

English | [中文](README.zh-CN.md)

Wombat is a local CLI and plugin for OpenAI Codex. See how many Tokens a project used, find the tasks that used the most, and go back to the original messages to see what happened.

Wombat calculates usage from local Codex records and locates evidence. It also returns project findings and cleanup lists that you can recheck. Codex uses that evidence to explain what happened and make authorized project changes.

## Install Wombat for Codex

**Stable: [`v0.7.1`](https://github.com/YannByte/wombat/releases/tag/v0.7.1).**

Install a version of Codex that supports native plugins, with the `codex` command available locally. Wombat's local analysis needs no separate API key or development tools.

On macOS or Linux, install Wombat and its plugin:

```sh
curl -fsSL https://raw.githubusercontent.com/YannByte/wombat/main/scripts/install/install.sh | sh -s -- --plugin
```

On Windows PowerShell:

```powershell
& ([scriptblock]::Create((irm https://raw.githubusercontent.com/YannByte/wombat/main/scripts/install/install.ps1))) -Plugin
```

1. Open your project in Codex.
2. Start a new conversation.
3. Ask this question.

> $wombat:wombat How many Tokens did this project use today? Show the three tasks that used the most and quote the original request for the top task.

The answer shows the date range, project total, task ranking, original message location and any gaps in the records. If installation fails, follow the recovery step in the output. See the [installation guide](docs/guides/installation.en.md).

## See the result

### Find the tasks that used the most Tokens

> Which three tasks used the most Tokens in this project this week? Show me the original request and what happened next for the top task.

**Example answer.** This project recorded 1.4 million Tokens this week. The three highest-usage tasks were installer troubleshooting (900,000), Windows test fixes (180,000) and release notes (120,000).

In turn 18 of “Installer troubleshooting,” the request was “Retest the installer in a clean directory.” Turns 19–21 record the failed installation check and a retry with a different installation path.

Run the ranking directly from the project directory.

```sh
wombat usage top week --project . --limit 3
```

### Recheck a specific project finding

> Check this project's release instructions. Show any broken references and how to verify the fix.

**Example finding.** Wombat located a link in `AGENTS.md` to the missing `docs/release-checklist.md`. The finding includes the project scope and the rule that flagged it.

> Restore the checklist and rerun the original check.

**Example recheck.** After Codex restored the checklist, Wombat reran the same reference check in the same project. The missing-file finding cleared.

### Review what can be cleaned up

> Check the build outputs and temporary files in this project. Keep the release package in `dist/` and show the cleanup list first.

**Example preview.** Wombat lists the paths, observed sizes and evidence for cleanup.

- `target/` is a 2.4 GB Cargo build cache that can be regenerated.
- `.tmp/test-run-42/` contains 180 MB of files. Its ownership is unclear, so Wombat marks it for review.
- `dist/` is marked to keep because it contains the release package.

> Remove only `target/`, then verify the original path.

**Example recheck.** After Codex removed the approved directory, Wombat checked the original cleanup list and confirmed that `target/` was gone.

## More ways to use it

Ask with `$wombat:wombat`, for example:

| Task | Example request |
|---|---|
| Compare project usage | Compare the last two completed weeks for this project. Which tasks and models account for the largest differences? |
| Review a high-usage task | How much of this task's usage was cached input? Show the original request and the turns that record the work. |
| Find a decision across conversations | Search this month's conversations in this project for what I said about retries, timeouts or backoff. Give me the original quotes and their locations. |
| Investigate a recurring failure | Find earlier occurrences of this installer failure. Show the original error, what was tried and any recorded result. |
| Recheck a finding | Run the same check on the same project after the fix. Is the finding still present? |
| Review cleanup candidates | Which build outputs can be regenerated? List their paths and the reason each is a candidate. Keep the deliverables. |

Use these examples as a starting point. Add a project, date range or specific problem to narrow the scope. Follow up with “Show the original quote” or “Run the same check again.”

See the [plugin guide](docs/guides/plugin.en.md) for more uses, or the [CLI guide](docs/guides/cli.en.md) for direct queries.

## Common questions

### How do I find the original conversation for a task?

Ask `$wombat:wombat` to quote the conversation behind a task in the ranking, or provide the project, date range and words you remember. Wombat returns matching passages and their original locations.

### Can I use Wombat without the plugin?

Yes. Omit `--plugin` or `-Plugin` when installing and use the CLI directly. CLI queries support JSON output; see the [CLI guide](docs/guides/cli.en.md).

## Before you use it

- CLI analysis stays local and makes no model calls. Selected Skill evidence enters the current Codex conversation and consumes Tokens; see [privacy](docs/reference/privacy.en.md).
- Cleanup needs authorization for the selected items. Deletion is permanent. Cleanup checks protect conversations, state, configuration and credentials. Changed or unknown conditions block deletion. See the [cleanup guide](docs/guides/cli.en.md).

Release packages support macOS arm64/x64, Linux glibc arm64/x64, and Windows x64. Codex artifact cleanup currently targets macOS layouts. For unsupported record formats, see [format recovery](docs/reference/cli.en.md).

## Update

Rerun the installation command above to update Wombat and its Codex plugin. Keep any options for a specific version or custom installation directory.

See the [installation guide](docs/guides/installation.en.md) for update options and runtime-only updates.

## Help and contribution

See the [CLI guide](docs/guides/cli.en.md) for direct queries, JSON output and budget controls. See [Contributing](.github/CONTRIBUTING.md) for source setup.

[Report a problem or request](https://github.com/YannByte/wombat/issues). Describe what happened and the tools involved; no private logs are needed. Use the [security policy](.github/SECURITY.md) for security reports.

## License

[MIT](LICENSE). See [third-party notices](licenses/THIRD_PARTY_NOTICES.md) for dependency licenses.
