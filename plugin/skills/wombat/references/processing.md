# Process and recheck

Before editing, retain the selected project and work item, original issue, evidence references, target content version, success criterion and baseline measures when relevant. Check that current target content still matches the reviewed version. A changed target requires renewed review within the authorized scope.

Process within the user's authorization in the receiving conversation. Keep purpose, triggers, necessary constraints, tests, references and shared projects. For a narrowly authorized edit, keep other source files. Do not rewrite them just to restate the result.

Codex owns file edits, review and recovery. Wombat has no apply/recover job API.

After editing an existing configuration suggestion, call `optimize recheck --suggestion ID --project-root PATH --project PATH --json` with current evidence. Examine per-rule outcomes. For a direct file review without a Wombat suggestion ID, check the original target and issue with an appropriate parser, reference check or behavior test. Do not create a suggestion merely to report a fix. A passing static recheck is not proof of actual adoption, better task quality or financial savings. Show remaining findings and unavailable checks. Keep user decisions separate from checks.

Use the recheck response directly. A resolved suggestion may leave the pending list. If more history is needed, query `optimize history --suggestion ID` with the original scope and ID. Do not query the old suggestion as a new pending detail. Do not select a replacement or run the recheck again to retrieve the same facts.

For changes prompted by a usage investigation, keep the original task, scope and baseline metrics. Give details of the actual change. If later observations are available, compare them with `compare` or the original analysis. Do not invent a configuration suggestion ID or require a static recheck for an unrelated workflow change.

For a confirmed defect, retain a reproducing case before editing when it can be exercised safely. After the final edit, run that same case and the relevant regressions; check both rejection of invalid work and continued acceptance of valid work. Report the actual final version and outcomes. A controlled replay verifies a mechanism without proving that a complete task improved. Use later comparable work to assess behavior, quality or usage. Missing follow-up data does not block a useful change summary; do not create tasks just to manufacture adoption or savings.

If the user explicitly chooses “keep” or “not applicable”, use `optimize keep|not-applicable --suggestion ID --reason necessary|object_changed|incorrect_evidence` with the reviewed readView/decisionRevision and scope. Read current help for exact arguments. Rechecks must not revoke those decisions. Do not create consuming tasks just to test adoption.

If the user asks to send to a separate Codex task, use `optimize handoff`. Preview the handoff. Bind the reviewed selection version. Then send the handoff.

Accepted, failed and unknown delivery are distinct. Never automatically resend an unknown delivery. Native allowance checks apply at send time.

Follow-up `read_content_matched` means that a reliably targeted native read result matches the saved recheck's content hash. `loaded_content_matched` means that a supported native instruction-load record matches that hash. Keep those distinct from identity-only use, successful MCP use and effective behavioral adoption. `usageComparison` compares equal elapsed windows before and after the recheck in its exact source/project scope.

The comparison includes task distributions, ledger totals and missing-date coverage. Present it as descriptive before/after evidence. Changed workloads or model mixes can cause differences. Neither version matches nor lower usage establish causality or savings.

Missing comparison evidence does not block reporting the edit and static recheck.
