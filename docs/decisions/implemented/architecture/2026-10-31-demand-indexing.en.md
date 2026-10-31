# Decision: demand-driven indexing

[中文](2026-10-31-demand-indexing.md) | English

Status: implemented

## Problem

The [resident-indexing proposal](../../proposed/architecture/2026-10-15-resident-indexing.en.md) moves preparation out of the first query but also performs work without a consumer. Startup restoration, source notifications and periodic discovery can allocate memory and update indexes that nobody reads. A visible Web page can perpetuate that work through polling.

## Decision

Separate process lifetime from permission to compute. Only a validated query can enqueue source work; service startup and controls do not open the index or restore views. Notifications invalidate cached views without scheduling synchronization. The next eligible query consumes invalidation or the elapsed revalidation interval. Explicit monitoring retains its authorized loop. Installation no longer opts users into resident startup. The [CLI and task-Skill decision](2026-10-31-cli-task-skill.en.md) removes Web delivery and its polling.

Retain existing canonical facts, durable incremental recovery and bounded query caches. They avoid replaying source history for every request and preserve accounting, dependency-family and fixed-revision rules. Both process lifetimes retire transient scan allocations before retaining compact headers. This partially supersedes automatic startup preparation, notification-driven increments and idle discovery in the resident proposal; its durable recovery, resource-reclamation rationale and outstanding native-platform and scale qualification remain applicable. Current behavior belongs in the [core reference](../../../reference/core.en.md).

The reference [Better Harness Codex analyzer at b605532](https://github.com/QoderAI/better-harness/blob/b605532fcf1014bcb48d3e6940a5bc64e80cddd5/scripts/session-analysis/platforms/codex.mjs) discovers candidates, selects sessions and then hydrates them. Its Skill shares a frozen evidence bundle across analyses. This supports scope-first reads and revision reuse, not a requirement to remove durable accounting facts. Its `all-eligible` selection returns the entire eligible population regardless of the supplied limit, and selected-session hydration retains event arrays. Neither a small output nor streaming input establishes bounded computation or memory. Wombat preserves exact aggregate populations and existing bounded detail reads rather than using semantic samples as usage totals.

## Alternatives considered

Delaying the same broad scan until a process starts preserves unnecessary startup work. Removing durable facts entirely makes repeated queries replay logs and weakens revision reuse. A single demand-owned writer retains the existing accounting and storage model while removing independent scheduling paths.

## Consequences and verification

First requests can wait for preparation; on-demand execution does not make full-history collection free or establish a hard latency or memory bound. Native notifications are invalidation evidence rather than proof of a captured boundary. A completed fresh ticket can return a stale view when later invalidation is pending, without a false timeout or discarded notification. Cached and fixed reads never authorize synchronization. Read-only idle cache reclamation and SQLite checkpoints remain maintenance, separate from source work.

Selection tests cover coalescing, cancellation, invalidation and delayed notifications. The real-core resident scenario covers quiet startup, no-reader changes, explicit queries, pause/resume, incomplete tails, replacement, restart, scope isolation and fixed reads. Installer tests cover default inactivity and explicit residency. Web and UI checks are retired under the CLI and task-Skill decision. Complete-request latency, real-source resources, long-duration scale and native platform acceptance retain their existing owning checks rather than being inferred from these tests.
