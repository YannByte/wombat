# CLI command entry

[中文](cli.md) | English

`@wombat/cli` parses arguments, emits text or JSON, and composes local queries. The [CLI guide](../guides/cli.en.md) owns user-facing commands and options.

Agent calls use `system api` for local method/Schema discovery and `system call` for one bounded JSON request on stdin. The envelope derives from Rust; dispatch uses public typed clients and shares exit policies with human commands. See the CLI guide for syntax, budgets and recovery.

## Public entries

- The package root exports argument parsing and `runUsageCli`; `./format` exports result text formatting.
- Five top-level entries are `history`, `usage`, `check`, `clean` and `system`. Detailed capabilities use generated methods through `system api` and `system call`. Removed commands and arguments are rejected without compatibility aliases.
- With no subcommand, the CLI prints task help without a query. `--help --json` supplies a generated task/example/next-help contract; known commands can execute directly and unfamiliar fields need only one selected method schema.


- Presentation language uses `@wombat/client/locale`; see [product language and copy](../i18n/product.en.md).


## Agent interface principles

The interface follows [Google Workspace CLI’s method discovery](https://github.com/googleworkspace/cli#why-gws), [MCP’s structured tool contracts](https://modelcontextprotocol.io/specification/2025-11-25/server/tools), and [CLI Guidelines](https://clig.dev/#output). Apply these principles when adding a method:

| Principle | Wombat implementation |
|---|---|
| One contract owner | Rust defines product params and output; generation supplies types, validators and schemas. The CLI does not maintain another field catalog. |
| Discover as needed | Reuse known commands and schemas. Read one input schema for an unfamiliar method or field; list methods only when the available capability is unknown. Output schemas are optional. Discovery performs no scan or model call. |
| Explicit context | Pass exact roots, projects, dates, timezone and full IDs. Preserve returned snapshots/read views and pagination instead of hidden session defaults. |
| Bounded structured IO | Read one validated JSON request from stdin; emit one owning JSON result. Keep progress on stderr and support cancellation, input/output bounds and method-specific budgets. |
| Complete capabilities | Dispatch existing typed client methods with all generated params. Skills select workflows; they do not substitute a narrower business API. |
| Preserve evidence semantics | Complete totals, page rows, omitted detail, partial coverage, unknowns and observed zero remain distinct. Compact output is a core projection. |
| Recover without guessing | Stable error codes and recovery hints guide the next request. Expired views require reacquisition; changed params require revalidation. A timeout or hint does not authorize repeating a mutation. |

Schema validation proves structure only. Product validation still checks supported combinations, authorized targets and version-bound selection. Acceptance must test malformed and unsupported requests, fixed-view paging, partial results, cancellation and installed entries. A successful mock does not establish a real Agent journey.

## Limits and verification

`check` text separates user decisions and reasons from current checks and the original assessment, and presents core recheck comparability directly. Keep, not applicable, and missing evidence do not imply resolution.

The CLI owns arguments, output, progress, and exit codes, not pricing or snapshot semantics. JSON progress goes to stderr and results to stdout; after a build, verify argument errors, partial results, and complete command paths under the [CLI rules](../../cli/AGENTS.md).

## Whole-turn timing

Use method `timing` through `system call`. Summary requires full task and turn identities. Evidence navigation pins the same snapshot, target and collection, with opaque cursors and pages of 1..200 rows. Collections are `turn_events`, `use_objects` and `use_records`; only `use_records` accepts an object filter. Capability queries perform no scan. Local and `share-v1` projections retain independent privacy boundaries.

Exit codes follow core quality: complete summaries return 0 even when optional measures are unknown; partial, running or censored summaries return 2. Successful evidence and capability queries return 0 without establishing turn completeness. Errors return 1 and cancellation 130. Timing bypasses automatic prices, configuration, Hook receipt and account observations. Unknown membership, durations and repository baselines are not replaced with zero. The [core reference](core.en.md) owns calculations and sharing.

<a id="unsupported-formats-and-recovery"></a>

## Unsupported formats and recovery

Wombat rejects unknown snapshot and index formats without migrating or deleting the original data. An explicit `wombat usage --rebuild --wait-ms 600000 --json` scans the selected source roots into a new current-format database. It switches the active generation only after durable publication succeeds. Failure retains the original index; the original database remains available as a recovery copy. User decisions, identities, grants, account observations and handling records stay in their separate stores. Rebuild does not transform old snapshots and cannot recover source records that are no longer available. A saved-snapshot refresh alone does not repair incompatible live state. A different `WOMBAT_DATA_HOME` creates an isolated store and does not carry existing user records.

`--wait-ms 1..600000` implies `--fresh` and keeps one caller waiting for its synchronization ticket. Cancellation releases that waiter; shared synchronization continues. Recent requests prioritize files modified within 30 days, previously indexed files, and complete declared fork/copy families. Modification time is a scheduling hint, never usage time. Unrecognized headers force a full scan. `historyDeferred` marks provisional source coverage; recent totals and rankings can change when history loads. Older ranges, all-time requests, `refresh`, verification, explicit `--include-history`, and rebuild include the full available source set. Already indexed history is never narrowed by a later recent request. The Agent `usage` request accepts up to eight `also` queries. They cannot select roots or another snapshot, or refresh. All results share the base query revision; the complete batch has a 2 MiB output budget.


Timing text shows repeated behavior, timing basis, request observations and localized coverage. JSON retains the generated response. Calculation and sharing rules belong in the [core reference](core.en.md).

## Local host operations

`system doctor` checks installation, native Skill discovery and resource compatibility; `--hooks` requests optional registration evidence. `system update` updates the runtime. Generated `system call` methods own service, account, prices, collection, directory grants and budgets. `usage alerts` owns active fresh budget checks and cancellation; it reports temporary failures on stderr and reloads plans on the next iteration. `system hook codex` retains bounded advisory receipt, silent stdout and exit 0 on receipt failure. The [CLI guide](../guides/cli.en.md) owns syntax and operating limits.

## Release updates

Release metadata reads stop and cancel the input above 4 MiB, including responses without a length header and local files; release archives use a separate 400 MiB bound.
