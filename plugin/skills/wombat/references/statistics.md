# Task distributions and growth

Use complete core statistics instead of calculating distributions from a task page. Keep the selected project/source/date/timezone/model/effort scope and a suitable fixed snapshot. If no suitable view exists or current data is needed, use the original scoped live usage or task query. Do not refresh separately before analysis.

| Question | Human CLI entry |
|---|---|
| Is this task unusually large? | `statistics --snapshot ID --thread THREAD_ID --since DATE --until DATE --json` |
| Did task count or usage per task increase? | `statistics --snapshot ID --since DATE --until DATE --baseline-since DATE --baseline-until DATE --presentation projects --json`. Use `--presentation models` for model groups. |
| Which project or model contributed to the change? | Use the returned complete contributions. `compare` supports another explicit period comparison. |

Replace IDs and dates with the selected values. Use the discovered `snapshot` or live `usage` method for structured requests. Compare `selectedTask` with the complete population. Keep incomplete and unassigned usage separately.

Mean, median and P90 refer to the chosen recorded-task population. A high percentile is not a quality judgment. Model groups can overlap, so do not sum their task counts. Growth contributions show arithmetic change, not its cause.

For period findings and possible causes use [investigation](investigation.md).
