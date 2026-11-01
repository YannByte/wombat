# Apply selected improvements and verify

| Selected change | Execution | Verification |
|---|---|---|
| Code or workflow | Codex edits the actual owner using existing project mechanisms | Repeat the original case and relevant project tests, including a meaningful failure/recovery case |
| Configuration finding | Codex uses authorized native controls or the configuration owner | `check verify --read-view VIEW --baseline BASELINE --project PATH --json`, retaining all original roots/source/rule inputs |
| Skill or MCP choice | Apply only the selected native project/global change | Reinspect the same identity/scope and test capabilities the user still needs |
| Supported Codex cleanup | Guarded `clean --apply` with the original selected IDs | `clean verify` on that selection; inspect per-item outcomes |
| Project cleanup | Native tools on rechecked authorized exact paths | Recheck original candidates and retained deliverables without rebuilding every removed artifact |

Recover criteria from the user request and project instructions. Before editing, retain the original problem, target version, inputs and relevant check/view/baseline. Check current files and uncommitted work. Preserve unrelated edits; report-local numbers authorize only their displayed objects and effects. Reuse explicit authorization rather than asking again at every step.

Combine changes sharing a file and respect dependencies. Artifacts needed for tests are cleaned last. A failed step stops dependent operations; independent selected actions may continue. Report new findings without adding them to the selection. Cancellation stops subsequent work; already completed or uncertain actions remain explicit. Before retrying a write, inspect actual state instead of repeating the entire batch.

Configuration rechecks use the original roots, project/source filters and rule overrides. A comparable miss establishes only that check's absence; a hit remains. Missing, expired or incomparable evidence cannot prove repair. Reinspect expired views without reconstructing an old comparison. “Keep” and “not applicable” remain explicit user decisions, not repair results or automatic recheck changes.

For extension changes, retain exact path/identity, native state, project/source/date scope and estimate method. Reinspect current state with the same scope and a fresh view, comparing selected identities to the retained before view. Historical use counts do not reset after disabling. Restore only selected values without overwriting later unrelated edits. Workspace-wide resets and blanket cache deletion are not recovery.

Report each selected action: what changed, original-problem result, passed/failed/unrun/unavailable checks and retained material. Editing, build success or command exit alone does not verify behavior or user acceptance. Tests without an actual release do not establish publication success. Catalog reductions are estimates, not confirmed context savings.

Immediate repair and later improvement are distinct. Compare naturally occurring later work only with comparable scope, workload, methods and quality. Lower usage alone cannot establish causality. Do not create demonstration tasks, automatic adoption history, execution receipts or another tracking system. Save a report or decision only when requested.
