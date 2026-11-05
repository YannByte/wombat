# Review the AI environment

| User task | Start with | Interpret carefully |
|---|---|---|
| Check whether the Agent environment is ready | `check --project PATH --json`; add `system doctor --project PATH --json` only for host/setup questions | Configuration, discovery, enablement, trust, connection/loading and execution each need their own evidence |
| Review Skill footprint and frequency | `check skills --project PATH --json` | Discovery-catalog Token estimate is separate from on-demand bodies and actual model-visible context |
| Review MCP use | `check mcp --project PATH --json` | Missing reliable definition payloads leave footprint unmeasured |
| Review instructions and references | `check rules --project PATH --json` | Length/repetition is a lead; inspect responsibilities and existing mechanisms |
| Review Hook targets | `check hooks --project PATH --json` | Configured, natively registered, enabled, trusted and executed are separate facts |

Keep the user's dates and project; use the default 30 local calendar days only when none were specified. Show selected entries together: name, origin/path, project/global scope, native enablement, footprint method, recorded use, last use and advice. Use `--sort context_tokens` or `--sort least_activity` for Skills when that dimension matters; retain original scope and pagination through generated `config` detail/evidence calls as needed.

Lead with the selected period and a short conclusion: “used N times,” “not used in this period” when absence is observable, or “no use observed in the available records” when coverage is partial. State the concrete gap once; do not repeat generic disclaimers. Partial zero observations alone never justify disabling or deleting an entry. Keep observed use when its count is unavailable. Read `check skills --explain` (or the matching MCP, rule or Hook command) only when the user asks why, disputes a result or needs counting details; use structured `config` detail/evidence to narrow the investigation to one item.

For a disputed use, pass its returned ID with `--item ID`; `--unassigned` locates records without a resolved inventory target. Explain the first relevant counting reason and original chat/log position. A verified native read counts even when it fails; a catalog entry or unverified shell classification does not. Save an explanation with `--json` only when a comparison is needed, then use `--baseline FILE` to recompute the original selection. Compare the input/configuration/scope status before attributing a count change to a rule fix. Page totals describe the whole selection; per-record differences can be incomplete when reports contain only some pages. None of these operations re-executes recorded commands or calls a model.

Recommend concrete choices for entries unused in a sufficiently observed period: retain for a known low-frequency task, enable on demand, narrow scope or consider disabling. Check the item's responsibility before recommending a change, and obtain authorization before applying it. Current enablement cannot prove historical loading. Do not derive exact savings from catalog text or configuration size.

For each consequential readiness gap, explain its effect and the smallest next check. A discovered/enabled Skill does not prove a new conversation loaded or used it; inspect a relevant real task to confirm that separately. An MCP declaration does not prove a live connection: use an existing host connection observation or propose a read-only probe with known effects, never launch an arbitrary configured server to fill the gap. A registered but untrusted Hook needs a user’s trust decision before execution; static target checks can inspect its path without running it. Mark inapplicable stages as such rather than filling unknowns with success.

Treat naming-only `advisory` findings as specification reminders, including for enabled provider plugins. Applicability to a project is separate from ownership of the source file. Before recommending edits, identify whether the file belongs to the project, a shared/global installation or a managed provider cache; prefer the provider’s supported update/configuration path and disclose effects on other projects. Do not present a standard mismatch as a demonstrated runtime fault.

Check selected responsibilities before suggesting overlap. Do not read every Skill or run every checker before answering. Inspection and advice do not change native state.

After an authorized choice, use [Execute and verify](verify.md). Native host controls or the actual configuration owner must support the selected project/global scope. If only global changes are possible, explain that difference without expanding the selection. Do not delete plugin caches to simulate disabling or invent an unavailable control.
