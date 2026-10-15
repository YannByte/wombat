# CLI and automation

[中文](cli.md) | English

Wombat synchronizes local Codex logs incrementally by default and updates queries incrementally. An explicit refresh also saves a fixed snapshot. Current functionality includes usage, tasks, whole-turn timing, configuration measurements and manual review queries. Every subcommand works without a TTY; JSON and Web use the same Rust queries.

```sh
wombat prices --json
wombat prices update --json
wombat refresh --root /path/to/codex-home --json
wombat usage --since 2026-09-23 --until 2026-09-30 --timezone Asia/Shanghai --json
wombat threads --sort tokens --json
wombat turns --thread THREAD_ID --sort tokens --json
wombat steps --thread THREAD_ID --turn TURN_ID --sort time --json
```

Repeat `--root` to include multiple roots. Without it, use CODEX_HOME or `~/.codex`. Both sessions and archived_sessions are read; the display range does not limit historical collection. Refresh rejects query filters.


`wombat web --open` asks the OS to open a browser after startup. If opening fails, the server stays available with the printed URL.

Hook inventory JSON exposes the same per-project native registration and plugin identity as Web detail; text output also names the plugin. Registration does not establish execution. See the [configuration contract](../development/contracts.en.md) for supported declaration forms and unknown states.


## Agent interface

Use `wombat api --json` to read the installed method names, descriptions and limits without scanning data. Use `wombat api --method usage --json` for one input Schema; add `--output-schema` when response fields are needed. Both schemas derive from Rust. Reuse them for the same runtime version.

```sh
wombat api --method usage --json
wombat call <<'JSON'
{"method":"usage","params":{"mode":"auto","query":{"action":"threads","scope":{"project":"/absolute/project","since":"2026-10-01","until":"2026-10-08","timezone":"Asia/Shanghai"},"sort":"tokens","limit":3,"compact":true}}}
JSON
```

`call` accepts one JSON object on stdin and defaults to JSON output without prompts. It dispatches only the listed generated product methods; use exact params from the selected Schema. The result retains its owning protocol, including the `usage` live wrapper's `result` and `freshness`. Keep snapshot/read-view identities, scope, page offsets and cursors for drill-down. Mutating actions retain existing authorization and selection requirements.

Input is limited to 1 MiB with a ten-second stdin deadline; output is limited to 2 MiB. Product query budgets still apply. Errors return `{outputVersion:1,error:{code,message,recovery}}` without raw input, paths or internal diagnostics. `recovery` tells the caller to read the Schema, reacquire the original view, narrow the query, retry the same scope, check setup or inspect state; it does not authorize an automatic retry of a write. Exit codes are 0 for a complete result, 2 for partial results, 1 for errors and 130 for cancellation. The same policies apply to corresponding human commands. Ctrl+C cancels this call without stopping shared scanning.

## Compare usage

Period and session comparisons read committed data without refreshing by default. Run `wombat usage --fresh` first, then use these commands. End dates are exclusive; periods must be equal-length and non-overlapping.

```sh
wombat compare --since 2026-09-08 --until 2026-09-15 --baseline-since 2026-09-01 --baseline-until 2026-09-08 --dimension project --json
wombat compare --thread THREAD_ID --other-thread OTHER_THREAD_ID --family --all-time --json
```

`--dimension` accepts `project`, `model` or `thread`; `--limit` and `--offset` page contributions. Session IDs use field `id` from `threads --json`; `--family` includes explicit descendants. Both comparisons retain date and dimension filters and accept `--snapshot`. Only explicit `--fresh` requests synchronization. Expired views return an error; read the list again before comparing. Live results expose `freshness.publicationChange` for the latest successful publication; text also shows change counts. No complete baseline means no summary. The [core reference](../reference/core.en.md) owns calculation and limits.

## Task distributions and budgets

Use task statistics to judge a task against a selected population or separate task-count growth from per-task growth. Keep a returned snapshot for follow-up pages and comparisons.

