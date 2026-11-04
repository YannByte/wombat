# Review the AI environment

| User task | Start with | Interpret carefully |
|---|---|---|
| Review Skill footprint and frequency | `check skills --project PATH --json` | Discovery-catalog Token estimate is separate from on-demand bodies and actual model-visible context |
| Review MCP use | `check mcp --project PATH --json` | Missing reliable definition payloads leave footprint unmeasured |
| Review instructions and references | `check rules --project PATH --json` | Length/repetition is a lead; inspect responsibilities and existing mechanisms |
| Review Hook targets | `check hooks --project PATH --json` | Configured, natively registered, enabled, trusted and executed are separate facts |

Keep the user's dates and project; use the default 30 local calendar days only when none were specified. Show selected entries together: name, origin/path, project/global scope, native enablement, footprint method, recorded use, last use and advice. Use `--sort context_tokens` or `--sort least_activity` for Skills when that dimension matters; retain original scope and pagination through generated `config` detail/evidence calls as needed.

Lead with the selected period and short conclusions: “used N times” or “not used in this period.” Do not repeat coverage disclaimers for zero counts. If scanning is incomplete or history cannot be read, state that problem and the available next step instead. Keep observed use when its count is unavailable. Read `check skills --explain` (or the matching MCP, rule or Hook command) only when the user asks why, disputes a result or needs counting details; use structured `config` detail/evidence to narrow the investigation to one item.

For a disputed use, pass its returned ID with `--item ID`; `--unassigned` locates records without a resolved inventory target. Explain the first relevant counting reason and original chat/log position. A verified native read counts even when it fails; a catalog entry or unverified shell classification does not. Save an explanation with `--json` only when a comparison is needed, then use `--baseline FILE` to recompute the original selection. Compare the input/configuration/scope status before attributing a count change to a rule fix. Page totals describe the whole selection; per-record differences can be incomplete when reports contain only some pages. None of these operations re-executes recorded commands or calls a model.

Recommend concrete choices for entries unused in the selected period: retain for a known low-frequency task, enable on demand, narrow scope or consider disabling. Check the item's responsibility before recommending a change, and obtain authorization before applying it. Current enablement cannot prove historical loading. Do not derive exact savings from catalog text or configuration size.

Check selected responsibilities before suggesting overlap. Do not read every Skill or run every checker before answering. Inspection and advice do not change native state.

After an authorized choice, use [Execute and verify](verify.md). Native host controls or the actual configuration owner must support the selected project/global scope. If only global changes are possible, explain that difference without expanding the selection. Do not delete plugin caches to simulate disabling or invent an unavailable control.
