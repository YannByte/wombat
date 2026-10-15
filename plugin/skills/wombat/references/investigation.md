# Usage investigation and period review

Select the analysis that answers the question. Use a suitable snapshot. If no snapshot exists or current data is needed, use the original scoped live usage or task query. Read the relevant input Schema once. The following human CLI entries show the corresponding query actions:

For an open-ended improvement question, use the [bounded query flow](query-flow.md): batch totals and rankings, select relevant turns, then read specific evidence. Give concrete changes only after checking the selected work. Do not start by running every whole-period analysis.

| Question | Query and follow-up |
|---|---|
| What needs a check? | `investigate --snapshot ID --limit 3 --json`. Select useful candidates and their evidence. |
| Which requests are slow, failing repeatedly or declined? | Read `inspection.activity` and its current-period evidence. |
| Did failures or durations increase? | `investigate --snapshot ID --since DATE --until DATE --json`. Examine the preceding equal-length baseline for each exact request group. |
| Which repeated workflows could become a script or Skill? | `investigate --snapshot ID --project PATH --json`. Examine `recurring_workflow` across tasks and make sure that the workflow is stable. |
| Where did this week's increase come from? | `review --snapshot ID --since DATE --until DATE --json`. Use `compare` for another explicit baseline. |
| What about amount anomalies, cache creation, model choice, sensitive paths, risky requests, questions, permissions or polling? | Read `inspection.opportunities`, its status/gaps, metrics and current/previous-period evidence. |
| Why did this task's input grow? | `trajectory --snapshot ID --thread THREAD_ID --json`. Examine input changes and compaction boundaries. |
| Which resources attracted repeated work? | `resources --snapshot ID --project PATH --limit 3 --json`. Examine reads, proposed changes and observed duration. |
| What happened around this turn's retries? | `optimize activity --snapshot ID --thread THREAD_ID --turn TURN_ID --json`. Examine fixed-turn checks. |
| What context records are available? | `context --snapshot ID --thread THREAD_ID --json`. Give recorded metadata and its limits. |

Replace IDs with returned values and keep project/source/date/timezone/model/effort filters where supported. `review` accepts both date boundaries or neither, rejects `--all-time`, `--undated` and pagination, and defaults to the week containing its cutoff. `optimize activity` uses the fixed task/turn selection, retains the selected view's `--root` and optional `--source`, and rejects usage date/model filters. In the Agent API it is the nested `activity` selection of `optimize`, not a `config` action.

For each selected signal, give its measured value and scope first. Link the returned evidence. Propose a concrete next step. Use captured failure denominators, cache shares and input changes, not new estimates or thresholds.

Examine evidence before proposing a workflow as a command or Skill. Candidate signals and configuration findings are separate. Only configuration suggestions have suggestion IDs for [processing](processing.md). Do not fabricate a candidate when no supported signal exists.

For a selected turn's duration use [timing](timing.md). Recorded Token input is not context occupancy or a tool's cost.

Activity groups complete safe request identities within one source and recorded project. An unknown project cannot join different tasks. Historical checks require both dates and keep source/project/model/effort filters in a preceding disjoint window. Missing fingerprints or insufficient baseline samples do not prove healthy operations.

Findings keep up to 30 groups and return the complete count. Narrow the original scope for more groups. Read current and baseline evidence with their own dates. Keep the same snapshot and authorized sources.

Recorded rejections are declined operations, not permission-prompt counts. The full approval conversation remains unavailable.

Additional checks keep complete counts and at most three findings each. Positive findings can coexist with gaps. Native question results can identify omitted answers. Missing responses remain unknown.

Permission checks count explicit request_permissions calls. Request-to-response intervals do not isolate user waiting. Security results contain categories and locations only. Consult original content only within authorization.

Model/cache clues invite comparison, not automatic changes or savings claims.

On RESOURCE_LIMIT, distinguish output size from computation. `--compact` bounds quality detail. `--limit` bounds returned rows. Neither flag reduces whole-task event work.

At the same snapshot, use `turns --thread THREAD_ID --matched-only --sort tokens --limit 3 --compact --json` with original filters. Then use `investigate --thread THREAD_ID --turn TURN_ID --snapshot ID --compact --json` with those filters. This reads one turn and relevant control evidence. Show selected-turn findings as examples, not whole-task rankings.

If one turn exceeds budget, use its public timing summary or `optimize activity` where supported and show the gap. Stop repeated failing analyses. Do not dump raw logs or reconstruct accounting in the conversation.
