# Usage and task navigation

Start with the original scope and a small compact query. In the Agent API, live `usage` wraps `query`, `mode` and `verify`. Fixed `snapshot` takes the query directly. For human CLI calls, use `usage --compact --limit 3 --json`.

If unsupported, omit `--compact`. A live query prepares data as needed. Do not refresh or scan separately before each task.

Resolve “today/yesterday” and relative periods using the user's local date and timezone, not display language. Pass the timezone explicitly. The CLI default is UTC. `--since` includes its date and `--until` excludes it.

Respect explicit ranges, offline requests and fixed views. With no requested or retained range, use the CLI's default recent range and show the returned dates. Filters include project, source, model and effort. A source root is not a project root.

For “where did it go”, select `projects`, `models`, `distribution` or `details` presentation and `threads --sort tokens|cost --limit 3 --compact --json`. Complete-scope totals come from the returned overview, not the current page. Keep Token categories, unpriced coverage and unassigned usage. For “is this unusually high” or period growth, use [statistics](statistics.md). For possible causes use [investigation](investigation.md).

For named tasks, use `threads --search TEXT`. For exact IDs, use `--locate-thread ID`. Pin the returned snapshot for follow-up pages. Use `turns --thread ID --sort tokens --limit 3 --json` at that snapshot.

For a report that needs several views, check whether the discovered live usage Schema supports `also`. Put the overview in `query` and up to eight required statistics or ranking requests in `also`, each with the intended filters, `compact:true` and a small limit. Additional queries cannot select roots or snapshots, or refresh. Use `result` and `additional` from one response; all share a revision. Do not synchronize each dimension separately. `historyDeferred` means provisional coverage; retain that limitation and load full history only when the requested range requires it or the user asks.

For a selected turn, use `--locate-turn ID`. For the turn's records, use `steps --thread ID --turn ID --json`. Pinned CLI queries cannot combine `--snapshot` with `--root` or `--fresh`. The snapshot already binds sources.

Keep project/source-ID/date/model/effort filters. An exact project cwd excludes other worktrees. Include related worktrees or descendants only when requested with their explicit identities.

Use `page.nextOffset`. Do not guess offsets or read fresh rankings. Turns cover the whole task by default. If the question needs only matching turns, use `--matched-only`.

Keep matched usage versus full-task usage and their denominators. A later “that project” uses the reliable cwd from the selected task. Missing text or attribution stays unknown. Operation associations do not allocate an independent fee.

Compact JSON keeps scope totals, snapshot, requested page and coverage. `quality.detailSummary` counts all issues and sources. `omitted*` fields mark omitted detail. Retained examples do not prove complete coverage. Request full detail only for a selected issue. Use [startup](startup.md) for partial or pending data and [investigation](investigation.md) for resource-limited analysis.
