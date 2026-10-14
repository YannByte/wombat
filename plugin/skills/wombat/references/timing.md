# Turn duration and timing evidence

For a selected turn's duration, use `timing` directly with full Wombat task and turn IDs. If the IDs are not known, locate the task and turn using [usage](usage.md). Do not guess a native ID or the current conversation. Broad repeated-failure or slow-request questions use [investigation](investigation.md), then timing for selected evidence.

After replacing the placeholders with returned IDs and a suitable snapshot:

```json
{"method":"timing","params":{"action":"summary","threadId":"RETURNED_THREAD_ID","turnId":"RETURNED_TURN_ID","snapshotId":"RETURNED_SNAPSHOT_ID","privacyProfile":"local","mode":"cached"}}
```

Timing bypasses automatic prices, configuration, Hook capture and account reads. Keep its returned scope/version and value statuses. Native duration, derived duration, native TTFT and first-content delay are different observations. Interval unions and sums can overlap.

Do not add categories into an invented elapsed total. Unknown optional values remain unknown even when the inspected scope is complete. A reported path or failed operation does not prove a file change or a code defect.

Read selected evidence at the same snapshot with `timing evidence --thread ID --turn ID --snapshot ID --limit 20 --json`. Use returned opaque cursors and the same collection. Read the Schema before selecting `use_objects`, `use_records` or an object filter. Human `timing --text` provides localized summaries.

If the user requests sharing, use `--share` for the core's safe projection. Timing rejects usage date/model/cost filters. With resource limits, native scalar observations can remain available while derived calculations are unavailable. Show that distinction.
