<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="assets/wombat-logo-dark.svg">
    <img src="assets/wombat-logo-light.svg" alt="Wombat" width="80" height="56">
  </picture>
</p>

# Wombat — Review Your Codex Work and Usage

English | [中文](README.zh-CN.md)

See what you worked on, where your Tokens went, and what you could improve.

Wombat calculates usage from local Codex records and locates earlier conversations. Its Skills help Codex read selected original text, review daily work, investigate a task, and check configuration.

## Get started

**Stable: [`v0.3.7`](https://github.com/YannByte/wombat/releases/tag/v0.3.7).**

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

> $wombat:wombat-daily-review Review this project's work and Token usage today. Suggest one concrete improvement with original evidence.

The review combines recorded usage with selected original evidence. It separates requests, recorded work, and verified delivery, and explains gaps in the evidence.

## Choose a task

| Task | Skill | Example request |
|---|---|---|
| Review daily or weekly work and usage | `$wombat:wombat-daily-review` | What did I work on this week? Where did usage grow, and what could I improve? |
| Investigate a selected task | `$wombat:wombat-task-retro` | Review this task for repeated work, failures, and delays. Show the original evidence. |
| Review project configuration | `$wombat:wombat-config-review` | Check this project's AGENTS.md, Skills, MCP, and Hooks. Point to specific files and suggest changes. |

For direct usage, cost, timing, allowance, or Token budget questions, use `$wombat:wombat`. For example: “How many Tokens did this project use today?”

Reviews start read-only. Codex applies changes only when authorized. After a configuration change, ask to recheck the original finding. A report request does not schedule future reports.

Answers follow your language. See the [plugin guide](docs/guides/plugin.en.md) for collection plugins and other invocation names, or the [CLI guide](docs/guides/cli.en.md) for continuous budget checks.

## First preparation and results

The installer enables background indexing and automatic startup for the current user. Initial preparation prioritizes the most recent 30 days. Older requests can require additional preparation.

Installation success does not mean the data is ready. Initial results can be incomplete. Check preparation with:

```sh
wombat service status --json
```

Keep missing or partial data separate from zero usage. Reviews cannot prove waste from high usage alone, or configuration inactivity from missing use records.

If the plugin stage fails, Wombat and its data remain installed. Follow the reported recovery step and the [installation guide](docs/guides/installation.en.md). The guide also covers service controls. For Web-only use, omit `--plugin` or `-Plugin` when installing.

## Open Web

```sh
wombat web --open
```

If the browser does not open, use the full URL printed in the terminal. See the [Web guide](docs/guides/web.en.md) for pages and data recovery.

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
