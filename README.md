<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="assets/wombat-logo-dark.svg">
    <img src="assets/wombat-logo-light.svg" alt="Wombat" width="80" height="56">
  </picture>
</p>

# Wombat: Codex history, usage and project diagnosis

English | [中文](README.zh-CN.md)

Wombat is a plugin and local CLI for OpenAI Codex. Search conversation history, analyze Token usage, estimate API-equivalent costs, diagnose project problems, and review files for safe cleanup.

Ask in natural language to recover past decisions, resume work or investigate recurring failures. Wombat retrieves local evidence and calculates usage; Codex explains the findings, makes authorized changes and rechecks the original problem.

## See the result

### Check Codex Token usage and estimated costs

> **You:** How much did I use today, and where did it go?

**Example answer:** 1.2 million Tokens: 900,000 for installation troubleshooting, 200,000 for test fixes and 100,000 for documentation. One model is unpriced, so the cost estimate is incomplete.

### Diagnose project problems and verify improvements

> **You:** Review this project's publishing process. What should I improve?

**Example answer:** `scripts/ship.ts` skips an existing package check before upload. Call that check before uploading. After the change, run the same publishing path with a valid archive and an archive missing a required file: the first must pass; the second must stop before upload.

### Preview and clean up Codex files

> **You:** Show which Codex files can be cleaned. Do not delete anything yet.

**Example preview:**

- **Candidate:** `releases/1.0.0-darwin-arm64`, observed size 240 MB; neither the current nor pending release.
- **Keep:** Both the current and pending releases, plus desktop caches while Codex is running.
- **Action:** Nothing has been deleted. Removing the selected old release would permanently delete it.

> **You:** Permanently delete only that old release and recheck. Keep everything else.

**Example follow-up:** The selected old release was removed; recheck confirms it is absent at its original path. The current release, pending release and desktop caches remain.

## Install Wombat for Codex

**Stable: [`v0.5.0`](https://github.com/YannByte/wombat/releases/tag/v0.5.0).**

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
3. Ask:

> $wombat:wombat Show today's usage and the three tasks that used the most Tokens. Tell me whether the data is complete.

The answer uses the language of your question and includes the time range, total usage, top tasks and any missing data. For installation trouble, follow the reported recovery step; see the [installation guide](docs/guides/installation.en.md).

## More ways to use it

Ask with `$wombat:wombat`, for example:

| Task | Example request |
|---|---|
| Find a decision | Why did we choose this installation approach last time? Find the discussion and show its location in the original conversation. |
| Resume work | Where did we leave this project yesterday? Find the last confirmed results, unfinished work and next step. |
| Review the week | What did I work on in this project this week? Separate verified results from work that still needs checking. |
| Compare usage | How did this project's usage change from last week to this week? Which tasks and models account for the main differences? |
| Check estimated costs | What would this week's usage cost at API prices? Which models are still unpriced? |
| Investigate a recurring problem | This installation problem keeps coming back. Use related records and current code to find what remains unresolved and suggest a change. |
| Verify a change | Recheck the original installation failure after this fix. What passed, and what remains unverified? |
| Free up space | How much space do old Codex versions and caches use, and which can be cleaned? Do not delete anything yet. |

Use these examples as a starting point and explore questions from your own work. Add a project, time range or specific problem to narrow the scope. You can follow up with “Show the original conversation,” “Suggest the smallest change,” or “Recheck the original failure after the change.”

See the [plugin guide](docs/guides/plugin.en.md) for more uses, or the [CLI guide](docs/guides/cli.en.md) for direct queries.

## Common questions

### How do I search Codex conversation history?

Ask `$wombat:wombat` for a past discussion, decision or unfinished task. Add the project, date range or words you remember. Wombat locates relevant records and returns original conversation locations.

### Does Wombat show my subscription bill?

No. Wombat calculates Token usage and API-equivalent cost estimates from local Codex records. Missing prices are not zero cost; subscription allowance is separate. See [pricing](docs/reference/pricing.en.md).

### Can I use Wombat without the plugin?

Yes. Omit `--plugin` or `-Plugin` when installing and use the CLI directly. CLI queries support JSON output; see the [CLI guide](docs/guides/cli.en.md).

## Before you use it

- CLI analysis stays local and makes no model calls. Selected Skill evidence enters the current Codex conversation and consumes Tokens. Missing prices may trigger an official catalog download; see [privacy](docs/reference/privacy.en.md).
- Cleanup needs authorization for the selected items. It protects conversations, state, configuration and credentials; changed or unknown conditions block deletion. See the [cleanup guide](docs/guides/cli.en.md).

Release packages support macOS arm64/x64, Linux glibc arm64/x64, and Windows x64. Codex artifact cleanup currently targets macOS layouts. For unsupported record formats, see [format recovery](docs/reference/cli.en.md).

## Update

Rerun the installation command above to update Wombat and its Codex plugin. Keep any options for a specific version or custom installation directory.

See the [installation guide](docs/guides/installation.en.md) for update options and runtime-only updates.

## Help and contribution

See the [CLI guide](docs/guides/cli.en.md) for direct queries, JSON output and budget controls. See [Contributing](.github/CONTRIBUTING.md) for source setup.

[Report a problem or request](https://github.com/YannByte/wombat/issues). Describe what happened and the tools involved; no private logs are needed. Use the [security policy](.github/SECURITY.md) for security reports.

## License

[MIT](LICENSE). See [third-party notices](licenses/THIRD_PARTY_NOTICES.md) for dependency licenses.
