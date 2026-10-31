# Contributing

English | [中文](CONTRIBUTING.zh-CN.md)

Wombat uses a shared Rust core with a Node.js CLI and one main Skill. Before you start, read [AGENTS.md](../AGENTS.md), [architecture](../docs/development/architecture.en.md), and the [development workflow](../docs/development/workflow.en.md).

Source builds require Node.js 26.4.0 or newer, Corepack, pnpm, and the Rust version in `rust-toolchain.toml`. Clone the repository. Install the locked dependencies and build the project. Then run type checks, generated contract checks, and tests. Cross-language tests use `dist` and must run after a build. For Rust changes, check formatting and run clippy with warnings treated as errors.

For documentation or Skill changes, update both versions of paired pages and run `corepack pnpm docs:check`. Follow the [language review rules](../docs/i18n/workflow.en.md) for ASD-STE100 technical writing and translation review.

```sh
corepack pnpm install --frozen-lockfile
corepack pnpm build
corepack pnpm typecheck
corepack pnpm contracts:check
corepack pnpm repo:check
corepack pnpm test
```

Keep business rules in Rust. Adapters normalize source facts; pricing and queries operate on Wombat types. CLI and automation must use the same contract. Do not add a ccusage build, runtime or test dependency. Independent synthetic expectations cover counting and money; snapshot regeneration is not an oracle.

Never commit real messages, tool output, credentials or unaudited raw fields. Preserve user changes and user-owned data. Read-only collection must not mutate source files. No arbitrary execution capabilities belong in rendering interfaces.

Dependency changes require license review and `licenses:generate` / `licenses:check`. Before packaging run `public:check`; this is a guard, not a publication or complete security audit. Record actual tested platforms and limits, not planned capabilities. See [third-party notices](../licenses/THIRD_PARTY_NOTICES.md).

## Review and automation

In bug reports, include a minimal synthetic reproduction, the version and platform, and the expected and observed behavior. In pull requests, explain the changes and the checks you ran. Do not attach private logs. Report suspected vulnerabilities as described in [Security](SECURITY.md).

CI builds and checks five native targets, exports the core and bundled Node.js runtime, and assembles five self-contained GitHub Release archives only when all native artifacts match. The final archive is exercised on every target without relying on a system Node runtime. Source tools use Node 26.4.0+. Each target exports its own dependency license inventory and Node runtime license; the checked-in dependency inventory is the macOS arm64 baseline. Other targets regenerate their inventory in CI before checking. CLI and installed assets require separate acceptance. Tag workflows publish only after the full matrix passes; ordinary CI never publishes or changes repository visibility.

Run `corepack pnpm audit --audit-level moderate` after dependency updates. The current pnpm audit may identify the local workspace directory `cli` as the unrelated npm package of that name; verify the lockfile path before treating that low-severity entry as a shipped dependency. This does not suppress actual advisories. Rust advisories are checked with cargo-audit 0.22.2 against `core/Cargo.lock`.
