# Decision Note: foreground queries without public views

[中文](2026-11-02-foreground-queries.md) | English

Status: implemented

## Problem

Retained read views introduced expiry, recovery and coordination work between otherwise bounded tool calls. An optional resident service also required endpoint discovery, startup registration, instance checks and platform-specific lifecycle controls. Keeping this machinery obscured source failures and made simple queries depend on process history.

## Decision

Run core operations in owned foreground child processes and retain SQLite only as an internal incremental index. Reuse one read and inventory for a project report. Close transactions and temporary state with each request. Remove public view selectors, saved-view operations, service controls and installation options without compatibility aliases.

Pass original selected findings and cleanup plans back as data for rechecks. Compare the original scope and rules with current evidence; do not retain a report lifecycle or treat a supplied plan as current deletion permission. Preserve selected identities, file checks and explicit decisions. Opaque cursors bind source targets and positions, not a process lease. Changed sources can invalidate a cursor; unrelated revision changes must not do so.

Explain configuration use at the point where the existing projection accepts or excludes an operation. Borrow the separation between reducer input, result and recomputation from [Redux DevTools instrumentation](https://github.com/reduxjs/redux-devtools/blob/main/packages/redux-devtools-instrument/src/instrument.ts), while retaining only requested record pages and streaming fingerprints. A second explanation engine could disagree with the count; a full event/state journal would duplicate source records and restore retention machinery. Supplied explanation baselines support same-input comparisons without executing historical operations or retaining old rule implementations. Adapter-verified native command targets are execution evidence; parsed display labels alone remain candidates. Existing accounting and pricing remain independent. Discover requested configuration categories before analysis, preserving metadata for unrequested categories. This prevents unrelated project traversal from starving known Skill locations and removes repeated whole-ledger copies and per-operation turn filtering.

Accounting, pricing rules and original source files remain unchanged. Existing hot/cold indexes, canonical source identities, source progress and compressed evidence stay in place. SQLite transactions preserve atomic incremental writes and consistent request reads. Replacing SQLite with ad hoc files would duplicate these correctness requirements without helping the user task.

This supersedes the service/view lifecycle in the earlier live-query and demand-indexing decisions, and rejects the resident-indexing direction. Their independent accounting, bounded-read and storage rationale remains applicable. Performance targets are not acceptance gates for this change. Current interfaces belong in the CLI guide and core reference.

## Alternatives considered

Keeping views internally behind automatic recovery would preserve expiry and coordination paths. Keeping an optional service would retain two execution modes and platform management. Removing SQLite would require a separate atomic publication, concurrency and query system. All three add work beyond the chosen foreground tool model.

## Consequences and verification

First use and history expansion can wait for collection. Removing the service does not make large queries free; no latency or resource ceiling is claimed. Concurrent indexed requests wait on a file lock before loading facts. Only explicitly driven foreground monitoring loops continue between checks.

The local darwin-arm64 product suite and staged runtime checks cover generated interfaces, independent accounting truth, scoped continuation after unrelated changes, stateless diagnosis and cleanup rechecks, protected deletion, and cancellation cleanup. The same real weekly totals and three conversation tasks were checked against the prior baseline. User decisions and original sources remain separate from temporary test indexes. Other operating systems and a permanent local upgrade were not qualified by this run.
