---
name: wombat-verify
description: Select verification scope and report evidence for Wombat cross-module changes, regressions, and release preparation; use when test and acceptance coverage needs judgment.
---

# Wombat Change Verification

Map the actual diff to owners and observable failure paths. [Root completion requirements](../../../AGENTS.md) remain authoritative.

Use `corepack pnpm skills:verify -- --scope <scope>` as the primary runner:

| Scope | Checks |
|---|---|
| `repository` | Source repository rules and whitespace; no build prerequisite |
| `client` | Build, types, and whitespace; add relevant focused behavior tests separately |
| `full` | Build, types, complete product tests, repository rules, and whitespace |

The runner stops on the first failure and reports completed stages. It resolves its repository from its own location rather than the caller's directory. Do not reload script source for ordinary execution. Rust-only, dependency, performance, and platform-specific work still needs the focused checks in the owning workflow; these scopes do not replace them.

Use independent synthetic truth for accounting and isolated valid/invalid fixtures for changed checks. Verify assembled product entries, browser interactions, and installed assets separately; mocks alone do not prove delivery.

Do not repeat unaffected passing checks. Follow the [failure investigation rules](../../../docs/development/workflow.en.md#failure-investigation): establish the root cause, inspect every matching entry, batch repairs, and verify the failure class before aggregate or platform retries; report commands actually run, results, build/corpus/platform conditions, and remaining gaps. A successful script does not establish a complete product journey.

After all build-dependent checks finish, apply [artifact cleanup](references/cleanup.md). The verification runner executes checks; its success does not prove cleanup. Keep downstream packaging inputs until used, and report removed paths and retained exceptions when finishing or stopping.
