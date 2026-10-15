# Decision Note: Rust live queries and failure isolation

[中文](2026-10-01-rust-live-query.md) | English

Status: implemented

The resident-indexing lifecycle partially supersedes the idle-exit choice; see the [resident-indexing proposal](../../proposed/architecture/2026-10-15-resident-indexing.en.md). Source isolation and immutable read semantics remain applicable.

## Problem

A read error in one source prevented healthy sources in the batch from committing. Restoring a large cached source held the shared query lock and delayed other ready sources. Repeated queries recomputed summaries, and direct-response appends copied candidate maps and ran general reconciliation.

## Decision

Each source uses a SQLite savepoint. A read failure rolls back that source and discards its working memory cache, retaining its last successful projection. Healthy sources continue committing; responses expose partial and sourceSyncFailed, including after restart. If all sources fail, the request still errors and retains the prior version. Cursors, projections and the view version still commit together in the outer transaction.

Cached restoration runs outside the shared entries lock, using a read transaction to pin the SQLite view. After reacquiring the lock, it publishes only if no view exists, avoiding overwriting concurrent sync results.

Each immutable live view owns an LRU query cache with at most 16 entries, a 2 MiB total encoded budget and a 256 KiB entry limit; this budget is not an exact heap-memory limit. Budget counting neither allocates a full encoded copy nor holds the cache lock. Keys include the full request and the date in its timezone so default date ranges expire across midnight. Freshness is attached separately when returning. Saved file snapshots are not cached.

For direct-response facts without identity migrations, reuse fact references and skip cumulative reconciliation. Legacy counters, mixed records and identity migrations retain the general path. Priced rows are reused by merging ordered identities; distribution queries skip unused model details, and live turn queries borrow measurement references.

## Alternatives considered

Database aggregation and persistent MVCC could further reduce full-history rebuilding, but involve identity migration, legacy-counter retractions and snapshot compatibility. This change first fixes reproducible blocking and redundant work while retaining current semantics; the full database design remains undelivered.

## Impact and verification

Public DTOs are unchanged. Regressions cover healthy-source progress, failed-source retention, all-source failure, restart and recovery; cache bounds, response ownership and revision isolation; existing adapter fixtures compare full scans, increments, restarts, corrections and retractions. Performance uses the same release synthetic corpus and records latency, peak memory and conservation; see verification records retained in Git history.

Appends still traverse some full-history safe facts and rebuild memory indices; uncached filters still aggregate. This is not constant time or memory. Roots share one sync worker; this change removes cached restoration blocking ready queries. macOS verification does not establish Windows/Linux acceptance.

## Local transport tradeoff

Consolidated from the retired npm distribution note: the shared service uses private Unix sockets and Windows named pipes rather than becoming an HTTP server. Loopback TCP adds port-discovery and authentication boundaries. Platform IPC can restrict access to the same user but still needs identity, lock, timeout, and cleanup checks. Windows pipes reject remote clients and use a current-user DACL; lock waits must not bypass connection deadlines or nonblocking waits. Startup locking, synchronization work, fixed-view caching, and idle exit share one service lifecycle. Successful installation or compilation does not replace Windows multiprocess and failure validation.
