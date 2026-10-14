# Token budgets and recurring checks

Use the generated `monitor` method Schema, or `monitor --help` without the Agent API. For a review or change of existing plans, start with `monitor list --json`. If the user requests a plan, save it with the user's threshold, calendar period, timezone and scope. Ask for missing values that determine the plan. Do not invent a threshold. Budget plans are distinct from [account allowance](account.md).

`monitor check --id PLAN_ID --json` obtains a recent view and reports new crossings and the latest closed-period review. The Agent `check` action takes `snapshotId` and plan `ids`. Obtain a suitable recent usage view for the plans' scope when needed. Listing plans needs no usage query.

`monitor watch --id PLAN_ID --interval 60 --json` checks only while that command runs. For ongoing local monitoring, tell the user about this lifetime. It does not install a schedule or notify outside the process.

Repeated checks deduplicate notices. Acknowledgement records a user choice separately. Show partial crossings as lower-bound facts, not a known remaining budget. Keep the selected plan's identity and scope on follow-ups. Viewing plans or notices does not authorize setting, removing or acknowledging them.
