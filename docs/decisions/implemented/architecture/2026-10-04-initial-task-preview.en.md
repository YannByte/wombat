# Decision note: provisional tasks during the first scan

[中文](2026-10-04-initial-task-preview.md) | English

Status: implemented

The service and retained-view lifecycle described below is superseded by the [foreground-query decision](2026-11-02-foreground-queries.en.md). Its independent source, accounting and bounded-evidence rationale remains historical context.

## Problem

Tasks and aggregates were unavailable until the first full scan and index transaction finished. Better waiting text alone cannot make some tasks available earlier; persisting provisional results as a completed index would misrepresent unknown coverage as fully read.

## Decision

Without a committed view, the Codex adapter reads complete first session_meta records, retaining only native identity, directory, time and source version. The shared budget is 64 files, 128 directories, 2,048 directory entries, depth 64 and 128 KiB per file. A 100-millisecond deadline is checked between directory operations; it is not a hard timeout for blocking filesystem calls. Directory links are not followed. Oversized, unfinished or unrecognized first records are omitted from the preview; the full scan still processes every valid record.

The preview lives only in memory, with partial source status and initialScanIncomplete. Shared response freshness.initialScan distinguishes provisional task metadata. Task usage remains unknown, and neither recent activity nor high-usage rankings are claimed complete. Automatic queries give fast scans up to 250 milliseconds to return completed results directly; slower scans can return a preview first. Unpinned cached queries read committed views only. fresh and refresh still require completed synchronization, and refresh does not accept a specified snapshot.

The full scan keeps its transaction, failure isolation and ledger reconciliation. Preview data never seeds persistent caches, and no preview database is added. When Web opens a provisional task, it releases the provisional revision pin while preserving filters and task identity, then fills in results automatically on completion. Ordinary fixed revisions do not advance automatically. Cancellation stops only page waiting and polling, leaving the scan shared with other entries running.

## Alternatives considered

Waiting for the full transaction cannot deliver tasks early. Repeatedly copying and publishing all facts after each file would increase memory and aggregation costs during the first scan. One bounded metadata pass costs up to 8 MiB of extra first-record reads. Sources without recognizable first records still wait for the authoritative scan.

## Impact and verification

Rust generates the cross-language field; CLI and Web share provisional results and partial semantics. Synthetic tests cover identity consistency, discarded body content, oversized and unfinished headers, shared budgets, directory conflicts, and cached/export rejection. Web state tests cover completion while a task is open. Browser journeys and measured resource limits are recorded in verification records retained in Git history. This design supplies neither a percentage nor an estimated completion time, does not solve memory costs from retaining all facts, and does not establish complete startup, all-source or other-platform acceptance.
