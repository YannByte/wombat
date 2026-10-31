## Establish the review boundary

Identify the requested commit, branch comparison, PR, files, or working-tree changes. State the baseline and included staged, unstaged, and untracked files. Use the user-specified base; otherwise inspect branch tracking and the merge base before choosing a comparison. Do not treat unrelated existing work as part of the change. For a whole-project review, state the covered modules and prioritize their public entries and cross-module paths rather than claiming exhaustive coverage.

Read the diff, affected implementation, callers, and tests before forming findings. Search relevant decisions by the changed capability, invariants, and paths rather than reading every record. Separate regressions introduced by the change from existing defects and intentionally unfinished proposals. If intent is unclear, investigate available evidence and identify the unresolved assumption instead of declaring a design preference a defect.

A review request authorizes inspection and relevant checks, not automatic source edits, commits, publication, or PR comments. When fixes or documentation maintenance are also requested, perform them within that scope, preserve unrelated work, and review the resulting diff again. Do not require fresh permission for work already authorized.

## Review implementation and evidence

Trace affected behavior from the real CLI entry through generated contracts to the Rust owner and back. Inspect alternate callers, not only the happy path. Use the code conventions as the checklist; prioritize observable consequences over formatting preferences.

| Changed area | Review focus |
|---|---|
| Module or API boundaries | Correct business owner, public imports, current consumer, generated DTO changes and all consumers, CLI/non-TTY delivery |
| Accounting or source evidence | Independent truth, hierarchical conservation, identity/replay handling, missing versus zero, pricing precision, partial failures, current versus historical evidence |
| State, storage, or async work | Atomic publication, failure recovery, user-record preservation, cancellation ownership, late responses, version isolation, bounded cleanup |
| Host or untrusted input | Authorization and version enforcement at execution, path scope, explicit process arguments, complete output/resource limits, sensitive-data handling |
| Skill or user operations | Shared business semantics, locale ownership, scope/selection retention, visible unknown/failure states, real interaction evidence |
| Developer or release tools | Actual aggregate/CI invocation, source-versus-build prerequisites, valid and invalid fixtures, nonzero failures, platform and artifact evidence |

For each candidate finding, identify a reachable trigger, the failing behavior, and its impact. Check callers and counterexamples that might invalidate it. Prefer a focused reproduction or existing test evidence where practical; label unverified hypotheses as questions rather than confirmed defects. Missing tests are actionable when a specific changed behavior or failure path lacks evidence, not simply because no new test file exists.

Use [wombat-verify](../../wombat-verify/SKILL.md) to choose checks by affected scope. Build before tests that consume dist, distinguish static checks from behavior acceptance, and avoid rerunning unchanged passing checks. Report commands actually run and relevant platform/corpus limits. A green aggregate is evidence, not a substitute for semantic review.
