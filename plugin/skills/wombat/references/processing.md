# Change and recheck

Keep the selected project, original problem, target version, evidence, and success criterion. Codex owns authorized file edits, tests and recovery. Export selected check JSON and source locations when the user requests a handoff; use Codex's native task tools to create a separate conversation only when requested.

For an existing configuration finding, retain its `baselineId`, the response's `readView`, source/project roots, filters and rule overrides before editing. After the edit, run `optimize recheck --read-view VIEW --baseline BASELINE --project-root PATH --project PATH --json` with those same inputs. The baseline binds the original view, scope, finding and rule parameters. An expired view requires a fresh inspection; do not claim a before/after verification from a replacement baseline.

Read each returned check's outcome and comparison. A comparable miss verifies that the selected problem is absent under that check. A hit means it remains. Unknown, insufficient or incomparable evidence cannot establish a fix. Recheck is read-only and never changes a user decision.

For a direct project review, retain a reproducing case and rerun that case after the authorized change. Use the relevant parser, reference check or behavior test. Do not create a configuration finding for an unrelated workflow improvement.

For a usage-driven change, retain the original scope and fixed usage baseline. Use `compare` on later comparable observations when available. Describe workload and model changes; lower usage alone does not establish savings or causality. Missing later evidence does not block a useful change report.

Only when explicitly requested, record `optimize keep|not-applicable` with the selected finding ID, reason and current decision revision. Ordinary inspection and rechecks do not create handling records. Existing applicable user decisions remain in effect; index rebuilds do not delete them.
