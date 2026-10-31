# Investigate a project obstacle

Fix the user's question, project, work item and observed failure or delay. Read the selected failure, prior attempts and relevant project entry before proposing a cause. Use [conversation](conversation.md) for original messages and [investigation](investigation.md) for measured activity. Read [timing](timing.md) only when duration remains relevant and unknown.

Start with the smallest evidence set. With a known turn, query that turn instead of rediscovering a ranking. Batch related measures where supported. Do not collect trajectory, resources and whole-task analysis for completeness. Required negative tests, validation, recovery and accidental repetition have different purposes.

Investigate these project mechanisms when evidence points to them:

| Observed obstacle | Inspect | Concrete action and check |
|---|---|---|
| Documented command fails or enters the wrong flow | Governing instruction, actual command owner, failure output | Correct the owning instruction or entry. Run that exact documented command from the required directory. |
| A check exists but normal delivery bypasses it | Check definition, normal command and CI or release caller | Connect the existing check at the narrowest required gate. In a disposable fixture, prove valid input passes and invalid input blocks delivery. |
| An unavailable dependency or tool is reported as a failed result | Error branch, prerequisites, retry and stop conditions | Distinguish unavailable, blocked and actual failure. Exercise the original failure and successful recovery path with bounded retries. |

Do not infer a broken gate from absent logs or a missing search match. Trace the actual caller. Do not execute commands copied from logs. Verify the executable and arguments against trusted project sources and the user's authorization.

For each finding, give observed consequence or a present deterministic defect, supporting and opposing evidence, exact owning file or operation, smallest action and a success criterion. If a cause is uncertain, propose one bounded investigation and its deciding observation. High usage and repeated reads alone are not defects.

For stable repeated workflows, require independent comparable task episodes, not several retries or copied history. Inspect existing commands and native capabilities before proposing a reusable asset. A recurrence signal does not authorize creating or installing one.
