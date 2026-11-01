# Query facts

For a simple total, use `usage --since DATE --until DATE --timezone ZONE --compact --limit 3 --json`. The start is inclusive and the end exclusive. Resolve relative dates once in the user's timezone; if no range was requested, report the returned scope. Answer from the returned totals without setup, refresh, inventory or a task ranking unless needed.

Use `usage stats` for task distributions and `usage compare` for periods or tasks. Keep core totals separate from the returned page. For rankings, navigation, original messages, timing, account or budget facts, use the existing generated method through `call`. Discover only unfamiliar input fields; output schemas are optional.

A live usage call wraps the query:

```json
{"method":"usage","params":{"query":{"action":"threads","sort":"tokens","limit":3,"compact":true},"mode":"auto"}}
```

Related statistics and rankings can use `also` when supported. Pin the returned snapshot for follow-ups and preserve the original filters. A fixed saved snapshot uses method `snapshot`; its sources are already bound. Explicit saved-snapshot refresh remains `snapshot` with `action:"refresh"` and selected roots. An ordinary fresh read does not save a snapshot.

For history, use `conversation` to search or locate, then read only relevant returned targets. Its `selection` uses arrays such as `projects`, `threads` and `turns`; it is not usage `scope`. Keep location and text cursors with their own original requests, snapshot and selection. An unfinished search cannot establish no matches. Read location and text consecutively; the optional `scripts/read-located.ts` helper exports `readLocated` for a TypeScript host. Otherwise use the same two public calls.

For resuming work, recover the decision, delivery location and next action. For a report, combine complete usage with selected work evidence. Distinguish requests, recorded actions and independently checked outcomes. A chat title is not a verified work identity; disclose sampling and uncertain links.

Use returned recovery hints. On `RESOURCE_LIMIT`, narrow historical evidence by date, task or turn while retaining the original full-scope usage totals. Label sampled coverage; do not add overlapping windows or describe a subset as complete. If a view expires, retain the selection, reacquire once and explain the version change. If evidence remains unavailable, answer the usable part and identify the deciding gap. Do not scan raw logs or repeat the same broad query to bypass limits.
