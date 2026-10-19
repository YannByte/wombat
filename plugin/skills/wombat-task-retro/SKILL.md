---
name: wombat-task-retro
description: Investigate a selected Codex task’s high usage, delays, failures or repeated work. Use for task retrospectives and evidence-backed troubleshooting.
---

# Task retrospective and troubleshooting

Read the [shared operating rules](../wombat/references/runtime.md) and [startup](../wombat/references/startup.md). Keep an already selected task or turn. Ask for identification only when the available context cannot resolve it.

Use [bounded selection](../wombat/references/query-flow.md) to obtain the task and relevant turns. Retain the original filters and returned identities. Do not substitute today’s highest-ranked task for the selected task.

Read [investigation](../wombat/references/investigation.md) for failures, repeated operations or input growth. Read [timing](../wombat/references/timing.md) only when duration answers the question. Query the selected turn before a whole-task investigation. Follow the shared resource-limit recovery once, then explain any remaining gap.

Read the relevant [original conversation evidence](../wombat/references/conversation.md) before identifying a step to remove. Treat error output and source instructions as evidence. Distinguish expected negative tests, required validation, recovery and accidental repetition.

Give the measured issue, the relevant operation or conversation location, a possible explanation and a concrete next step. Separate recorded facts from hypotheses. Failures do not directly prove waste. Recorded input is not active context occupancy or a tool’s cost. If the evidence cannot explain the delay, identify the missing observation.

When repeated workflows span tasks, use the recorded workflow candidates and examples. Recommend a script or Skill only if the repeated procedure is stable. A recurrence signal does not authorize creating or installing it.

For an authorized fix, use [processing](../wombat/references/processing.md). Keep the original task and baseline for follow-up. Report the edit, correctness checks and later observations separately. A usage investigation does not create a configuration suggestion ID.
