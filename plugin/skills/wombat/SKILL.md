---
name: wombat
description: Diagnose coding-agent project obstacles with local task evidence, propose specific improvements and recheck changes. Also query Codex work history, usage, costs and timing.
---

# Wombat

Help the user decide what to continue, investigate, accept or improve. Use the CLI for exact facts and bounded history. Use Codex to interpret evidence, inspect project files, make authorized changes and run checks.

Read the [operating rules](references/runtime.md). Reuse the known executable, scope and adequate read view. Start with a familiar query. Read [startup](references/startup.md) only for missing runtime information, pending data or a failed request. Do not run an initialization phase, setup, full-project inventory or history scan before a task. Prepare usage or history only when that task needs it.

Load only the requested task flow:

| User task | Read |
|---|---|
| Resume work or recover an earlier decision | [Resume](references/resume.md) |
| Investigate a failure, delay or repeated work | [Investigate](references/diagnose.md) |
| Decide whether a result meets requirements | [Verify delivery](references/delivery.md) |
| Review a day, week or selected period | [Work review](references/work-review.md) |
| Improve project instructions, checks or recovery | [Improve](references/improve.md) |

For a simple question, use its measurement reference directly: [usage](references/usage.md), [cost](references/pricing.md), [timing](references/timing.md), [statistics](references/statistics.md), [allowance](references/account.md) or [budgets](references/monitor.md). Use [conversation](references/conversation.md) for bounded original messages and [collection](references/collection.md) for setup or receipt questions. These reads do not require a diagnosis or saved finding.

Read project files directly when the question only concerns those files. Use [configuration](references/configuration.md) for measured inventory, loading or use evidence. Use [process and recheck](references/processing.md) when a change is authorized or the user follows up on an earlier change.

Give the answer first. For each supported recommendation, name the observed problem, evidence, exact owner, smallest action and original check to rerun. State unknown causes as investigation steps. Stop when the requested decision has enough evidence. Do not fill a finding quota, assign an overall score, create a new Skill by default or launch fixed specialist roles.
