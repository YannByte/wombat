<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="assets/wombat-logo-dark.svg">
    <img src="assets/wombat-logo-light.svg" alt="Wombat" width="80" height="56">
  </picture>
</p>

# Wombat — Codex Token Usage Analysis & Configuration Review

English | [中文](README.zh-CN.md)

Wombat analyzes local Codex records. Find the tasks behind token growth and inspect recorded timing. Review AGENTS.md, Skills, MCP, and Hooks with evidence from local records.

Use the Codex plugin to ask questions. Open the local Web dashboard for charts and detailed records.

## Get started

**Stable: [`v0.3.6`](https://github.com/YannByte/wombat/releases/tag/v0.3.6).**

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

> $wombat:wombat Where did this week's token usage increase? Find the main tasks and turns.

The answer identifies the main contributions and input, cache, and output changes. Ask to open matching Web details to inspect the records.

An existing collection plugin uses `$wombat-collection:wombat`. If multiple instances exist, select the discovered invocation for your project. See the [plugin guide](docs/guides/plugin.en.md).

If plugin installation fails, Wombat remains installed. Follow the [installation recovery steps](docs/guides/installation.en.md). For Web-only use, omit `--plugin` or `-Plugin` when installing.

## Main uses

- Compare task usage and period growth. Inspect the tasks and turns behind changes.
- Find recorded activity and earlier messages. Let Codex read selected original text for daily work reports and follow-up questions.
- Inspect recorded durations, activity intervals, overlaps, and coverage gaps.
- Find repeated operations across tasks for Codex to assess as possible scripts or Skills.
- Check project instructions and extensions. Review locations, evidence, and suggested actions.
- Save configuration review decisions. After authorized edits, rerun the same checks.
- Set daily, weekly, or monthly token budgets. Review the most recent closed period.

Budget checks require a running `wombat monitor watch` or periodic checks enabled in an open Web budget panel.

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
- Local analysis uploads no logs and makes no model calls. Missing prices can trigger an official price download; see [privacy](docs/reference/privacy.en.md).
- Tokens, API-equivalent estimates, and account allowance remain separate. Estimates are not subscription charges; see [pricing](docs/reference/pricing.en.md).

For unsupported data formats, see [format recovery](docs/reference/cli.en.md).

## Help and contribution

See the [CLI guide](docs/guides/cli.en.md) for JSON queries and budget controls. See [Contributing](.github/CONTRIBUTING.md) for source setup.

[Report a problem or request](https://github.com/YannByte/wombat/issues). Describe what happens and the tools involved; no private logs are needed. Use the [security policy](.github/SECURITY.md) for security reports.

## License

[MIT](LICENSE). See [third-party notices](licenses/THIRD_PARTY_NOTICES.md) for dependency licenses.