```sh
wombat statistics --project /absolute/project --since 2026-10-01 --until 2026-10-08 --presentation projects --json
wombat statistics --snapshot SNAPSHOT_ID --thread THREAD_ID --since 2026-10-01 --until 2026-10-08 --json
wombat statistics --snapshot SNAPSHOT_ID --since 2026-10-01 --until 2026-10-08 --baseline-since 2026-09-24 --baseline-until 2026-10-01 --presentation models --json
wombat monitor set --id project-week --period week --tokens 1000000 --review --project /absolute/project --timezone Asia/Shanghai --json
wombat monitor check --id project-week --json
wombat monitor watch --id project-week --interval 60 --json
wombat monitor list --json
wombat monitor acknowledge --notification NOTIFICATION_ID --json
wombat monitor remove --id project-week --json
```

Statistics defaults to the last thirty calendar days; `--all-time` selects all available dates. Project/model groups accept `--limit` and `--offset`; the population covers the full scope. A selected `--thread` is ranked against that population. Complete task totals provide the mean, median and P90; incomplete tasks and unassigned measurements remain separate. Model populations can overlap when a task used multiple models. Equal-length, disjoint comparison periods retain all other filters. The [core reference](../reference/core.en.md) owns calculation methods and limits.

A monitor plan saves its identity filters, timezone, calendar period and optional Token threshold. `--review` also requests the most recently closed period's statistics. Add `--disabled` to save a paused plan; reuse its ID to change settings. Default warning is 80% of the threshold, configurable with `--warning 0.9`. These are user budgets, separate from account allowances. `check` checks all enabled plans unless `--id` selects one; repeated `--root` selects sources. A recent pinned check uses `--snapshot ID --id PLAN_ID` without source roots. An expired view requires a fresh check of the original scope.

`watch` checks while this command runs and emits only new notifications. It does not install a background service or send messages; Ctrl+C stops it with exit130. `--interval` accepts 5–3600 seconds. Web has the same plans, manual checks and an optional sixty-second loop while its budget panel stays open. Monitoring needs an active host, and source logs must record usage before thresholds can be detected. Partial facts can prove a threshold crossing but cannot prove remaining budget. Acknowledgement is stored separately from the original facts. Period reviews cover the latest closed period, without catching up every missed period.

## Usage inspection and review

Read or refresh usage first, then pin these queries with the returned `snapshotRef.snapshotId`. They read committed data without synchronization by default.

```sh
wombat investigate --snapshot SNAPSHOT_ID --all-time --limit 3 --compact --json
wombat investigate --snapshot SNAPSHOT_ID --thread THREAD_ID --turn TURN_ID --all-time --compact --json
wombat context --snapshot SNAPSHOT_ID --all-time --json
wombat trajectory --snapshot SNAPSHOT_ID --thread THREAD_ID --all-time --json
wombat resources --snapshot SNAPSHOT_ID --project /absolute/project --all-time --json
wombat review --snapshot SNAPSHOT_ID --since 2026-09-08 --until 2026-09-15 --json
wombat steps --snapshot SNAPSHOT_ID --thread THREAD_ID --turn TURN_ID --locate-operation OPERATION_ID --json
wombat account history --json
```

Review does not accept pagination. Without dates, it selects the fixed view cutoff's week, starting Monday. Other inspections accept `--limit` and `--offset` while retaining scope. Drill with returned complete evidence identities. An absent operation returns `NOT_FOUND`; after view expiry, reacquire the original scope and locate identities again. Allowance history needs no native process and reads stored observations only. The [core reference](../reference/core.en.md) owns thresholds, calculation and budgets. Signals do not establish waste, input is not context occupancy, and change reports are not verified file changes.

## Codex Skill

