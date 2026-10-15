# Proposal: resident local indexing

[中文](2026-10-15-resident-indexing.md) | English

Status: proposed

## Problem

The first real query waits for initial preparation. Idle service exits repeat startup and parser recovery. Keeping the existing process alive would also retain source-wide parser state and detail caches. Moving work into the background alone does not reduce its computation or memory.

## Proposal

Use one resident Rust service with the existing writer and indexing thread. Managed installation starts recent preparation; native user-session management owns login startup and bounded failure recovery. CLI, Codex and Web share committed views. File notifications trigger coalesced increments, with periodic discovery for missed events. Historical preparation stays explicit and retains complete declared dependency families.

Persist parser-specific recovery state in the same transaction as canonical facts and source progress. Reference stable canonical values where semantics permit; retain pre-reconciliation exceptions where references could change or disappear. Evict oversized source caches without erasing facts or forcing raw-event replay on every return. Heavy batches retire their thread-local heaps before the coordinator creates retained headers, separating transient and persistent allocation lifetimes. Compact committed headers serve queries; detail caches and expired view leases are disposable. Idle checkpoint rebasing must preserve exact revisions and source epochs. Apply background scheduling only to indexing, so query requests remain responsive. Release oversized parser state before allocating retained headers, then collect unused pages through the locked mimalloc allocator. Native allocator inspection distinguished a small live heap from a much larger retained scan footprint. Direct OS segments avoid large reusable arenas and allow empty segments to be unmapped, without adding process orchestration solely to reclaim freed pages. Collection and scheduling hints are not resource quotas.

This partially supersedes the idle-service lifecycle in the [live-query decision](../../implemented/architecture/2026-10-01-rust-live-query.en.md) and the installed-index-scheduler exclusion in the [optimization proposal](../../proposed/product/2026-10-03-optimization-lifecycle.en.md). Budget monitoring still requires its authorized host; resident indexing does not add account polling, model calls, notifications or automatic full-history loading. The [persistent-index proposal](2026-09-30-live-usage.en.md) remains the owner of tenfold storage reduction, bounded affected computation and complete-request latency acceptance. Current commands and limits belong in the [CLI guide](../../../guides/cli.en.md) and [core reference](../../../reference/core.en.md).

## Alternatives considered

An install-only warmup improves the first query but does not keep data fresh. Starting a new indexing process for each update discards reusable parser state and adds recovery work. Retaining the complete existing object graph avoids recovery but makes memory grow with history. A resident indexer with durable recovery and bounded caches addresses those costs together while keeping the modular monolith.

## Acceptance criteria

Verify clean-install preparation, indexing without clients, pause/resume, missed notifications, incomplete tails, replacement, truncation, late association, process failure, restart, upgrade and explicit removal. Confirm immutable old reads and independent accounting truth through CLI and Web. Test each native user-session manager on its own target; generated definitions alone do not establish installation acceptance.

Run a reproducible synthetic workload for at least 24 hours and record process memory, idle CPU, append cost, query latency, database and WAL sizes, cache state and source revision. Add real-machine first-install evidence after deleting historical Wombat versions and product state during preparation, while keeping original Codex logs read-only. Parser cache encoding budgets do not establish hard process-memory bounds; source-wide active reconciliation and integrity reads remain explicit scale limits until measured replacements satisfy the owning proposal.
