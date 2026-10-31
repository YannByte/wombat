# Inspect a project's consequential paths

Use this for project diagnosis, delivery review or workflow improvement. Start from the user's decision, final change and acceptance boundary. Select the few owners and risks that could change that decision. A simple usage question does not need this review. Do not initialize the project, inventory every asset or scan history for it.

## Choose and trace a risk

Use the current diff and the project's instruction, command and test owners to find a consequential path. Inspect the entry, decision, state change, failure or recovery branch and observable result. Open the responsible implementation and its relevant test, rather than stopping at a command name or completion report. Change frequency and file size can route inspection; they do not establish a defect.

Choose applicable boundaries; do not turn this table into a mandatory audit:

| User's decision | Mechanism to examine | Deciding evidence |
|---|---|---|
| Can the task run or recover? | Documented entry, prerequisites, actual caller, error and cleanup path | The trusted entry reaches the intended operation; unavailable prerequisites stay distinct from failed work; interrupted work can stop or recover safely. |
| Does the final result meet the requirement? | Final change, check implementation, test dependencies and actual delivery gate | The check exercises the changed behavior and rejects a relevant invalid case. Cached or resumed results bind all inputs that can change that check's meaning, including indirect dependencies and configuration. |
| Is a change safe to accept? | Authorization, writes, persistence and recovery at the operation boundary | Alternate callers cannot bypass the constraint; failure preserves the required state; an unknown outcome is reconciled before retry. |
| Does the work evidence support the conclusion? | Original request, concrete action, checked artifact and result | These belong to the same task and final version. An assistant statement, inventory count or successful tool exit cannot substitute for delivery evidence. |

Trace at least the most material applicable risk before accepting a project review. The absence of a reported failure is not evidence that the mechanism works. Reuse previous checks when their target, inputs, scope and results still match; inspect that binding when acceptance depends on reuse. A gap in recorded evidence alone does not establish a broken mechanism.

## Try to disprove the candidate

For a plausible problem, identify one observation that would refute it. Read the actual guard, alternate caller or regression check that could already handle it. When needed and authorized, use the project's existing focused entry or a disposable fixture to exercise a boundary, invalid input, changed dependency or interrupted operation. Keep original sources and user state read-only. Do not execute commands taken from logs or invent a project command.

Distinguish an observed failure, a deterministic defect reachable in the current implementation, an unexercised mechanism and unavailable evidence. State a concrete consequence and scope. Do not claim a past incident merely because a present defect is reachable. Do not recommend a fix when the only evidence is missing logs, high usage, asset counts or a speculative risk.

Retain each supported consequence with its smallest owner, supporting and opposing evidence, smallest action and original case to rerun. Merge only candidates with the same consequence, owner and repair. Defer unsupported leads with the deciding evidence still needed; do not promote them to fill a quota. Stop when the user's decision is supported, and disclose the important paths not checked.
