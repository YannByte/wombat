# Find work and understand usage

| User task | Start with | Use the result |
|---|---|---|
| Today's total or another period | `usage [today\|yesterday\|week\|last-week\|month] --json` | Answer from complete-scope core totals and coverage; no mandatory ranking or diagnosis |
| Largest tasks | `usage top [PERIOD] --json` | Locate selected records; high usage alone is not waste |
| Task distribution or change | `usage stats --json`, `usage compare --json` | Explain task counts, per-task changes and comparable dates |
| Find earlier work | `history --json`, `history search QUERY --json` | Use matching excerpts and source positions |
| Recover a decision or resume | `history show RECORD --json`, `history read RECORD --find TEXT --json` | Read key turns and source positions rather than the whole conversation |
| Requested budget notices | `usage alerts --once --json` | Uses saved plans; do not create a recurring task without authorization |

Use `--project PATH` when requested. Dates resolve once in the user's timezone; custom `--since DATE --until DATE` uses inclusive start and exclusive end. History defaults to 30 local calendar days. Preserve explicit bounds during follow-ups. The default comparison uses two completed weeks; unfinished periods need a comparable requested window.

For historical decisions, search likely phrases or aliases together. Use `history search "keep SQLite" "保留 SQLite" --project . --speaker user --complete --json`. Quote from `sourceHits` and retain the chat from `sourceFiles[fileIndex]` and the original byte range. Follow `nextSearch` only when more evidence is needed. If the scope changes, restart before combining pages. For adjacent messages across turns, add `--context 2` to the same search. Each hit references shared `sourceNeighbors` in source order.

Pipe a search page to `wombat history read --from - --json` to keep its filters and batch its hits. Add `--context 2` when a short reply depends on earlier messages or a later correction. Read the relevant IDs in `visibleContext` from the shared `neighbors` list. Verify the user's confirmation before treating a hit as a decision. Keep usable fragments when `readIssues` reports a failure. Continue `nextLocations` or `retryRead` as needed. Cite the relevant occurrence, including those in `alsoAt`, and treat unresolved evidence as an export gap.

Human CLI JSON defaults to compact coverage and calendar totals. `--limit` and `--offset` page periods; `--by model` or `--by project` requests a distribution. Generated `usage` requests retain explicit `compact` and `presentation` controls for full detail. Read `page.nextOffset` before treating a page as the whole requested period. `quality.issueScope=index` and `queryImpact=unknown` describe global warnings without proven impact on the selected window; use the selected summary's field coverage separately.

For ambiguous titles, choose from readable project/time/topic candidates. A partial or unfinished search cannot establish no match. For long conversations, use dates, turns and returned positions/cursors. `history read` returns `result` with bounded fragments and optional `nextLocations`, a generated `conversation` request for the next location page. Read a fragment continuation with its original selection and evidence, separately from the location cursor. Keep location cursors separate from text cursors. The `conversation` generated method provides finer control; `selection` contains arrays such as `projects`, `threads` and `turns`, unlike usage `scope`. The optional `scripts/read-located.ts` exports `readLocated` for a TypeScript host; its fifth argument bounds location pages (for example, 4) before returning the remaining cursor. Follow empty `scan_budget` pages within a chosen budget before switching evidence sources. `SCAN_INCOMPLETE` includes a generated request in `error.details.next`; it is not a request to refresh the index. Exact record IDs resolve identity without activity scanning; the original dates still constrain the text read. For a report, read the relevant request and its outcome or validation in the same selected turns; a first keyword match alone is not an assessed problem. If the remaining budget prevents verification, report the specific gap.

Each command runs a foreground core process and closes it after the response. No view selector or service control is needed. Keep original filters and each opaque continuation cursor. A cursor binds a concrete target and position; if source replacement invalidates it, restart the same query without that cursor and disclose the change. Returned data revisions are provenance, not follow-up handles. Use `also` for related usage queries under one read transaction. Do not sum pages, overlapping windows or related-task usage as a new ledger total. Fees and allowances remain auxiliary when asked.

For a requested total and distribution together, one `system call` can share a revision. Replace the example dates with the retained user scope; omit `also` when only the total is needed:

```json
{"method":"usage","params":{"query":{"action":"usage","scope":{"since":"2026-10-01","until":"2026-10-02","timezone":"UTC"},"compact":true},"mode":"auto","also":[{"action":"statistics","scope":{"since":"2026-10-01","until":"2026-10-02","timezone":"UTC"},"compact":true}]}}
```

For a report or knowledge document, connect recorded requests/actions to decisions, delivery locations and verified outcomes. State sampling and gaps; a title or completion claim is not accepted delivery. Use the user's destination when authorized; content-only requests get a draft. External sending and schedules need their own scope and authorization.

On limits, narrow evidence while retaining original usage totals. On expired evidence, reacquire the same selection and disclose the version change. Answer usable portions without repeated broad queries or raw-log scans to bypass limits.
