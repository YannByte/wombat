# Diagnose the project

| User task | Start with | Next decision |
|---|---|---|
| Diagnose this project | `check --project PATH --json` | Use separate checks, configuration, recent-work and artifact sections to select consequential paths |
| Investigate recorded repeated work | `check --task RECORD --turn TURN --json` | Inspect the original failure, selected turns and current owner/caller |
| Investigate a known current defect | Native project reads | Trace the reachable trigger and existing protections; query history only if it could change the judgment |
| Inspect unfamiliar detailed evidence | `system api METHOD --json`, then `system call` | Select only the needed generated operation; preserve scope and identity |

A diagnosis-only request remains read-only for project files and enablement. Wombat may prepare its derived data on demand. Installation, builds, services or tests with writes/external effects belong in the proposed operations unless existing authorization already covers them. Missing history limits historical judgments without blocking current-code inspection.

For open diagnosis, proactively trace important execution, validation or recovery paths from actual project entries and recent problems. Inspect the real owner, callers and tests, including evidence that could disprove a finding. Do not stop at configuration size reminders or assume a named check tested the right behavior. A deterministic current defect needs no recorded incident.

`check --task` inspects a complete selected turn and rejects date filters. For date-bounded evidence, retain dates in generated `conversation` selection instead.

Treat failures, repeated reads/requests, input growth and waits as leads. Exclude expected negative tests, required validation and reasonable retries. Keep current configuration separate from historical loading. Report sections retain their own gaps; shared indexed facts do not establish a whole-code audit or completed delivery.

Combine signals from one mechanism in one explanation with their source positions retained. Prioritize consequence, evidence and feasible action without a score or finding quota. For each worthwhile action, state trigger/consequence, confidence, exact file/operation, smallest correction, behavior to preserve and original case to repeat. Prefer an existing validator, gate or recovery mechanism before adding another policy, Skill or service.

Separate confirmed defects, bounded investigations and user choices. Complete a permitted bounded investigation when it can decide the cause. Do not turn a hypothesis, low use or an evidence gap into a repair claim. If no supported improvement emerges, state inspected scope and important limits.

Present a reviewable selection using report-local numbers. Include affected objects, exclusions, dependencies, validation and recovery; disclose global effects or permanent deletion. Wait for selection for diagnosis-only requests. Existing explicit edit authorization remains valid. “Execute all” covers the displayed executable plan, not hypotheses, undisclosed effects or later findings. Use [Execute and verify](verify.md) after selection; [AI environment](environment.md) and [Cleanup](cleanup.md) supply their specific evidence rules only when relevant.
