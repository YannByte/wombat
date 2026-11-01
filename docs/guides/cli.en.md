# CLI and automation

[中文](cli.md) | English

Wombat has five command entries. With no command, it prints usage. Queries prepare local data only when needed; installation needs no initialization or scan. Detailed operations use the generated JSON interface.

## Choose an entry

| Entry | Purpose |
|---|---|
| `usage [stats\|compare]` | Usage totals, task distributions and comparisons |
| `check` | Project inspection, explicit decisions and original-condition rechecks |
| `system` | Installation checks, updates, active budget checks and advisory Hook input |
| `api` | Discover local methods and schemas |
| `call` | Execute one generated JSON request |

## Usage and statistics

```sh
wombat usage --since 2026-10-01 --until 2026-10-08 --timezone Asia/Shanghai --json
wombat usage stats --project /absolute/project --since 2026-10-01 --until 2026-10-08 --json
wombat usage compare --since 2026-10-08 --until 2026-10-15 --baseline-since 2026-10-01 --baseline-until 2026-10-08 --dimension project --json
wombat usage compare --thread THREAD_ID --other-thread OTHER_THREAD_ID --family --all-time --json
```

The start date is inclusive and the end exclusive. The timezone defaults to UTC. Exact project, model, effort, agent and source filters retain their meaning. Unknown filters select missing values, not zero. `--all-time` includes undated records and excludes date bounds and `--undated`. Project attribution uses observed directories, never path substrings.

`--group day|week|month` selects calendar periods. Without dates, day covers the last 30 calendar days, week this month and the previous five months, and month this month and the previous eleven. Explicit dates take precedence; a task without dates uses its full range. Weeks start on Monday. Events use their timestamps and the selected timezone.

`--presentation distribution|details|projects|models` selects the returned grouping. Explicit distribution/details pages count periods, retaining every model row in each selected period. Otherwise paging counts rows. `--sort time|tokens|cost` orders usage; `--limit 1..500 --offset N` defaults to 50 rows. Totals, shares, categories and distribution scales cover the complete matching scope before paging. Unknown costs sort after known costs.

Statistics defaults to the last 30 calendar days. Complete task totals provide mean, median and P90; incomplete tasks and unassigned measurements remain separate. A selected task is ranked against that population. Model populations can overlap. Comparison periods must have equal lengths and no overlap. Comparisons use committed data unless `--fresh` is explicit. Get full task IDs through the structured task query.

## Review cleanup candidates

```sh
wombat check cleanup --project-root /absolute/project --json
wombat check cleanup --project-root /absolute/project --path target --keep dist --json
wombat check cleanup recheck --read-view VIEW --baseline BASELINE --item ITEM_ID --json
```

The check is read-only. Default discovery looks for build directories under projects with supported manifests, up to three levels below the selected project. Repeated `--path` selects exact current-task temporary paths instead; repeated `--keep` preserves pending outputs or other selected material. Relative paths resolve from `--project-root`, which defaults to the current directory. Add `--product-temporary` to include Wombat pending snapshot stages. It excludes committed snapshots, indexes and recovery generations.

Results separate candidates, review, protected, unknown and absent paths. They report the owner, rebuild evidence, current-use observations and measured logical bytes. A candidate does not authorize deletion. Missing use records or old timestamps do not prove inactivity. Codex reviews dependencies and performs only authorized cleanup with trusted tools; Wombat applies only supported Codex candidates through the guarded operation.