The redesigned [user Skill](plugin.en.md) starts from conversational tasks and can use Web to inspect evidence. Processing continues in the current Codex conversation; Web retains viewing and rechecks. The Skill consumes existing CLI JSON and bounded index restoration/synchronization; account reads do not wait for logs.

Codex manages the formal plugin, whose invocation name on verified Codex 0.160.0 is $wombat:wombat. Standalone local trials use $wombat:

```sh
wombat skill install --json
wombat skill status --cwd /path/to/project --json
wombat skill install --replace --json
wombat skill uninstall --json
```

Standalone installation copies only reviewed local product assets, defaulting to ~/.agents/skills/wombat. --directory selects a directory ending in wombat; --cwd selects the native discovery project. Only unchanged managed copies can be explicitly replaced or removed; custom directories, links, and user changes are protected. Independent v1 JSON separates file state, discovery/enablement, runtime capabilities, and data not requested. Installation does not scan logs. Unconfirmed native discovery exits2 without rolling back completed file installation. These commands do not manage plugins.

web --context FILE --json accepts a restricted generated contract: page is usage/threads/instructions/extensions/optimize, with the corresponding read-only usage/configuration/optimization request. --root/--project-root still authorize startup roots. The host reads and validates versions before returning effective context and a connection URL. Project, source, full object IDs, timezone, dates, and versions are retained; the product converts exclusive CLI until into display dates. Supply paired dates or allTime; unsupported browser filters are rejected. The browser uses its own pagination size. Restart requires a new URL, and connection tokens must stay local.

For a separate Codex task, CLI handoff checks actually enabled Skills per project, rechecks names/paths, and sends text plus a skill item to the persistent queue. Use --skill PROJECT_ID=PATH. Missing, disabled, or conflicting instances require selection or explicit --without-skill to use existing handoff behavior. Unknown delivery is never retried automatically; acceptance does not establish completed changes or rechecks. The primary Web interface no longer provides dispatch buttons; existing handoff interfaces and records remain. See the [product plan](../decisions/proposed/product/2026-10-04-codex-skill.en.md).

## Language

Choose presentation language with `--lang zh` or `--lang en`, or set `WOMBAT_LANG`. JSON fields and source content retain their original values. See [product language](../i18n/product.en.md) for precedence.

## Automatic synchronization

- Ordinary queries synchronize by default, waiting up to 2 seconds. Existing results are returned with `freshness`; without data, return `SYNC_PENDING`. Building a large initial index can take longer.
- `--fresh` waits up to 10 seconds for this synchronization and reports failure or timeout. `--cached` returns the committed index without scanning sources; it fails if no index exists.
- `usage --watch --json` emits one JSON object per line when the revision, date range, or status changes. Ctrl+C exits with 130. Ordinary `--json` still emits one object.
- `refresh --verify` rereads the full source before saving a snapshot, checking historical prefix edits that the append fast path cannot establish. Ordinary refresh uses incremental cursors.
- `--snapshot ID` pins a current snapshot. Continue pagination with the same snapshotId instead of resolving latest again. Unknown formats are explicitly rejected.
- Live queries accept `--root`. Omitting it always selects the default Codex source; another window's roots do not change this. Fixed usage snapshots cannot also specify source roots; timing roots bind the requested source scope to the fixed version.

One data directory shares a Rust service. The installed background service keeps indexing without a CLI or browser; `usage --watch` only observes results. Accounting uses complete log records and cannot show tokens the model has not yet logged. Service lifecycle and resource limits belong in the [core reference](../reference/core.en.md).

## Background indexing

```sh
wombat service status --json
wombat service pause --json
wombat service resume --json
wombat service stop --json
wombat service start --json
wombat service disable --json
wombat service enable --json
```

`status` reports the running instance, preparation, committed-data readiness and configured automatic startup separately. `pause` takes effect at the current transaction boundary and preserves queries; `resume` continues indexing. `stop` waits boundedly for this instance to exit; later queries or login can start it again. `disable` removes current-user automatic startup and stops the service. `enable` registers and starts it. Automatic startup needs a managed installation and an available native user-session manager. An explicit-root Web host cannot change global service settings. Web setup exposes status and permitted pause/resume controls.

