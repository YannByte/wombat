# Bounded usage and evidence flow

Use this flow for usage improvement questions or a work review that needs original evidence. Keep a simple totals question to one query.

1. Resolve the requested dates, timezone and filters. Start with a compact usage query. Batch only needed rankings through `also`; totals come from `summary`, never the returned page. With a ready background index, use `mode:"cached"` when its recorded cutoff is adequate. Otherwise use the normal readiness flow.
2. Select a task from the returned ranking. Query its matching turns with `matchedOnly:true` and the original filters. Use `method:"usage"`, `mode:"cached"` and `query.snapshotId` for a returned `live:` ID. `method:"snapshot"` addresses saved disk snapshots; omit `roots` because that snapshot already binds its sources.
3. For a specific candidate, inspect its exact task and turn. Do not start with a whole-day deep analysis when bounded evidence can answer the question. On `RESOURCE_LIMIT`, narrow once to that selection. Smaller pages cannot reduce whole-task computation.
4. Execute dependent `conversation` location and bounded reads consecutively in one host invocation. Use [read-located.ts](../scripts/read-located.ts) when the host can import TypeScript. It needs a caller that uses the selected CLI, sends one generated request, accepts exits 0/2 and parses the validated JSON. Do not install a runtime for this helper; the same two public calls work in other hosts.

The helper accepts the generated locate/search request unchanged except for a bounded page limit. Pass the returned snapshot, original selection and optional cursor. Optional arguments control bytes per target and target count; defaults are 8 KiB and three. It returns the original location and read responses separately, preserving partial coverage and both kinds of continuation. If no readable location is found, the second result is null. A pending cursor is not proof of no matches. Continue only when the question needs more evidence, using that response's original snapshot and selection.

Conversation `selection` differs from usage `scope`. Use its generated fields: `projects`, `sources`, `threads` and `turns` are arrays. Do not copy the usage scope object into selection. Keep usage model/effort filters when selecting tasks and matching turns; conversation selection cannot filter messages by model or effort. Evidence within a selected turn does not allocate usage to a message.

Keep the CLI caller's timeout, cancellation and output bounds. The helper makes at most two read-only calls, does not refresh or retry, and propagates errors. On expiry, retain the selected task/turn identities and original scope. Tell the user about the new view, reacquire it once, then rerun location and reading consecutively. A second expiry is a reported gap, not permission to scan logs or change storage.

This is a synthetic batch example. Replace its date bounds and timezone with the user's scope. Copy supported request fields from the discovered Schema.

```json
{"method":"usage","params":{"query":{"action":"usage","scope":{"since":"2026-01-02","until":"2026-01-03","timezone":"Asia/Shanghai"},"compact":true,"limit":1},"also":[{"action":"usage","presentation":"models","scope":{"since":"2026-01-02","until":"2026-01-03","timezone":"Asia/Shanghai"},"compact":true,"limit":3},{"action":"threads","sort":"tokens","scope":{"since":"2026-01-02","until":"2026-01-03","timezone":"Asia/Shanghai"},"compact":true,"limit":3}],"mode":"cached"}}
```

Give recommendations as: observed step, proposed change, and a check for correctness or effectiveness. Read selected evidence before naming a step to remove. Separate expected negative tests from accidental failures. High usage, cached reads and a costly model alone do not prove waste. Do not calculate operation costs or promise savings without a comparison.
