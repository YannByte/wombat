# Process and recheck

For a Web handoff, preserve supplied cwd, target IDs/paths, hashes/existence, selectionVersion, readView and decisionRevision. Verify scope and current file state before editing. Changed targets require renewed review; do not substitute a reordered “first” item or expand to the whole machine.

Process within the user's authorization in the receiving conversation. Preserve purpose, triggers, necessary constraints, tests, references and shared projects. Codex owns file edits, review and recovery. Wombat has no apply/recover job API. Do not send another handoff to yourself.

After editing a configuration suggestion, call `optimize recheck --suggestion ID --project-root PATH --project PATH --json` with current evidence; inspect per-rule outcomes. A passing static recheck is not proof of actual adoption, better task quality or financial savings. Report residual findings and uncheckable rules. Keep user decisions separate from checks.

Use the recheck response directly. A resolved suggestion may leave the pending list; if more history is needed, query `optimize history --suggestion ID` with the original scope and ID. Do not query the old suggestion as a new pending detail, select a replacement, or repeat the recheck just to retrieve the same facts.

For changes prompted by a usage investigation, retain the original task, scope and baseline metrics. Report the actual change and compare relevant later observations when available, using `compare` or the original analysis. Do not invent a configuration suggestion ID or require a static recheck for an unrelated workflow change. Describe observed improvement separately from a causal explanation; missing follow-up data does not block a useful change summary. Do not run extra model tasks just to manufacture a benefit measurement.

If the user explicitly chooses “keep” or “not applicable”, use `optimize keep|not-applicable --suggestion ID --reason necessary|object_changed|incorrect_evidence` with the reviewed readView/decisionRevision and scope. Follow current help for exact arguments. Rechecks must not revoke those decisions. Do not create consuming tasks just to test adoption.

Only use `optimize handoff` when the user asks to send to a separate Codex task. Preview first, bind the reviewed selection version, then send. Accepted, failed and unknown delivery are distinct; never automatically resend an unknown delivery. Native allowance checks apply at send time.

Follow-up `read_content_matched` means a reliably targeted native read result matches the saved recheck's content hash; `loaded_content_matched` means a supported native instruction-load record matches it. Keep those distinct from identity-only use, successful MCP use and effective behavioral adoption. `usageComparison` compares equal elapsed windows before and after the recheck within its exact source/project scope, with task distributions, ledger totals and missing-date coverage. Present it as descriptive before/after evidence. Changed workloads or model mixes can explain differences; neither version matches nor lower usage establish causality or savings. Missing comparison evidence does not block reporting the edit and static recheck.