## Whole-turn timing

```sh
wombat timing --thread THREAD_ID --turn TURN_ID
wombat timing summary --thread THREAD_ID --turn TURN_ID --text --lang en
wombat timing --thread THREAD_ID --turn TURN_ID --snapshot SNAPSHOT_ID --share
wombat timing evidence --thread THREAD_ID --turn TURN_ID --snapshot SNAPSHOT_ID --limit 50
wombat timing evidence --thread THREAD_ID --turn TURN_ID --snapshot SNAPSHOT_ID --limit 50 --cursor OPAQUE_TOKEN
wombat timing capabilities
```

`timing` and `timing summary` are equivalent. Default output is one final v6 JSON object; `--text` selects localized human text and cannot accompany `--json`. Use complete Wombat thread and turn identities from the task query. Missing values remain null in JSON; text explains required records or the available scope; native duration, derived duration, native TTFT, and first-content record delay remain separate. Interval unions and sums may overlap and are not added together.

Summary supports repeated `--root`, `--source`, and one of `--fresh` or `--cached`. `--snapshot` pins the result and cannot accompany `--fresh`. When roots or source are supplied with a fixed version, they must match its authorized scope. Use the returned snapshot identity for subsequent evidence pages; a missing or expired version never falls back to latest. Evidence is local only, requires that snapshot and the same target/scope, accepts `--limit 1..200` (default50) and the unchanged `nextCursor.token`, and accepts no refresh mode. Paging does not change the whole-turn summary. Capabilities accepts only output/language options and optional `--share`, reports parser support without scanning, and does not establish availability in a particular turn.

`--share` asks Rust for a separate safe summary projection; CLI does not remove fields from local JSON. Timing bypasses automatic prices, configuration scans, Hook capture, and account observation. Date, Token, cost, offset, compare, and watch options are rejected. A resource-limited result can retain verified native scalars while affected derived values are omitted with calculation limits.

Summary exits0 when the core marks the inspected scope complete, even if optional fields are not recorded; partial or provisional results exit2. Successful evidence navigation and capabilities exit0 without establishing whole-turn completeness. Errors exit1 and cancellation exits130. JSON errors use `{outputVersion:1,error:{code,message}}` with localized safe templates, including NOT_FOUND and VIEW_EXPIRED, without source paths or underlying error details. Status stays on stderr. Ctrl+C cancels this call and does not stop shared synchronization. Use `corepack pnpm verify:e2e` for real-core and browser acceptance; results cover only the selected platform and scope.

## Official price catalog

Live queries automatically check the official catalog when repairable missing rates are found. Failures retain results and expose `priceUpdate`. Downloads are throttled for 15 minutes after failure and 24 hours after success. Set `WOMBAT_AUTO_PRICES=0` to disable automatic networking; `--cached` and `--snapshot` never trigger it. Manual `prices update` bypasses the automatic retry interval.

`prices` (or `prices status`) reads the current full catalog offline. `prices update` explicitly downloads and validates a fixed official source. Its default output is a short result; `--json` returns `outputVersion:1`, action, origin, updated, source, sourceHash, catalogHash, and the complete catalog. Price responses and errors are versioned separately from usage v5. Errors use `{outputVersion:1,error:{code,message}}`, with exit code 1 or cancellation code 130. Common failures include PRICE_FETCH_FAILED, PRICE_SOURCE_CHANGED, PRICE_CACHE_INVALID, OUTPUT_LIMIT, TIMEOUT, and UPDATE_BUSY.

