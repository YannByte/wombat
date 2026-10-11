# Decision Note: Immutable snapshots and on-demand detail reads

[中文](2026-09-30-snapshot-storage.md) | English

Status: implemented

The prior compatibility decision is superseded by [current formats before the first release](2026-10-03-current-format-only.en.md).

Extracted from the version-one specification on 2026-09-30, covering explicitly saved v3 snapshots. The current SQLite live index coexists with snapshots; its full extension goals remain in the [live usage proposal](../../proposed/architecture/2026-09-30-live-usage.en.md).

## Problem

A single JSON document containing all history makes small detail queries read large amounts of unrelated data. Failed refreshes must preserve previous results, and pagination and drill-down need fixed versions, totals, and share denominators. Old snapshots must retain their original identity, pricing policy, and missing-value semantics.

## Decision

- Use immutable generations: a small manifest, compact measurement ledger, conversation JSONL shards, and turn offsets/hashes. Usage reads the ledger, turns read the target conversation, and steps read and verify only the relevant fragment.
- Refresh holds a process file lock. It writes all files and hashes in a private temporary generation, commits the manifest, then atomically updates latest. Cancellation, failure, and limit violations do not publish partial snapshots; when all sources are unreadable, old latest remains intact.
- Queries fix snapshotId, filter, sort, and aggregate the full scope before pagination. Share denominators are not computed from returned pages. Facts without a turn association remain unassigned records.
- Support only the current snapshot format. Original logs and recovery materials are outside index cleanup scope.
- Store only allowlisted metadata, excluding messages, model bodies, full command arguments, and tool output. Source files are bounded for each read; reading multiple files is not a source-level atomic transaction.

## Alternatives considered

| Option | Tradeoff |
|---|---|
| Keep one large JSON file | Simple to implement, but detail queries and repeated parsing grow with all history, making on-demand expansion inefficient |
| Introduce a database and permanent service for all storage in version one | Adds deployment, recovery, and lifecycle costs; explicit snapshots first establish fixed results and an independent installation baseline |

Automatic synchronization subsequently introduced SQLite and an on-demand service, so the second choice is not a permanent ban on databases. The live index owns incremental state, while explicit snapshots own fixed persistence and repeatable queries. Their responsibilities are defined by the [current architecture](../../../development/architecture.en.md).

## Impact and verification

Shards, offsets, hashes, publication ordering, and current-format validation require maintenance. File counts increase, and pagination alone does not guarantee constant computation. Routine expansion p95 ≤300ms and cold queries ≤1s are targets, not achieved performance claims.

[Fixed-query evidence](../../../benchmarks/usage-v1-query-2026-09-30.json) covers 500 conversations, 5,000 turns, and 10,000 measurements, with 11,000,000 tokens / $28.35 conserved across layers and all pages. Measurements include process startup, I/O, serialization, and text rendering; system file caches were not cleared. Cancellation, corruption, limits, legacy identity, and policy regression coverage is recorded in verification records retained in Git history. Updates to this record must check `core/src/usage_store.rs` and storage/integration tests; historical timings do not verify the current build.
