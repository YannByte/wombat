# Decision Note: compact live index and replacement algorithm

[中文](2026-10-02-compact-live-index.md) | English

Status: implemented

The prior compatibility decision is superseded by [current formats before the first release](2026-10-03-current-format-only.en.md).

The physical row/reference layout and page-cache choice below are historical and partially superseded by [bounded compressed facts](2026-10-11-packed-live-index.en.md). Other sharing and transaction rationale remains applicable.

## Problem

The old index repeated scope text and JSON-encoded field/identity keys for each safe fact. Both parser candidates and committed projections must be restorable, so repeated keys grew with record counts. Whole-field replacement allocated encoded desired keys and copied all old keys. WAL files from initial large transactions could retain their peak size through later small appends.

## Decision

SQLite in the current live-v2 directory independently uses user_version=4; the pre-event-upgrade live-v1/version 3 is retained and never mixed into current reads. A buckets dictionary uniquely maps(scope,field) to integers. Entries use a(bucket,id) composite primary key and WITHOUT ROWID, avoiding repeated scope/field text and JSON keys. SQLite itself encodes jsonb and decodes json; payloads remain opaque internal database BLOBs. Public DTOs, current snapshots and adapters manage their versions independently. JSONB saves space without claiming constant-time field access; see [SQLite documentation](https://www.sqlite.org/json1.html#jsonb).

Equal measurement/operation projections reference parser facts through source_bucket and a fixed member path, keeping one JSONB payload. Different cumulative reconciliation results retain their own payloads. The adapter verifies full value equality before writing, prepares field locations once, and confirms the source payload within each insert. Updates, deletions and cursors share one transaction. Reads resolve one reference within the same SQLite view and reject missing targets. Published in-memory revisions remain immutable. Only the current layout is maintained, without migration.

Field replacement uses a HashSet of borrowed identities, streams existing identities and copies only obsolete ones before deleting after the scan. Reads do not mutate the scanned table; unchanged payloads are not rewritten. Appends still write only dirty measurements, operations and cursors, without per-append VACUUM or whole-database rewrites.

Sharing immutable priced facts back into parser caches uses merge cursors over three identity-sorted sequences, checking shared pointers first and reusing only equal values. This replaces per-row BTreeMap lookups and projected-fact binary searches with a linear pass per source without allocating another collection. Corrections with different values, missing identities and other sources are not overwritten. Multiple sources still traverse the global measurement sequence separately; local appends are not claimed independent of full-history size.

Within one catalog, pricing construction remembers at most 4,096 input groups. Hash candidates are checked against full model, token-category and requestScoped values. Only immutable price results are shared: measurement identities and ledger contributions remain independent. Missing values, zero, conditions and amounts stay distinct. Inputs beyond the budget are priced normally, and the pool is released after construction. Persisted JSON is unchanged. Dirty-identity scratch sets are cleared after writing rather than retained between reads.

Repetitive internal measurement and operation fields, including source, thread/turn, model and status, use Arc<str>. Scanning and live-index restoration share only fully equal text without merging fact identities. Each pool retains at most4,096 entries and1MiB of text, excluding strings longer than1KiB, with additional bounded table overhead. Exhausting the budget preserves original values and still reuses existing entries, without truncating fields or dropping records. Parser pools live with their caches; temporary projection-restoration pools are released after reading. Already shared facts are not copied just to compact them. New revisions retain copy-on-write behavior without changing prior views. This optimization stores no bodies, adds no dependency and changes no JSON strings, Schema or database layout. Distinct facts and prices still have independent costs.

Requests with different totals can still contain equal pricing components. Components use Arc sharing only when category, count, amounts, status and rate are fully equal. The pool retains at most 4,096 entries and 1MiB of component text, excluding entries with more than 1KiB of text, plus bounded structure and table overhead. Descriptive price text uses an independent pool with the same bounded text-sharing implementation as facts. Over-budget values remain independent; published parts are not copied just to compact them. Request counts and amounts are never merged. Aggregation streams into four category accumulators, eliminating input-sized component-reference lists and rate sets. Distinct bases, issues and price revisions still need their own sets, so total aggregation space is not claimed constant. Both totals and components validate status and exact amounts; single-row queries also validate while preserving request-specific evidence.

Each connection sets a4MiB page-cache target, file temporary storage,512-page automatic checkpoints and an8MiB journal-retention target. WAL, FULL synchronization and the original commit transaction remain. Journal retention is not a hard space limit: large transactions, active readers and failed checkpoints can temporarily grow WAL. The page-cache budget is not a process peak-memory limit; in-memory facts and revisions have separate costs.

On 2026-10-03, the unreleased first-version baseline removed kv migration and compaction branches. Only empty databases initialize the current layout. Unsupported layouts or versions are rejected without deleting existing data. Review records and immutable snapshots use their own stores.

## Alternatives considered

Keeping text kv and only limiting WAL reduces journal retention without removing repeated row keys. General compression of small records adds codec costs and dependencies; this change reuses locked SQLite first. Equal candidates/projections now share storage; distinct derived values remain separate. Database aggregation and content-addressed snapshots affect fixed revisions and read costs and still need separate validation.

## Impact and verification

Current regressions cover reopening the current layout, maximum integers and tiny floats, Unicode, unsupported layouts/versions without data loss, empty maps/scalars, field/source isolation, deletion, unchanged writes and corrupt payloads. CLI checks cover cached revisions, cursor appends, fixed queries and snapshot export.

Release benchmarks with fixed fixtures separately record cold/warm queries, appends, peak RSS, database/WAL and disk after idle exit. Results and limits are in verification records retained in Git history. Equal facts no longer have duplicate payloads, but in-memory facts still grow with history. Million-record scale, long residency, other platforms and real sources are not accepted by this change.
