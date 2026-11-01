# Development and delivery rules

[中文](workflow.md) | English

Module responsibilities belong in [architecture](architecture.en.md), product fields in [contracts](contracts.en.md), and repository constraints in [AGENTS.md](../../AGENTS.md).

## Code conventions

- Deliver new capabilities through CLI and generated non-TTY interfaces. Check all consumers and cancellation, error, and state semantics when changing public DTOs.
- Validate untrusted data at configuration, file, process, and network entries; do not repeatedly validate typed same-process values. Keep TypeScript strict. Explain why narrowing is infeasible for new any or assertions; never use double assertions to bypass validation.
- Exhaust closed unions by their discriminants; open source values need an explicit unknown branch. Resolve defaults once at the owning entry and report invalid configuration at the earliest reliable point.
- Enforce authorization and version checks in the operation itself, not solely through disabled UI or wrapper filters. Verify that direct and alternate callers cannot bypass them.
- Give each asynchronous operation one lifecycle controller or transaction. Additional state needs an independent responsibility; settle success, failure, and cancellation. Cleanup waits boundedly for child work to stop, and late results cannot update expired views.
- Publish state and notifications only after durable commit succeeds; preserve committed facts on failure. Derive caches and presentation from the same authoritative result.
- Apply resource limits where the complete emitted or retained value is known, including envelopes, metadata, and multibyte encoding. Cover tiny limits, exact boundaries, and oversized single chunks.
- Limit catch blocks to the expected failing operation. Explain ignored errors and preserve observable failure. Callback exceptions must not break unrelated requests or cleanup.
- Comments describe caller-relevant behavior, failures, timing, and ownership. Link decision rationale instead of narrating code or review. Update the owning documentation with the change; local edits need no new decision.


## Verification

Run `corepack pnpm repo:check` for source rules without dist; CI runs it independently of platform builds. Follow [scripts/AGENTS.md](../../scripts/AGENTS.md) for check scripts and [wombat-verify](../../.agents/skills/wombat-verify/SKILL.md) for scope selection. Code review and relevant behavior tests verify the semantic conventions above; static checks do not claim complete coverage.

```sh
corepack pnpm build
corepack pnpm typecheck
corepack pnpm contracts:check
corepack pnpm test
~/.cargo/bin/cargo fmt --manifest-path core/Cargo.toml -- --check
~/.cargo/bin/cargo clippy --locked --manifest-path core/Cargo.toml --all-targets -- -D warnings
```

Select commands by changed scope; run all for full-chain changes. Cross-language tests use dist, so build first. Do not repeat unaffected passing checks. Product behavior changes require observable results through a real assembled entry; manually composed mocks alone cannot establish delivery. External services and nondeterministic inputs may be replaced. Accounting uses [independent truth](adapters.en.md), not old output as its only oracle.

Verify CLI, real core and installed assets separately. Check cancellation, failures and recovery through the assembled entry. Performance uses fixed fixtures and release builds, recording cache state, startup, elapsed time, peak memory, and result consistency separately; benchmark entry points are the package.json benchmark scripts.

For dependency changes run `corepack pnpm licenses:generate` and `corepack pnpm licenses:check`. Verify target platforms and clean installation under the [release Skill](../../.agents/skills/wombat-release/SKILL.md). Report actual verification only; builds do not establish product or platform acceptance.

CI execution, caching and timing analysis are described in [CI](ci.en.md).

### Failure investigation

- Run `corepack pnpm test:repo` and `corepack pnpm test:prebuild` for source-only script tests and platform assumptions. CI runs these checks once on Linux, macOS, and Windows, alongside the five native builds. The local release gate runs source checks before Rust compilation. These checks do not replace product, browser, or installation acceptance.
- Preserve source timestamp precision in test arithmetic. Test process ownership with an instance identity; PIDs and ports can be reused. Use a stable failure code when either a probe or its enclosing deadline can expire first. An unconfirmed probe must not establish successful cleanup.
- Treat process startup, data readiness, and acceptance as separate states. A first read can return a syncing preview. Tests that require final counts must request a completed read and bind subsequent queries to its version. Do not use arbitrary sleeps to establish readiness.
- Stop retries and preserve the failing stage, logs, source revision, and environment. Trace the root cause. Check upstream documentation for unfamiliar behavior; reruns, parameter changes, and longer timeouts cannot replace analysis.
- Inspect callers, platforms, build prerequisites, and CI/release workflows that share the assumption. Batch the fixes and regression coverage before verification.
- Reproduce the whole failure class first, then run aggregate checks on settled changes before costly platform or release tasks. Reuse only passing evidence that remains valid.
