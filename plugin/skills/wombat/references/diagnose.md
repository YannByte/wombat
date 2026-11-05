# Diagnose the project

| User task | Start with | Next decision |
|---|---|---|
| Explain development obstacles or improve Agent work | Project/date-scoped `history search`, then bounded `history read` | Recover the request, failure and subsequent handling before selecting execution analysis or current-code checks |
| Diagnose this project | `check --project PATH --json` | Use separate checks, configuration, recent-work and artifact sections to select consequential paths |
| Investigate recorded repeated work | `check --task RECORD --turn TURN --json` | Inspect the original failure, selected turns and current owner/caller |
| Investigate a known current defect | Native project reads | Trace the reachable trigger and existing protections; query history only if it could change the judgment |
| Inspect unfamiliar detailed evidence | `system api METHOD --json`, then `system call` | Select only the needed generated operation; preserve scope and identity |

A diagnosis-only request remains read-only for project files and enablement. Wombat may prepare its derived data on demand. Installation, builds, services or tests with writes/external effects belong in the proposed operations unless existing authorization already covers them. Missing history limits historical judgments without blocking current-code inspection.

For open diagnosis, proactively trace important execution, validation or recovery paths from actual project entries and recent problems. Inspect the real owner, callers and tests, including evidence that could disprove a finding. Do not stop at configuration size reminders or assume a named check tested the right behavior. A deterministic current defect needs no recorded incident.

`check --task` inspects a complete selected turn and rejects date filters. For date-bounded evidence, retain dates in generated `conversation` selection instead.

Treat failures, repeated reads/requests, input growth and waits as leads. Exclude expected negative tests, required validation and reasonable retries. Keep current configuration separate from historical loading. Report sections retain their own gaps; shared indexed facts do not establish a whole-code audit or completed delivery.

For development history, group evidence by the user's request, corrections and explicit parent/child task links. Include related turns and review feedback; retain changed scope and authorization. Titles or nearby timestamps alone do not establish one task. Fold linked review chats into their parent investigation; keep standalone or unlinked reviews distinct.

Read the original error and the next relevant action, not just a keyword match or failure count. If matching is insufficient, distinguish missing identity, conflicting endpoints and excluded receivers, then read the located request/result directly. A missing repeat chain does not make the original failure uninspectable. An opaque tool response proves that a result was returned, not that the underlying action succeeded or failed; read it before deciding what the result means. Check expected test outcomes, changed files, historical cwd/version/configuration and existing safeguards before diagnosing a defect. Do not reconstruct historical conditions from current settings.

Use `history read RECORD --turn TURN --tools --find TEXT --json` to locate original diagnostic tool output, then continue only the selected fragment if needed. This explicit path includes locators for failed, interrupted, declined and unknown results without filling ordinary chat results with tool logs. If the original record exists but parsing or association cannot explain it, report a Wombat parsing/association gap with its position rather than claiming the user lost or deleted history.

Inspect returned `operationDiagnostics` before treating conflicting operations as missing evidence: each entry gives the actual reducer's reason codes, operation identity and sampled original positions. The count covers the selection; bounded samples disclose completeness. A request error's field path and schema command describe which input to fix. Keep the original thread, turn, source and collection when continuing; do not replace the cursor or restart a broader search to hide an error.

Complete the useful chain: task and expected result → failed step and original error → subsequent handling and final state → supported cause and alternatives → current owner/minimal change → original verification. When the action was a valid negative test or the existing recovery worked, explain why no repair is needed. If a link is missing, name that link and the smallest next evidence read or check; do not replace the investigation with a generic input-growth warning. Editing, passing a test and resolving the original failure remain separate conclusions.

Combine signals from one mechanism in one explanation with their source positions retained. Prioritize consequence, evidence and feasible action without a score or finding quota. For each worthwhile action, state trigger/consequence, confidence, exact file/operation, smallest correction, behavior to preserve and original case to repeat. Prefer an existing validator, gate or recovery mechanism before adding another policy, Skill or service.

Separate confirmed defects, bounded investigations and user choices. Complete a permitted bounded investigation when it can decide the cause. Do not turn a hypothesis, low use or an evidence gap into a repair claim. If no supported improvement emerges, state inspected scope and important limits.

Present a reviewable selection using report-local numbers. Include affected objects, exclusions, dependencies, validation and recovery; disclose global effects or permanent deletion. Wait for selection for diagnosis-only requests. Existing explicit edit authorization remains valid. “Execute all” covers the displayed executable plan, not hypotheses, undisclosed effects or later findings. Use [Execute and verify](verify.md) after selection; [AI environment](environment.md) and [Cleanup](cleanup.md) supply their specific evidence rules only when relevant.
