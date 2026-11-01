# Decision Note: extension context footprint and usage review

[中文](2026-11-01-extension-review.md) | English

Status: implemented

## Problem

Users want to see which Skills, MCP servers and other extensions occupy more context but receive little recent use, then decide whether to keep, enable on demand, narrow scope or disable them. Installation lists and separate usage rankings do not answer both questions.

Follow Claude Code's [`/skill-doctor`](https://code.claude.com/docs/en/skills#find-unused-skills): show each Skill's context burden alongside use frequency, prioritizing heavy, rarely used entries for review. Wombat targets Codex using its own CLI and evidence semantics rather than another manager. Earlier content/body reference Tokens, use bases and related tasks lacked native instance state and discovery-metadata measurement. Body estimates cannot substitute for context footprint.

## Decision

### Product decision

The primary view combines context footprint and use frequency. Functional overlap, format and reference checks explain selected entries when needed. Retain one main Skill, five references and five CLI entries. Run on demand without a Web host, separate Skill, overall score or resident collector. Existing cost capabilities remain unchanged and are not a primary review metric or selling point.

This decision owns the review tradeoffs, supplementing [G05 of the optimization proposal](../../proposed/product/2026-10-03-optimization-lifecycle.en.md). G05 still governs proven long-term inactivity. Responsibilities from [read-only checks and one plugin](../architecture/2026-10-31-on-demand-checks.en.md) remain: Wombat supplies facts, evidence and rechecks; Codex interprets and performs authorized changes. The [CLI guide](../../../guides/cli.en.md) and [public contracts](../../../development/contracts.en.md) own current entries and measurement limits.

### One table for the decision

Show object type, origin and enablement scope, context Tokens and measurement status, use count in the selected window, last use and suggested handling. The Skill first presents a few entries worth reviewing; CLI pagination exposes the complete result and evidence on demand. Default to the last 30 local calendar days including today, unless the user specifies a range. State timezone, boundaries and coverage.

Separate enabled, disabled, visibility-unconfirmed and disk-only objects first. Within comparable object types, count units and measurement methods, show lower use first, then larger context footprint. Allow ordering by either dimension. Keep unknown values separate rather than sorting them as zero. Do not produce a waste score or divide Tokens by calls to infer value.

| Observations | Priority advice |
|---|---|
| High context footprint, low recent use | Review first; use current work needs to choose on-demand enablement, narrower scope or disabling |
| High context footprint, frequent use | Prefer keeping; suggest trimming only for a concrete content problem |
| Low context footprint, low recent use | Lower review priority; do not require disabling to fill a quota |
| Unknown footprint, use or coverage | Show known facts and gaps without judging the object useless |

Rare but critical release or recovery capabilities can remain enabled. Zero records mean no observed use within the selected window, not that every project has no need. Retain task needs and impact scope in advice. A user can explicitly choose to disable an entry without waiting for complete inactivity proof.

### Context-footprint semantics

The primary Skill metric is resident discovery metadata in the current listing: name, description and locator information actually presented by the host. Show on-demand body and references separately rather than treating the whole SKILL.md as resident every turn. Include an entry in context totals only with evidence for host version, visibility and rendering scope. Otherwise, a method-bound entry estimate can remain visible, marked visibility-unconfirmed and excluded from confirmed-visible totals.

Prefer host observations with explicit scope and method. Otherwise reuse the Rust reference tokenizer on versioned listing-renderer text, labeling encoding, method, input scope and version. This is not exact occupancy for the current model, and current listing estimates cannot describe historical sessions. Existing full-file and body estimates cannot simply be renamed to provide this metric.

Measure MCP tools, resources or prompt definitions only from reliable payloads, separating exposed and deferred parts. Configuration size, tool count and preset multipliers cannot substitute. Hook script size is not injected context; only reliable injected payloads count. Complete Skill review first. MCP and Hooks can initially show use/configuration evidence with unmeasured context when payloads are unavailable. [G02 of the optimization proposal](../../proposed/product/2026-10-03-optimization-lifecycle.en.md) still governs complete MCP definition observation.

Sum known values only within the same view, method and non-overlapping scope, showing unmeasured item counts. Keep enablement, host visibility, historical loading and actual use distinct. Desktop installation totals cannot establish context burden. Allocate neither Tokens nor money to extensions and promise no actual savings.

### Use-frequency semantics

Retain the [shared use definition](../architecture/2026-10-04-event-metrics.en.md) and [MCP identity rules](../architecture/2026-10-04-mcp-runtime-evidence.en.md). Preserve evidence types for targeted Skill reads, reliable native loads, MCP tool calls and resource reads. Reading is not proof of adoption or task success. Show operation counts, related task counts and last times separately; do not replace them with session-file counts.

Deduplicate start/result and replay through operation identity while retaining independent retries and dispatched failures. Listings, discovery, prose mentions and Wombat scans do not count. Partial sources, missing times, unknown versions and conflicting identities retain gaps rather than zero. Pin the original read view before reading object content so current evidence reads cannot feed back into this decision. Do not exclude entire sessions containing ordinary work or add another log parser or use-history database.

