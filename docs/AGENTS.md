# Documentation Instructions

Follow [root rules](../AGENTS.md). Check claims against source, generated contracts, and execution; proposals do not prove delivery. Use [wombat-docs](../.agents/skills/wombat-docs/SKILL.md) for restructuring.

## Ownership

Apply the [one fact, one home rule](../AGENTS.md#standing-constraints) with the owners below. Add pages only for distinct reader tasks or responsibilities. Parent pages orient readers without repeating descendant details.

| Subject | Owner |
|---|---|
| Standing Agent rules | Root or scoped AGENTS.md, in English |
| Procedures | Dev: .agents/skills/; user: plugin/skills/; use module contracts |
| Durable rationale and tradeoffs | [Decisions](decisions/README.md); no record for mechanical/local fixes |
| Unfinished requirements and acceptance | Owning proposed decision; no parallel specification or status page |
| Module relationships and data flow | [Architecture](development/architecture.en.md) |
| Public entries, behavior, and limitations | Owning module README or technical reference; fields derive from source/generated contracts |
| Verification evidence | Test output, CI/PR results, and scoped benchmarks/ artifacts; no rolling progress ledger |
| Contributor workflow | [Development](development/workflow.en.md) |
| User operations | guides/ and module READMEs |

## Editing

- Public prose follows the [bilingual workflow](i18n/workflow.en.md): update both languages, review technical prose under ASD-STE100, and confirm only reviewed pairs. AGENTS.md files use English and are exempt from pairing.
- User docs exclude release internals; docs:structure checks this.
- Tutorials follow prerequisites, steps, observable results, and failure recovery. References describe current behavior by topic. Split substantial mixed content.
- Use one Markdown title and one physical line per prose paragraph; preserve code, tables, and quotes. docs:structure checks paired prose and budgeted instructions; leading HTML branding is allowed.
- Update only affected owners and links. Update proposal status only when acceptance changes. Do not recreate roadmap, support-matrix, or progress ledgers elsewhere.
- Verify commands, defaults, errors, and platform claims against current source/execution. State unverified limits; historical tests and screenshots do not validate today's checkout.
- Generated schemas/types/validators derive from Rust DTOs. Do not hand-maintain field catalogs. Comments document non-obvious behavior, failure, timing, and ownership, not reasoning transcripts or tests.
- Keep current promises in maintained prose, rationale in decisions, and historical evidence in Git/CI or scoped artifacts. Remove duplicate or obsolete instructions without discarding outstanding requirements.
- Public references must not expose private material, conversations, account data, or unreviewed output.
- [Budgets](../scripts/doc-budgets.json) count non-whitespace Unicode characters. Relocate or condense before raising a ceiling; justify changes, including language conversion. Never remove necessary failure/limitation rules merely to fit.
- Moves/deletions update incoming links, bilingual records, budgets, and release manifests together. Run corepack pnpm docs:check and git diff --check; repository-rule changes also require corepack pnpm repo:check.