After a successful update, the next live synchronization creates a complete read revision using the new catalog. `refresh` can save another snapshot; old snapshots retain their amounts. Updates reject custom URLs, import paths, and usage filters. See [pricing](../reference/pricing.en.md#联网更新价表) for network scope, proxies, supported tables, and persistence.

## Filtering and pagination

- `usage --group day|week|month`: without dates, day selects the last 30 calendar days, week selects this month and the previous 5 months, and month selects this month and the previous 11. All end today; response until is tomorrow, exclusively. Explicit since/until takes precedence, and changing grouping preserves a manual range. A task filter without dates selects its full range. Weeks start on Monday; events are assigned by timestamp and selected timezone.
- `--since` is inclusive and `--until` exclusive. `--all-time` includes unknown dates and cannot accompany date bounds or --undated. CLI timezone defaults to UTC; Web uses the system timezone.
- `--model`, `--effort`, and `--project` match exactly. A project is observed directory evidence, not a path substring. `--model-unknown`, `--effort-unknown`, and `--undated` select missing model, effort, and date respectively, and cannot accompany corresponding explicit values or date ranges.
- `threads --search TEXT` searches titles or projects; its sorts are `tokens|cost|recent`, using matching usage or latest matching measurement. Turns and steps use `tokens|cost|time`.
- `--snapshot ID` pins a current snapshot. Continue pagination with the same snapshotId instead of resolving latest again. Unknown formats are explicitly rejected.
- `usage --presentation distribution|details`: defaults to details for category breakdowns; distribution returns only period subtotals. Explicitly setting this option pages by period: limit counts periods and details retain every model row in each selected period. Omitting it preserves row pagination. `--sort time|tokens|cost` orders by descending date or consumption. Complete-range distribution scales, peak scopes, unpriced tokens, and cost shares are computed before pagination; unknown costs sort after known costs.
- `--limit 1..500 --offset N`, default 50. Sorting, amounts, categories, and shares are computed over the full matching range before pagination.

## JSON

`--compact --json` bounds quality detail to three examples per retained list and omits facets; `quality.detailSummary` retains full issue/source counts, grouped counts and omitted-detail counts. Totals, requested rows, pagination, scope and snapshot remain unchanged. Remove `--compact` for full detail. `--limit` changes returned pages, not whole-task computation. On RESOURCE_LIMIT, select a turn with `turns --matched-only`, then investigate that exact turn at the same snapshot and filters. Selected-turn findings do not establish whole-task rankings.


Usage uses `outputVersion: 5`. Success includes action, snapshotRef, scope, availableRange, summary, items, page, and quality. Generated schemas validate runtime data; unknown arguments are rejected. Ordinary queries emit exactly one final JSON object on stdout, while watch emits NDJSON. Status goes to stderr. Live results also contain freshness; status distinguishes current, syncing, stale, failed, and fixed, and checkedAt is the last successful check. Current only means the observed log range has been processed.

Amounts are decimal strings; tokens are safe integers or null. `price.cost=null` means an incomplete amount; `knownCost` is the known subtotal, with priced, partial, and unknown statuses. Missing is not zero, and reportedCost is not added to the standard equivalent. Never sum amounts already rounded to two display decimals.

Tasks return matchedUsage and threadUsage. Turns cover the whole task by default while matchedUsage preserves incoming filters. Add --matched-only to turns to return only turns with matching measurements; --locate-turn ID locates its page, taking precedence over offset when found and otherwise using offset. Filtering turns does not change the full-task summary. Turn shares use the full task denominator; step shares use the full turn. Operations without exclusive accounting do not display a cost. `unassigned` holds records without a turn within the task.

Exit codes: 0 for success or an empty range; 2 for usable but incomplete reads or unconfirmed synchronization; 1 for errors; 130 for cancellation. Errors use `{outputVersion:5,error:{code,message}}`. Common codes include INVALID_ARGUMENT, NO_SNAPSHOT, SOURCE_UNREADABLE, SNAPSHOT_CORRUPT, UNSUPPORTED_VERSION, UPDATE_BUSY, CANCELLED, RESOURCE_LIMIT, and DETAIL_UNAVAILABLE. Partial results can still be queried using the returned fixed snapshot.

## Current formats

Only current usage-v4 snapshots (schema4) are readable. Usage JSON v5 and timing JSON v6 are independent output formats. Unknown versions are rejected while existing files and user records remain intact. No migration or old-command/output compatibility is provided. Retired scan/report/checkup/quota/codex/observe/compare commands have no placeholder entries.

## Local Web

Run `wombat web` and open the printed link. `--port 0` selects a port automatically by default; repeat `--root <directory>` to restrict sources, use `--lang zh/en` for the initial language, and `--json` for one startup JSON record. Ctrl+C stops this Web host; closing a tab does not exit the CLI.

Access is local only; restart requires a new link. Pages provide usage, tasks, turns, sources, and prices; date, model, effort, and directory filters are retained in the URL. Reloading reads a local version again; use Refresh data to recover an expired version. Existing business interfaces are connected over HTTP and source logs remain read-only; remote deployment is unsupported. See [architecture](../development/architecture.en.md).

New queries: `usage --presentation projects|models` groups by historical directory or model; `--project-unknown` selects missing directory evidence, `--agent` / `--source` limits sources, and `threads --locate-thread ID` returns the page containing a full ID. Task sorting uses matching usage/latest matching measurement; full-task usage remains separate.

## Read-only configuration

`wombat web --project-root /path/to/project` adds a configuration folder absent from history and is repeatable; omission retains the launch directory. Local Web automatically adds reliable historical cwd values from current sources to its project catalog; direct CLI configuration queries still use `--project-root` for explicit scope. `--root` continues to select Codex sources separately from project configuration roots.

`wombat optimize inventory --json` provides equivalent non-TTY queries. Filter and sort with `--kind rule|skill|mcp`, `--observation used|loaded_only|unknown`, `--search`, and `--sort tokens|activity|size|name|content_tokens|characters|recent`. `--limit` defaults to50, maximum200; `--offset` starts at0. `--since` / `--until` include the start and exclude the end; `--timezone` defaults to UTC.

Details and evidence use `--action detail|evidence|related_scopes --item ID --read-view VERSION`; a new view can use `--snapshot live:…` to pin usage. Continued queries must keep the same source and project-root arguments; query the list again after expiry. `--thread ID` lists only configuration with evidence linked to that task. `--action capabilities` does not scan configuration. See `wombat optimize inventory --help` for all options.

Responses use configuration v1 JSON. Incomplete historical coverage currently returns exit code2 with usable results; argument/service errors return1 and cancellation130. File reads are not Skill calls; associated tokens are not exclusive configuration costs. Full-content tokens use a fixed reference method; static suggestions and manual rechecks are connected, while source changes remain closed. See the [configuration contract](../development/contracts.en.md) for scope and data semantics.

## Static suggestions and manual rechecks

To inspect one turn, first obtain its snapshot/thread/turn IDs from the task list or timing summary. Run `wombat optimize activity --snapshot SNAPSHOT_ID --thread THREAD_ID --turn TURN_ID --json`, retaining the same `--root` and optional `--source` as the selected view. Text output shows three check results and applicable inspection advice. This read collects no configuration and saves no handling decisions. Use timing evidence with the same selection to inspect records. If the view expires, reopen the list and select a new view; do not combine results from different snapshots.

Run `wombat optimize list --json` with the same `--root` / `--project-root` as configuration. Suggestions merge by object in repair/trim/organize/space categories; the latter two explicitly remain unavailable without adapters. Checks accept no usage dates/models, and static reminders do not establish configuration health.

```sh
wombat optimize list --project-root /path/to/project --json
wombat optimize detail --suggestion SUGGESTION_ID --read-view READ_VIEW --decision-revision REVISION --json
wombat optimize keep --reason necessary --suggestion SUGGESTION_ID --read-view READ_VIEW --decision-revision REVISION --json
wombat optimize history --read-view READ_VIEW --json
wombat optimize redisplay --suggestion SUGGESTION_ID --read-view READ_VIEW --decision-revision REVISION --json
wombat optimize not-applicable --reason incorrect_evidence --suggestion SUGGESTION_ID --read-view READ_VIEW --decision-revision REVISION --json
wombat optimize recheck --read-view READ_VIEW --decision-revision REVISION --json
wombat optimize capabilities --json
```

Retain original roots and project/source scope, using the returned decisionRevision after each operation. Reload on conflicts. detail/keep/not-applicable/redisplay require suggestion identity; keep/not-applicable also require a reason. Lists support category, offset and limit 1..200 (default 50). Edit outside Wombat, then recheck directly; --suggestion optionally limits rechecks to one problem. Rechecks do not write sources or revoke decisions; redisplay does not restore files. Configuration/review results use v1, exit 2 for evidence gaps, 1 for errors and 130 for cancellation. Capabilities do not scan. Index rebuilds retain product-data records.

Adjust product reminders with `--agents-bytes 16384 --description-characters 500`, retaining the same flags in subsequent operations. Values apply to authorized current configuration and stay in records. The body5,000 reference line and description1,024 specification maximum are immutable; the Web reminder panel shares this contract. Full/body estimates are separate; see [contracts](../development/contracts.en.md).

The Web optimization list presents a suggestion, value and key metric. Details expand handling steps and basis, with record counts, turn tokens and estimated API cost visible together. Related record dates do not change current static checks; exact turns return to the original suggestion and filters. No association remains unknown; costs are not allocated by counts. CLI/Agent first obtain item.id/readView from optimize, then use inventory --action evidence --item ID --read-view VERSION with date/timezone flags; returned usageRevision/threadId/turnId locate turns. Keep authorized roots and avoid mixing read versions. Explicit rechecks expose before/after text measurements through reviewBaseline/item; unknown or mismatched methods are incomparable, and changes do not establish savings.

## Local Codex handoff and account

Handoff requires a runnable local Codex; the native interface is verified on0.160.0. Codex owns sign-in, model review, execution and restoration. CLI and the retained HTTP handoff interface share the same selection; sign-in credentials are not copied.

```sh
wombat optimize handoff preview --project-root /path/to/project --json
wombat optimize handoff send --project-root /path/to/project --selection-version SELECTION_VERSION --read-view READ_VIEW --decision-revision REVISION --json
wombat account read --json
wombat account refresh --json
```

Review project working directories, files and evidence in preview, then send using its selectionVersion; retain the same source, project grants and filters. Omitting `--suggestion` selects all pending items independently of list pagination or category; repeat the flag for a single item/subset. Shared files appear once and same-file rules merge; changing projects requires a new confirmation. Changes to files/evidence before sending are explicitly rejected.

Delivery returns accepted, failed or unknown and an available Codex task ID; accepted establishes request acceptance only. Inspect with `codex resume TASK_ID`. After disconnection inspect Codex before deciding to resend; resending is manual, without execution receipts or historical lookup. Repeated manual sends may create new tasks; closing Wombat does not cancel accepted tasks. Use `optimize recheck` after handling to determine whether a problem remains.

Low allowance only warns; a reliable native restriction on the current task prevents sending. Expired, unknown or other-model restrictions do not establish exhaustion for this task or trigger automatic retries. Sending checks the actual task again; a final restriction may leave an empty Codex task with no queued request. The legacy confirmation component remains but is not a primary Web entry.

The account v1 response has independent identity, allowance and activity status/read times, with masked email only. Actual window names, models, periods and resets come from Codex, without fixed five-hour/seven-day periods or legacy single-bucket fallback. Failures retain previous data/read times; account changes clear old facts. A past reset does not establish restored allowance. Balances and spending limits retain native decimal strings without guessing units. Reset credits are read-only; missing details and an empty list differ, and the 128-row detail limit never replaces the native count. Overview and account details share observations independently of project/date scope. Allowances are not added to or converted from project tokens/API estimates. Partial reads or unconfirmed delivery exit2, errors1 and cancellation130. `turns --sort recent` orders by reliable activity times, with unknown times last.

## Setup and collection

`wombat setup --project /path/to/project --json` checks native discovery and registration, preserving each state independently. It reports the full Codex version, Wombat bundle version, and each discovered copy’s declared capabilities and content verification. Text output includes partial error codes. Log queries need no Hook trust; an available and compatible Skill can be used in a new Codex conversation. It does not install, trust, scan account credentials or invoke a model. `--root` selects source directories. A missing Skill does not block viewing existing data.

```sh
wombat collection status --json
wombat collection mode hooks --json
wombat collection events --project /path/to/project --limit 50 --json
wombat collection pause --json
wombat collection resume --json
wombat collection mode logs --json
wombat usage --watch --json
```

Preferences are machine-wide; `--project` and repeated `--root` filter status and events only. Mode `logs` ignores new Hook input and preserves data. Mode `hooks` permits safe receipt but establishes neither registration nor trust. Install the [local collection package](plugin.en.md), then review its declarations in Codex `/hooks`. The POSIX bridge resolves the managed launcher or Codex PATH; Windows remains unaccepted. Plugin removal does not delete observations.

Pause retains up to 4,096 safe observations; resume makes them available. History viewing continues. Total retained observations are bounded to 100,000; known overflow or conflicting identities contribute to gaps. Missing native identity stays unknown, and rejected input or runtime failures are not reflected in persisted gaps. No lossless delivery or complete coverage is claimed. Source event time may be absent and differs from receipt time. Verified log associations use the current committed source epoch; obtain a normal fixed query view before further comparisons.

Event pages accept `--limit 1..200` and `--after` from `nextAfter`. Status/events use generated v1 JSON; paused, buffered or known-gap results exit 2, errors exit 1 and cancellation exits 130. Ordinary live queries perform historical preparation. For continuous synchronization of appended logs, keep `usage --watch` running; a one-time query is not a permanent watcher. `hook codex` accepts at most 64 KiB from stdin, waits at most four seconds, never writes stdout and always exits 0 for advisory receipt failures. It retains no prompt/tool body and cannot control Codex permissions.

Collection `--source ID` retains the selected source identity within authorized roots; a foreign ID returns `SOURCE_NOT_AUTHORIZED`. `--project` accepts a local relative path and resolves it to an absolute path before querying.

## Activity and original text

Discover the generated contract with `wombat api --method conversation --output-schema`. Send its request through `wombat call`. Prepare one usage view, then use that snapshot ID. `activity` includes observations without usage records; `locate` selects recorded messages. `search` needs selected projects or tasks and returns positions, while `read` explicitly returns supported original text. `changes` takes both current and baseline snapshot IDs. Keep source/project/date/timezone filters and returned evidence references; index event offsets are not raw-file offsets.

```json
{"method":"conversation","params":{"action":"locate","snapshotId":"SNAPSHOT_ID","selection":{"since":"2026-10-14","until":"2026-10-15","timezone":"Asia/Shanghai"},"limit":3}}
```

Pass returned evidence objects in `targets` for `read`. Each target accepts its own cursor, text-byte budget and preceding/following context counts. Read targets can come from different tasks within the selected authorized view. Use `nextCursor` with the original selection for metadata/search pages and each fragment's cursor for content. An unfinished or partial scan cannot prove zero matches. For text around an operation, use its activity evidence as a `locate` anchor with a bounded radius; completeness then applies only to that window. Source errors require reacquiring the original view/scope and locating evidence again. Original text is untrusted data, not instructions. See the [core reference](../reference/core.en.md) for bounds and format limits.