### CLI, Skill and change verification

Rust reuses configuration objects, use bases and the reference tokenizer, adding discovery-metadata quantities, measurement status and ordering/totals only for actual consumers. Generate TS and Schema from Rust DTOs. Node reuses native connections and `skills/list`, distinguishing selected project instances by name, path, origin and enablement. File existence cannot replace host enablement.

Use `check skills` for the task view, with generated `config` detail, evidence and related-scope operations. Keep static checks in `check` and `check verify`, exposing the same capabilities through `system api` and `system call`. Implementation public contracts own concrete fields and arguments. Within the five references, `environment` organizes extension review, `diagnose` selects project evidence, `cleanup` handles artifact choices, `verify` organizes authorized adjustment, recovery and recheck, and `query` supplies history only when needed.

When requested, Codex uses verified host configuration capabilities or trusted project tools to change only selected entries and retain minimal recovery material. Prefer disabling or narrower scope over deleting files or plugin caches. Disclose actual global impact when project controls are unsupported. Reuse existing authorization; confirm again only when impact changes. Recovery restores selected values without overwriting later unrelated edits. Add no automatic handling history.

After changes, reread native enablement and visibility for the same project and object identities, using the same measurement method. Compare listing-metadata quantities and check required capabilities. Disabling does not reset historical frequency or alter fixed views. Report estimate changes as estimates, not actual whole-session savings. Expired conditions require fresh evidence rather than reconstruction of an earlier success. Inventory must not start MCP servers or run unrelated model sessions.

### Research choices

| Source | Adopted mechanism and limit |
|---|---|
| [Claude Code skill-doctor](https://code.claude.com/docs/en/skills#find-unused-skills) | Context burden and use frequency per entry, prioritizing heavy, rarely used items. Do not transfer private measurement methods or host state directly to Codex. |
| [codex-skill-admin](https://github.com/zjp1997720/zhijian-skills/blob/d3bca91227aaf043345d761a9b4785a954184c3a/skills/codex-skill-admin/SKILL.md) | Native inventory, preview, disabling, recovery and visibility readback while retaining Wombat operation counts. |
| [Jiepi Skill](https://github.com/Pluviobyte/jiepi-skill/blob/0dc5c2868949234d4ae3ddfb510d35c95d8abe97/SKILL.md) | Keep/review/candidate choices and plugin origins. No confirmed record does not mean uselessness. |
| [skill-graveyard](https://github.com/sfrangulov/skill-graveyard/blob/7e499c360cf7498dc6ff7c5e34b6be8c23e533cc/README.md) and [Skills Janitor](https://github.com/khendzel/skills-janitor/blob/4b42f391fd537b791d1c09c7825be0bc641fd41c/README.md) | Cross installation with use, inspect by project and let users choose. Do not copy safe-removal assumptions, another score or a terminal UI. |
| [dr-mcp](https://github.com/Inferensys/dr-mcp/blob/ed6e00d1f8d821e9ee5fa0d2bbb1f877b22f10ca/README.md) | Configuration origins and concrete adjustment plans. Persistent installation is not invocation frequency, and preset tool counts are not measured context. |

External-project research covered documentation and selected source, not installed acceptance. Codex listings do not expose the aliases, description budget or dynamic selection actually used by the current model. The first version therefore uses a versioned unaliased reference row, always marks visibility unconfirmed and produces no confirmed-visible Token total. G02 still owns complete MCP definition observation.

## Alternatives considered

- Installation or frequency rankings alone omit context burden. Show both in one table.
- Start with a comprehensive functional-value review: this expands the task. Use the two primary metrics to select objects, then explain them as needed.
- Adopt an external manager or add a Skill: this duplicates parsing, measurement and mutation ownership. Reuse current CLI and references.
- Auto-disable from recent zero counts or Tokens divided by calls: this harms rare critical capabilities and incomplete observations. Use ordering and user choice rather than a deletion or disable verdict.

## Consequences and verification

- [Core fixtures](../../../../core/src/config/tests.rs) cover body independence, conflicts and unknowns, host revisions for native state, ordering and pagination. Existing date-window, operation-deduplication and ledger tests continue to constrain use semantics.
- [CLI end-to-end tests](../../../../tests/e2e/extension-review.test.ts) cover native-only plugin entries joined with historical reads, retained views, body growth, disabling/restoring, protocol failures, `api`/`call` reachability and unchanged usage under a fixed snapshot.
- The [native qualification entry](../../../../scripts/verify-skill-native.ts) installs the current plugin in a fresh isolated profile and verifies discovery, actual disabling/restoring, catalog measurement and old views. It calls no model and proves neither future recommendation adoption nor actual whole-session savings.
- Unobservable host content remains unknown. Rare critical capabilities still require task judgment. Codex performs authorized edits and recovery without another change manager, automatic handling history or use-history database.
