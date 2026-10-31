# Decision Note: Simple local Codex handoff and independent account reads

[中文](2026-10-03-native-codex-handoff.md) | English

Status: implemented

The current simplification supersedes task sending, alternate installation modes, automatic review history and adoption tracking described here. Other evidence constraints and unfinished items remain. See [read-only checks and one plugin](2026-10-31-on-demand-checks.en.md).

## Problem

Wombat must hand deterministic recommendations to local Codex. The user explicitly considers duplicate sends low-impact, requests a simple integration and uses rechecks to establish resolution. Recreating proposals, execution, recovery and persistent task deduplication would form a second execution system; account facts cannot be inferred from project usage or fixed allowance periods.

## Decision

Rust selects all eligible pending items in the current authorized scope, merges shared physical files and same-file rules, and builds a version-bound selection per project cwd. CLI/Web preview before confirming send and revalidate files/evidence before delivery. Node calls only fixed native methods: start the Codex-owned background host, create durable tasks and enqueue requests, without developer-chat coordination tools. Acceptance and actual resolution are separate.

Duplicate clicks are blocked only while sending, without historical task traversal, saved task associations, cross-process locks or Wombat execution receipts. Native message IDs are generated internally and are not product arguments. Unknown delivery requires inspection in Codex; users may confirm again and resend, potentially creating another task. Disconnecting Wombat cleans up only its own connection and does not cancel accepted Codex tasks.

Independent native reads provide account identity, allowances and activity for Rust normalization, without credential reads or copies. Only current multi-bucket fields and actual windows are used, without legacy single-bucket fallback or fixed five-hour/seven-day periods. Sections retain their own read times; failures retain stale facts and account changes discard old information. Unsafe integers and invalid resets remain unknown, without inferring ordinary-use limits or allowance restoration.

Balances, spending limits and reset credits retain their native units and states; reset credits are read-only. Invalid fields preserve valid siblings as partial. Counts, missing details and empty lists differ, and truncated details never replace the native count. Overview and the account dialog share observations; language, project and date changes do not independently request allowance. Fields and limits are in the [account contract](../../../development/contracts.en.md).

Allowance assessment uses an explicitly restricted request model, without guessing bucket applicability from normalModelSlug presentation metadata or percentages. The upstream [protocol definition](https://github.com/openai/codex/blob/main/codex-rs/protocol/src/protocol.rs) and [native notice handling](https://github.com/openai/codex/blob/main/codex-rs/tui/src/chatwidget/backend_banners.rs) distinguish presentation models from blocked_model_slug. Preview reads have time/count bounds and do not prevent local review on failure. Sending re-reads against the actual task model and revalidates files. The 60-second observation deadline bounds freshness, not recovery; see the [contract](../../../development/contracts.en.md). A final restriction may leave an empty task, avoiding a second task-deletion/restoration system in Wombat.

## Alternatives Considered

Persistent task associations, historical traversal and cross-process locks were investigated for deduplication, then removed after the user confirmed limited benefit. Wombat-owned proposal/application/restoration is outside current responsibility. A short-lived child process cannot reliably own a task after the page closes, so Codex's durable host is used.

## Impact and Verification

The native interface is verified on local macOS arm64 with Codex0.160.0. Other versions/platforms need separate verification, and interface failures are explicit. Web limits targets to startup grants; the portable client exposes no arbitrary RPC. Processes have cancellation, timeouts, output bounds and cleanup. A pinned WebSocket dependency and license materials are added; ordinary usage loads native modules on demand.

An isolated synthetic project verifies native acceptance and task retention after disconnect. Synthetic HTTP checks all-pending selection, merged shared targets, invalidated cross-project selections and changed-content rejection. Account fault tests cover old read times, account-switch clearing and credential-free output; native protocol tests cover split UTF-8, malformed responses, sanitized rejections, timeouts, cancellation and disconnects. Full-upgrade/platform limits remain in [unfinished proposal](../../proposed/product/2026-10-03-optimization-lifecycle.en.md); actual evidence is in verification records retained in Git history.
