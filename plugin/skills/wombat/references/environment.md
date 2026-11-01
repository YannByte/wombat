# Review the AI environment

| User task | Start with | Interpret carefully |
|---|---|---|
| Review Skill footprint and frequency | `check skills --project PATH --json` | Discovery-catalog Token estimate is separate from on-demand bodies and actual model-visible context |
| Review MCP use | `check mcp --project PATH --json` | Missing reliable definition payloads leave footprint unmeasured |
| Review instructions and references | `check rules --project PATH --json` | Length/repetition is a lead; inspect responsibilities and existing mechanisms |
| Review Hook targets | `check hooks --project PATH --json` | Configured, natively registered, enabled, trusted and executed are separate facts |

Keep the user's dates and project; use the default 30 local calendar days only when none were specified. Show selected entries together: name, origin/path, project/global scope, native enablement, footprint method, recorded use, last use and advice. Use `--sort context_tokens` or `--sort least_activity` for Skills when that dimension matters; retain view and pagination through generated `config` detail/evidence calls as needed.

A partial count is a lower bound, including zero. No record does not establish inactivity or uselessness. Large rarely used entries deserve review, while low-frequency release/recovery abilities can be valuable. Current enablement cannot prove historical loading. Do not derive exact savings from catalog text or configuration size.

Check selected responsibilities before suggesting overlap. Explain the concrete task need and smallest useful choice: retain, enable on demand, narrow scope or disable. Do not read every Skill or run every checker before answering. Inspection and advice do not change native state.

After an authorized choice, use [Execute and verify](verify.md). Native host controls or the actual configuration owner must support the selected project/global scope. If only global changes are possible, explain that difference without expanding the selection. Do not delete plugin caches to simulate disabling or invent an unavailable control.
