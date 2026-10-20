---
name: wombat-config-review
description: Review Codex rules, Skills, MCP or Hook configuration and recheck authorized changes. Use for configuration cleanup, scope conflicts and loading questions.
---

# Configuration review and recheck

Read the [shared operating rules](../wombat/references/runtime.md). Keep the requested files, project and configuration kinds. For a few specified files and a static question, read those files directly; do not run setup, history scans or inventory discovery just to enter this workflow.

Use [configuration queries](../wombat/references/configuration.md) for inventory, measured findings, related scopes or native loading evidence that the question needs. Reuse supplied runtime information; use [startup](../wombat/references/startup.md) when it is missing or a call fails. Start with one scoped query, then read only selected details at its version. A static file review does not need a usage view.

Prioritize problems that affect the requested task: conflicting instructions, broken references, invalid fields or a concrete loading burden. Show the file or section, evidence, user impact and exact proposed change. Do not recommend renaming or stylistic consistency alone unless requested or supported by a compatibility problem. Keep format reminders separate from native loading failures. Missing use records cannot justify removal or disabling.

For a proposed split, name the sections to move and when to read them. Preserve triggers, common constraints, relative references and shared-project behavior. Review related scopes before editing. Check representative paths after a split; a shorter entry or text estimate does not prove lower injected Tokens or better output.

If editing is authorized, follow [processing](../wombat/references/processing.md). Bind the reviewed target version, edit only that target and recheck the same issue. For an existing Wombat suggestion, retain its ID and rules. For a direct file finding, use its original section and a suitable parse, reference or behavior check; do not invent a suggestion ID. Keep user decisions separate from checks.

For a follow-up such as “check yesterday’s change,” retain the original issue and target. If they cannot be recovered reliably, ask for the missing selection. Do not choose a replacement from the current ranking.

Report the finding, any actual edit, the recheck result and remaining uncertainty. Static success does not prove historical loading, effective adoption, better task quality or savings. Use later version-bound observations and comparable usage windows only when available.