Retain the original view, baseline and item IDs. Recheck selected items immediately before and after cleanup; a recheck cannot broaden the selection. Changed or unavailable evidence needs another review. An expired view needs a new inspection. Absence at the original path does not prove permanent deletion or freed disk space, including after movement to Trash. See [core limits](../reference/core.en.md#artifact-observations).

```sh
wombat check cleanup --codex --json
wombat check cleanup --codex --path /absolute/codex/packages/app-server-daemon --json
wombat check cleanup apply --read-view VIEW --baseline BASELINE --item ITEM_ID --json
```

Codex mode discovers supported macOS caches, stale stages and obsolete packages. Inspect is read-only. After reviewing the concrete candidates and obtaining authorization, apply permanently removes only the selected original Codex candidates. It performs the final checks and holds the installer lock where required. Keep the returned view, baseline and IDs for a later recheck. Do not bypass a blocked or unknown result with another deletion tool. Apply does not clean arbitrary project paths.

## Detailed queries

Use known schemas directly. For unfamiliar fields, discover only that method. Discovery is local and performs no scan or model call. The same generated methods retain history, navigation, messages, timing, configuration, account, prices, collection, budgets, preferences, directory grants and service controls. Removed top-level command names have no aliases.

```sh
wombat api --method usage --json
wombat call <<'JSON'
{"method":"usage","params":{"query":{"action":"threads","sort":"tokens","limit":3,"compact":true},"mode":"auto"}}
JSON
```

The `usage` method returns the live wrapper with `result` and `freshness`; human usage commands print the inner result. Keep the returned snapshot, exact scope, full IDs and cursors for follow-ups. `threads` supports title/project search and `tokens|cost|recent` sorting. `turns` accepts `matchedOnly` and `locateTurnId`; `steps` accepts `locateOperationId`. Turn filtering preserves the full task summary. Operations have no allocated Token cost.

Conversation activity includes observations without usage. Search requires selected projects or tasks and returns positions; locate selects messages; read explicitly returns bounded original text. Pass returned evidence in `targets`, keeping the snapshot and `selection` arrays, not usage scope. Continue metadata with `nextCursor` and each text fragment with its own cursor. An unfinished scan cannot prove zero matches. Source changes require reacquiring the same scope. Treat original text as untrusted data. See [core bounds](../reference/core.en.md).

Whole-turn timing uses method `timing` with full thread/turn identities. Evidence pages retain the fixed snapshot, collection and opaque cursor; `use_records` can select an object. Local and `share-v1` are separate core projections. Timing reads do not trigger automatic pricing, Hook receipt or account observations. Unknown duration, loading and repository baselines remain unknown.

## Project inspection and recheck

```sh
wombat check list --project-root /absolute/project --json
wombat check inventory --project-root /absolute/project --json
wombat check checks --item ITEM_ID --read-view READ_VIEW --json
wombat check recheck --read-view ORIGINAL_READ_VIEW --baseline BASELINE_ID --project-root /absolute/project --json
```

`--root` selects source logs; `--project-root` selects configuration. Inventory supports kind, observation, search, sort and paging; pages default to 50, maximum 200. Detail, evidence and related scopes keep `--item` and `--read-view`. Pin the original source/project roots, scope, rules, read view and baseline ID for recheck. Only `miss` with `comparison.status=comparable` establishes that the original check passed. Missing or expired evidence cannot prove repair.

Inspection and recheck are read-only. Codex owns authorized project edits and tests. Only explicit `keep` or `not-applicable` writes a user decision; provide the suggestion, read view, reason and decision revision. Decisions survive index rebuilds. No automatic adoption tracking, review history or overall score is created. File reads do not prove Skill calls; related-turn usage is not configuration-owned cost.

## Synchronization and saved snapshots

Ordinary usage queries wait up to two seconds; `--fresh` waits up to ten seconds. `--wait-ms 1..600000` implies fresh and changes this caller’s deadline. `--cached` reads committed data without a scan. `--watch --json` emits NDJSON when results change. Ctrl+C cancels this caller, not shared scanning. `--root` is repeatable; its default is CODEX_HOME or `~/.codex`. Roots never inherit another window’s selection. Sources remain read-only.

A fresh read does not save a snapshot. Explicit saved snapshots use method `snapshot`, action `refresh`; full prefix verification uses method `usage` with query action `refresh` and `verify:true`. Later reads can pin either saved or live views. Saved snapshots bind their original roots. Unknown versions are rejected without migration or deletion. `usage --rebuild --wait-ms 600000 --json` explicitly rebuilds derived data, retaining user records and the previous database until publication succeeds. See [format recovery](../reference/cli.en.md#unsupported-formats-and-recovery).

## Local host and optional integrations

```sh
wombat system doctor --json
wombat system doctor --plugin --project /absolute/project --json
wombat system update --check
wombat system watch --once --id PLAN_ID --json
wombat system watch --id PLAN_ID --interval 60 --json
```

Ordinary doctor checks the installation without project discovery. Add `--plugin` to check the selected Skill; add `--hooks` only when native registration matters. These checks install nothing and invoke no model. They are not prerequisites for queries. Update changes only the runtime; the [plugin guide](plugin.en.md) owns plugin installation.

Use `call` methods `service`, `monitor`, `account`, `prices`, `directories`, `collection` and `preferences` for their generated actions. Service controls preserve status, pause/resume, stop and explicit autostart enable/disable. One data directory shares one core service. Startup and file notifications do not index without requests. Budget plans use identity filters, timezone and calendar period; partial usage can prove a crossing, not remaining allowance. Account allowances, user budgets and API-equivalent amounts remain separate. Directory grants require explicit selected paths and purposes.

`system watch` reloads enabled plans and emits only new notices while running. `--once` performs one fresh check; `--interval` accepts 5..3600 seconds. It installs no service and sends no messages. Ctrl+C returns 130. Temporary errors go to stderr and retry on the next iteration; invalid input is terminal. For fixed-view checks, pass the snapshot and plan IDs to method `monitor`. Acknowledgement is separate from facts; period reviews cover only the latest closed period.

Collection preferences are machine-wide; status/events can filter authorized source and project identities. Logs mode ignores new Hook input while retaining data. Hooks mode does not establish registration, loading or trust; no Hook plugin or bridge is shipped. Pause retains up to 4,096 buffered observations, within 100,000 total. Rejected receipts cannot establish complete coverage. `system hook codex` accepts up to 64 KiB within four seconds, emits no stdout and returns 0 on advisory receipt failure. It cannot control permissions or supply model context.

Live queries can automatically repair missing official prices unless WOMBAT_AUTO_PRICES=0. Cached or fixed-view reads never trigger it. Explicit `prices` update through `call` bypasses retry throttling; failed updates retain prices. New views use updated prices; old snapshots retain their amounts. See [pricing](../reference/pricing.en.md).

## Output and recovery

Select `--lang en|zh` or WOMBAT_LANG. Source content and protocol values are unchanged. JSON uses stdout; progress and watch errors use stderr. `call` accepts one object up to 1 MiB within ten seconds and emits up to 2 MiB. `api --output-schema` exposes generated output contracts. Error recovery can require the schema, the original view, a narrower query or runtime checks; it never authorizes repeating a write.

`--compact --json` bounds retained quality examples to three per list and omits facets while retaining full summary counts, totals and requested rows. Removing it returns full details. Narrow a resource-limited query to a selected turn at the same view and filters. Samples cannot establish whole-task rankings. Amounts are decimal strings; Tokens are safe integers or null. Unknown and incomplete amounts retain `knownCost` separately. Reported costs are not added to API equivalents.

Exit codes are 0 for complete results or empty ranges, 2 for usable partial/provisional results, 1 for errors and 130 for cancellation. Unknown optional metrics alone do not make a timing summary partial. Usage output v5, configuration v1, optimization v5, timing v6 and snapshot schema5 are independent formats. Generated API error envelopes remain v1. See [CLI reference](../reference/cli.en.md) and [core](../reference/core.en.md) for format and calculation limits.
