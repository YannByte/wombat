# Decision Note: bounded compressed live-index facts

English | [中文](2026-10-11-packed-live-index.md)

Status: implemented

## Problem

The previous row layout stores repeated observation envelopes and one projection reference per event. Parser measurement and operation copies are redundant for recovery: replay already reconstructs them from observations. Removing these copies reduces persistent data without changing accounting. Compressing isolated rows cannot exploit their repeated source, task, and schema fields.

## Decision

Retain SQLite transactions and pack related facts with the pinned Zstandard library. Keep safe observations sufficient for offline replay; remove redundant parser projections and per-event reference rows. The [core reference](../../../reference/core.en.md) owns the current format, bounds, and recovery behavior. This partially supersedes the row/reference layout and page-cache choice in the [compact-index decision](2026-10-02-compact-live-index.en.md); its identity, exact-value sharing, current-format-only, and atomic-publication rationale still applies.

Source-local windows bound append rewrites. Canonical query facts use separate bounded frames so queries and restoration do not need to decode observation blocks to obtain measurements. Small appends reuse the current fact frame. Checksums, decoded-size limits, explicit codec versions, and identity checks protect reads. All block, membership, checkpoint, and view writes use the existing transaction boundary. User decisions and collection history remain in their independent stores.

Retain lossless safe evidence rather than switch to source-only locators in this change. Source removal must still allow existing accounting and analysis to recover. A source-only profile requires a distinct availability contract and verified source generations. No conversation or tool-output body is added to the index.

Prepare bounded exact summary groups with each immutable live view. Queries fold only fully selected groups and retain row calculation for partial selections. This reduces first-request aggregation work without relying on a previously cached response; corrections and price changes produce a new aggregate owner. The core reference owns admission limits and fallback behavior. Persistent aggregate tables remain outside this change.

Unix service connections wait for socket readiness instead of periodic polling. This removes an avoidable foreground delay while reducing idle wakeups. It does not alter query freshness, synchronization, or cancellation contracts.

## Alternatives considered

The [storage experiment](../../../benchmarks/storage-layout-2026-10-11.json) compared SQLite and DuckDB with compressed safe observations. DuckDB improved broad fact scans in that experiment, but used more total space and introduced a second engine. SQLite reuses the existing transaction and deployment boundary. Keeping only aggregates would lose exact corrections and supported evidence. Source-only storage has different offline guarantees. The [live-usage proposal](../../proposed/architecture/2026-09-30-live-usage.en.md) retains unfinished aggregate, residency, and scale acceptance; compressed storage alone does not establish those claims.

## Consequences and verification

The storage format changes independently of public DTOs and source mappings. Unknown or older layouts are rejected and retained; this change does not migrate or delete them. Block compression adds CPU work and native dependency packaging. A correction rewrites its frame, while unchanged frames keep their bytes. SQLite WAL and temporary-space peaks remain separate from settled database size.

Rust regression tests cover exact numbers and nulls, small append packing, corrections, retractions, missing or corrupt frames, read-only selection, replay after source removal, restart, and transactional rollback. The [production benchmark](../../../benchmarks/production-storage-2026-10-11.json) checks independent tokens and money, task and daily totals, snapshot export, restart, and rebuild through real CLI entries. Platform, corpus, cache, and memory limits belong to each benchmark result. Full-history parser and snapshot residency still require the follow-up design in the proposal.
