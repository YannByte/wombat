<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="assets/wombat-logo-dark.svg">
    <img src="assets/wombat-logo-light.svg" alt="Wombat" width="80" height="56">
  </picture>
</p>

# Wombat — Diagnose and Improve Coding-Agent Work

English | [中文](README.zh-CN.md)

Find what blocks a coding-agent task, make a concrete improvement, and check the original problem again.

Wombat combines efficient local work-history retrieval with exact usage calculations. One Skill selects the evidence needed for your task and helps Codex inspect, change and verify the project. Start with your question. No project initialization or full-history scan is required for a static engineering review.

## Get started

**Stable: [`v0.4.0`](https://github.com/YannByte/wombat/releases/tag/v0.4.0).**

Local analysis needs no API key or development tools. Install a compatible local Codex before using the plugin.

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
3. Ask:

> $wombat:wombat Investigate why this project's delivery check failed. Show the evidence, the exact change to make, and how to verify it.

## Choose a task

Use the same `$wombat:wombat` entry for every task:

| Task | Example request |
|---|---|
| Resume work | What did we decide about this task, and what should I do next? |
| Investigate an obstacle | Why did this check keep failing? Point to the actual project entry. |
| Verify a delivery | Does this result meet the original requirements? Which checks are missing? |
| Review work and usage | What did I work on this week, and which change would help most? |
| Improve the workflow | Connect the existing check to delivery, then verify that invalid input is blocked. |

Simple usage, cost and timing questions use the same entry. Reviews start read-only. After an authorized change, recheck the original issue. A static check, later adoption and improved outcomes are separate results. A report request does not schedule future reports.

Answers follow your language. See the [plugin guide](docs/guides/plugin.en.md) for collection or standalone invocation, or the [CLI guide](docs/guides/cli.en.md) for direct queries.

## First preparation and results

Installation does not start indexing or enable automatic startup. The first query prepares the data it needs and reuses the local index on later queries. Recent queries prioritize the most recent 30 days; older requests can require additional preparation.

Initial results can be incomplete while a requested query is preparing. Check its progress with:

```sh
wombat service status --json
```

Keep missing or partial data separate from zero usage. Reviews cannot prove waste from high usage alone, or configuration inactivity from missing use records.

If the plugin stage fails, Wombat and its data remain installed. Follow the reported recovery step and the [installation guide](docs/guides/installation.en.md). The guide also covers service controls. For CLI-only use, omit `--plugin` or `-Plugin` when installing.

## Update

Rerun the installation command above to update Wombat and its Codex plugin. Check for updates, or update only the Wombat runtime:

```sh
wombat update --check
wombat update
```

See the [installation guide](docs/guides/installation.en.md) for specific versions and custom installation directories.

## Scope and limits

- Analyzes local Codex records. Release packages support macOS arm64/x64, Linux glibc arm64/x64, and Windows x64.
- Wombat's local queries upload no logs and make no model calls. Using these Skills in Codex consumes model Tokens.
- Missing prices can trigger an official price download. See [privacy](docs/reference/privacy.en.md) for network and original-text access boundaries.
- Tokens, API-equivalent estimates, and account allowance remain separate. Estimates are not subscription charges; see [pricing](docs/reference/pricing.en.md).

For unsupported data formats, see [format recovery](docs/reference/cli.en.md).

## Help and contribution

See the [CLI guide](docs/guides/cli.en.md) for JSON queries and budget controls. See [Contributing](.github/CONTRIBUTING.md) for source setup.

[Report a problem or request](https://github.com/YannByte/wombat/issues). Describe what happens and the tools involved; no private logs are needed. Use the [security policy](.github/SECURITY.md) for security reports.

## License

[MIT](LICENSE). See [third-party notices](licenses/THIRD_PARTY_NOTICES.md) for dependency licenses.
