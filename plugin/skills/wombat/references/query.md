# Find work and understand usage

| User task | Start with | Use the result |
|---|---|---|
| Today's total or another period | `usage [today|yesterday|week|last-week|month] --json` | Answer from complete-scope core totals and coverage; no mandatory ranking or diagnosis |
| Largest tasks | `usage top [PERIOD] --json` | Locate selected records; high usage alone is not waste |
| Task distribution or change | `usage stats --json`, `usage compare --json` | Explain task counts, per-task changes and comparable dates |
| Find earlier work | `history --json`, `history search QUERY --json` | Retain the record, snapshot, selection and continuation cursors |
| Recover a decision or resume | `history show RECORD --json`, `history read RECORD --find TEXT --json` | Read key turns and source positions rather than the whole conversation |
| Requested budget notices | `usage alerts --once --json` | Uses saved plans; do not create a recurring task without authorization |

Use `--project PATH` when requested. Dates resolve once in the user's timezone; custom `--since DATE --until DATE` uses inclusive start and exclusive end. History defaults to 30 local calendar days. Preserve explicit bounds during follow-ups. The default comparison uses two completed weeks; unfinished periods need a comparable requested window.

For ambiguous titles, choose from readable project/time/topic candidates. A partial or unfinished search cannot establish no match. For long conversations, use dates, turns and returned positions/cursors. `history read` returns `result` with bounded fragments and optional `nextLocations`, a generated `conversation` request for the next location page. Read a fragment continuation with its original selection and evidence, separately from the location cursor. Keep location cursors separate from text cursors. The `conversation` generated method provides finer control; `selection` contains arrays such as `projects`, `threads` and `turns`, unlike usage `scope`. The optional `scripts/read-located.ts` exports `readLocated` for a TypeScript host.

Pin the returned view for follow-ups. A generated live `usage` call wraps `query` and `mode`; saved views use `snapshot`. An ordinary fresh read does not save a snapshot. Related queries may use `also` to share a revision. Do not sum pages, overlapping windows or related-task usage as a new ledger total. Fees and allowances are auxiliary when asked, not subscription bills or inferred balances.

For a requested total and distribution together, one `system call` can share a revision. Replace the example dates with the retained user scope; omit `also` when only the total is needed:

```json
{"method":"usage","params":{"query":{"action":"usage","scope":{"since":"2026-10-01","until":"2026-10-02","timezone":"UTC"},"compact":true},"mode":"auto","also":[{"action":"statistics","scope":{"since":"2026-10-01","until":"2026-10-02","timezone":"UTC"},"compact":true}]}}
```

For a report or knowledge document, connect recorded requests/actions to decisions, delivery locations and verified outcomes. State sampling and gaps; a title or completion claim is not accepted delivery. Use the user's destination when authorized; content-only requests get a draft. External sending and schedules need their own scope and authorization.

On limits, narrow evidence while retaining original usage totals. On expired evidence, reacquire the same selection and disclose the version change. Answer usable portions without repeated broad queries or raw-log scans to bypass limits.
