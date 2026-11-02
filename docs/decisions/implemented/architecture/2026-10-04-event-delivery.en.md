# Decision Note: Event query interfaces and shared pages

[中文](2026-10-04-event-delivery.md) | English

Status: implemented

The service and retained-view lifecycle described below is superseded by the [foreground-query decision](2026-11-02-foreground-queries.en.md). Its independent source, accounting and bounded-evidence rationale remains historical context.

## Problem

This record owns all design responsibilities from section 7, section 8, section 9, section 15, section 16 of the [upgrade overview](2026-10-04-codex-task-timing.en.md), as their single detailed design owner. The overview retains research evidence, cross-module constraints, and delivery limits; its entries define neighboring decision responsibilities.

## Decision

The [statistical analysis architecture revision](2026-10-05-analysis-first-events.en.md) updates observation semantics, partial results, fallback calculations, and suggestion requirements. It takes precedence over older whole-result unavailability constraints below. Privacy, use-count semantics, identity protection, and final acceptance remain applicable.

### 7. Minimum CLI and JSON contract

The CLI contract has three actions; current usage is documented in the [CLI guide](../../../guides/cli.en.md):

```sh
wombat timing [summary] --thread THREAD_ID --turn TURN_ID [--snapshot SNAPSHOT_ID] [--share]
wombat timing evidence --thread THREAD_ID --turn TURN_ID --snapshot SNAPSHOT_ID [--limit 50] [--cursor OPAQUE_TOKEN]
wombat timing capabilities [--share]
```

Summary supports repeated `--root`, `--source`, `--fresh` / `--cached`, `--snapshot`, `--lang`, and cancellation. Accept full Wombat identities without silently falling back to upstream IDs; reject mismatched or missing targets. `--turn` is required; task aggregates come later. Reject date/Token/cost filtering or pagination that fragments the summary's whole-turn window. Evidence uses the same fixed target/version/scope with default50/max200 rows and an opaque cursor, local profile only and no refresh mode. Capabilities accepts no target or source paths and performs no scan. Emit one final JSON object by default with status on stderr; explicit `--text` and `--json` are mutually exclusive. Sharing requests Rust's separate projection. Timing bypasses automatic pricing, configuration scans, Hook capture, and account observation. Errors retain the separate v1 safe envelope; cancellation exits130 and does not stop shared synchronization.

Rust defines the narrow summary/evidence/capabilities requests, local/share response branches, and independently versioned errors. Generated Schema, TypeScript, and validators are authoritative; see the [core reference](../../../reference/core.en.md). Keep response/method/storage/adapter versions independent. Numeric fields retain recorded, derived, proxy, or unavailable bases; zero requires evidence. Sharing is a separate allowlisted projection with fresh aliases, never a frontend deletion of selected local fields. The [analysis revision](2026-10-05-analysis-first-events.en.md) preserves useful partial metrics and specific explanations.

Use exit 0 for success, including complete reads without causal attribution; 2 for partial reads/key evidence gaps/provisional running results; 1 for argument/operation errors; 130 for cancellation. Missing optional capabilities still return results marked unavailable. Current-format snapshots with missing source fields return unavailable; old formats are rejected without reading current logs into fixed history. Errors use `{outputVersion:1,error:{code,message}}` with safe template messages. Reuse INVALID_ARGUMENT, VIEW_EXPIRED, SNAPSHOT_CORRUPT, SOURCE_UNREADABLE, RESOURCE_LIMIT, CANCELLED. Add diagnostic quality reasons TIMING_DETAIL_UNAVAILABLE for required facts absent from source logs; missing required shards/envelopes use SNAPSHOT_CORRUPT and TIMING_BOUNDARY_CONFLICT for contradictory explicit boundaries. Return available native scalars with affected derived values null, preserving useful results when an individual metric is missing.

### 8. Historical compare and on-demand watch

Do not expose `compare` in the first phase. Later stratify history by source/explicit project, model, reasoning effort, and diagnostic method. Separate mixed-model turns; do not invent defaults for missing model/effort/window. Also match input median/P90, window ratio, compaction, operation volume, labels, task state, and log version. Projects use explicit historical directory evidence. Cross-worktree grouping requires user declarations or reliable repository identity, not path substrings.

Return full selection rules, sample counts, exclusions, periods, coverage distributions, per-turn statistics, and quantiles. Prefer equally weighted turns; report per-cycle weighting separately so large turns do not dominate. Define strata before examining results, retaining long waits and compactions. Excluded variants are named sensitivity analyses only. Without comparable samples, return unavailable, not stable speed multipliers or claims that one model is better.

These are observational comparisons, still confounded by task difficulty, scope changes, host load, periods, and quality. Controlled benchmarks require separate experiments with matched tasks, initial repository/context, tools/network, and settings, interleaved/random model order, repeated runs, and quality assessment. Historical filters do not become controlled experiments or establish quality/platform throughput from speed.

Later `timing --watch --json` observes only a selected running turn, reusing on-demand service/cancellation. Emit ordered NDJSON revision/result/error when data/state changes; Ctrl+C exits 130. Emit a final result and exit after an explicit terminal event. Running intervals are right-censored at the observation cutoff as lower bounds with a separate `openIntervalCount`. Activity not yet logged must not be filled in as model work. Watch does not launch Codex, install Hooks, run permanently in the background, or silently widen scope. Fixed snapshots/cached mode are incompatible with watch; no promise of retaining live versions permanently after exit.

### 9. Privacy and shareable artifacts

Follow [privacy rules](../../../reference/privacy.en.md): read-only sources, local processing, no basic-scan uploads/model calls/body replay. Store only necessary identities, types, times, counts, enum states, and local evidence. Exclude reasoning bodies/summaries, messages, full commands/arguments, stdout/stderr, diffs, raw JSON, and encrypted content. Existing paths, titles, tool/MCP names, and stable identities are not automatically shareable.

The first phase provides a separate safe projection through `timing ... --share --json` and a previewable Web summary, clearly distinct from ordinary JSON. Request `privacyProfile: local|share-v1` selects separately generated Rust response DTOs. Sharing removes `readView` and local `scope/evidence`, replacing them with package aliases and safe basis codes. Confirmed values may be flattened while retaining unknown states and methods; frontend removal of a few fields is insufficient. Remove source roots, project/file/evidence paths, native/Wombat identities, titles, commands/arguments, message bodies, tool output, repository URLs/commits, account metadata, trace/call/response identities, custom tool/MCP/provider names, and annotation prose. Allow only known public model identifiers; map other values to unknown. Hashing real paths is not anonymization.

Use fresh export-specific aliases T1/R1/E1 consistent within each package. Store milliseconds relative to turn zero; omit absolute times/timezones by default. Free-text issues/errors/findings use stable codes and safe templates. A privacy manifest records omitted fields. Behavioral statistics may still identify people, so call this a summary with direct identifiers removed, not complete anonymity.

Later evidence bundles contain only summary, optional safe relative intervals/samples, Schema/method versions, and exported-file hashes. Hashes establish package-file integrity, not log authenticity; exclude linkable raw-file hashes. Do not export alias mappings; returning to source evidence is a local action. Preview fields before explicit user-directed saving; write only to the selected destination atomically, leaving no partial bundle on cancellation. No automatic uploads, external links, or log attachments. Validate traversal, symlinks, internal paths, and size limits. Snapshots are not encrypted vaults; define retention/deletion with the new storage format.

### 15. Restricted interfaces and host integration

P0 timing Request is a generated action-tagged union: `summary` returns whole-turn summaries, `evidence` returns paginated safe facts, and `capabilities` reports field-level support without scanning. Summary requires threadId/turnId. Evidence also requires the prior result's fixed snapshotId, with default50/max200 pagination that never changes summary denominators. Capabilities accepts no task or source paths; it declares parser support only, while summary coverage reports actual availability in the selected turn. `privacyProfile` is local/share-v1; sharing exposes no local evidence action. Rust, client, and host reject unknown fields.

Live requests add narrow envelope `{timing: Request}` to the existing on-demand service. Fixed disk snapshots use separate core operation `{op:"timing",args:Request}`. One Node `UsageClient.timing(request, options)` selects these restricted transports without exposing op strings. Add the envelope to shared request dispatch in [live/transport.rs](https://github.com/YannByte/wombat/blob/475d0bbf0cc86281875b227c00d7c1e413c9ba81/core/src/live/transport.rs), retaining common handling for Unix sockets and Windows named pipes, with separate platform connection verification.

Extract source scope, selector, mode, and read-version selection from `select_view` in [live/selection.rs](https://github.com/YannByte/wombat/blob/475d0bbf0cc86281875b227c00d7c1e413c9ba81/core/src/live/selection.rs) into internal `ReadViewSelector`. Usage and timing validate their own business parameters before shared selection. Do not construct fake usage/refresh requests to obtain synchronization or add actions to legacy `live.query` unions. Selection returns `Arc<Snapshot>` and freshness, preserving auto's2-second/fresh's10-second waits, offline cached behavior, and existing expiry. `UsageClient.timing` bypasses `withAutomaticPrices`, Node Hook capture, account reads, and Codex handoff. Initial previews return partial without silently advancing a timing query to a newer revision; an explicit refresh switches usage and timing together. The existing query path may resolve the same snapshot retained by a configuration view; missing or expired identities fail rather than falling back to latest data.

The generator adds diagnostic request, local response, and sharing response Schema/TS/validators. All transports validate outputVersion, action, profile, and read identity. Add a separate optional timing transport to `createUsageClient`; hosts without this optional capability expose unavailable; incompatible protocol versions are rejected, never replaced with fabricated UI data. Organize transport assembly parameters during implementation rather than copying increasingly long positional calls. Public `UsageClient` remains narrow typed methods, not generic dispatch.

Web adds `/api/timing`, reusing Bearer, Origin/Host, JSON POST, and NDJSON envelopes. Reject browser-supplied roots/projectRoots and arbitrary snapshot file paths. Inject startup roots, accept only published read identities, and verify task/source membership in that version. Preserve the host's128-identity bound and actual core expiry. Diagnostics cannot authorize reading repositories/configuration under historical cwd.

Cancellation is layered: abort closes the caller's transport and isolates late results without stopping shared synchronization or other clients. Target computation loops gain request-level cancellation checks and never cache cancelled results. If disconnect cannot promptly propagate cancellation, describe only caller-side waiting cancellation; interval/time budgets bound remaining core work. Do not claim shared scans were cancelled. Fixed-snapshot subprocesses reuse timeout/output/cleanup behavior. New cross-host collaborative cancellation is not an implicit P0 promise.

### 16. Web presentation and resource boundaries

Retain existing task→turn detail, URL scope, and exact turn identity, adding “Execution” within the same detail rather than a separate “Timing data” page. Prioritize turn status, total duration, and Tokens above the fold; first Token, compaction statistics, input distributions, and historical windows belong in expandable details. Show only the execution timeline by default; “Time distribution” expands category unions, exclusive coverage, and overlap explanations. Summaries and evidence consume Rust results; the browser neither pairs events nor recalculates totals.

Use fact/proxy/unavailable states and corresponding evidence copy. No ranked “main causes,” generic health score, or unsupported interval presented as slow-model time. Name input/window ratios separately from exact context occupancy. Do not show final duration for a turn or interval that is still running; show committed records and the “In progress” state. A UI timer cannot manufacture unlogged model activity. Derive values when evidence supports them. Distinguish source fields that are not recorded, unsupported, or not applicable; omit inapplicable fields. Keep values missing when evidence is absent and never substitute zero. Preview safe sharing output; copying must not mix in page titles, paths, or addresses.

New `useTiming` coordinates identity, version, requests/cancellation, and caching only. Bind requests to the same snapshotId returned by the turn list; a configuration readView is not a usage snapshot identity. Returning to a turn or changing language preserves filters. Task/version changes invalidate prior request identities; late results cannot replace the new target. Load timing on demand without blocking initial usage display.

Define P0 budgets in Rust: at most100,000 diagnostic facts per target turn and200 evidence rows per page. Exceeding fact budgets must not produce complete P90/duration from a truncated subset; return native scalars with derived-metric gaps. Parse valid large lines fully, without silent input truncation. The100,000-fact limit bounds target-turn computation, not scan/persistence retention or full-source fact memory; D1/D2 separately measure full-source residency and incremental appends. Target summaries≤256 KiB; retain Web's16 MiB response limit as final protection. Build success is not resource acceptance.

Each read version owns an independent diagnostic result cache, initially16 entries/2 MiB total/256 KiB each. Oversized results are uncached. Keys include task, turn, method, detail page, and profile; the owner fixes the read version. Do not force new DTOs into the current usage-Response-only cache. Sharing caches deterministic identifier-free values only; aliases are freshly generated per output, never reused across exports. Limits, cancellation, and failure must not cache “complete success.”

#### 16.1 User questions and page hierarchy

The initial page serves a single-turn review: enter a turn from the existing task list, check consumption, inspect the process, and verify individual operations. Users should not need to understand events, projections, read watermarks, or algorithm versions. Reuse existing navigation, filters, and usage components rather than adding another task entry point. The current page uses production components and a repository-hosted synthetic-data preview; this development preview is not screenshot acceptance or acceptance in a real browser or user journey.

| User question | Primary presentation | On-demand details |
|---|---|---|
| Which turn am I viewing, and is the data complete? | Turn identity, source, running status, data update time, and gaps affecting conclusions | Read scope, source time basis, and version details |
| How much time and how many tokens did this turn use? | Reuse the existing Token summary and show evidenced turn duration; explain missing values | First Token, input distribution, historical window, calculation definitions, and sample counts |
| What happened during this time? | Simplified timeline showing recorded commands, MCP, compaction, and uncovered intervals | Category intervals, overlaps, corresponding operations, and evidence |
| Which Skills/MCPs were used, and how often? | Object names, use counts, and completeness | Time, operation status, and source basis for each use |
| Where did this number come from, and can I share it? | “View evidence” beside metrics and “Share summary” | Calculation explanation, paginated evidence, and safe sharing preview |

Order the page as “turn and data status → core summary → timeline → Skill/MCP usage → additional metrics and evidence.” Do not put every statistic above the fold; keep P90, interval coverage algorithms, and versions in details. Skill/MCP usage can be located directly within the same turn detail without first understanding the timing chart. If a current-configuration entry is shown, label it as current configuration rather than merging it with historical use into one state.

Integrate the existing turn summary, Token/cost composition, and steps: keep one summary, expose existing composition through “Usage breakdown,” and have the timeline and operation list reference the same operations without adding duplicate lists. Omit the usually constant related-turn count of 1 from a single-turn page. Retain that metric for task or cross-task summaries without introducing a new summary entry point.

#### 16.2 Interaction and wording

Use turn-relative time on the timeline, with original timestamps and their basis in operation details. Derive times when event or source evidence supports them. When a missing time affects understanding or a decision, give its specific reason, such as “Duration not recorded in the log”; never guess a position or invent a duration. Omit missing details that do not affect understanding. Preserve separate tracks for overlapping activities and explain that “Some operations run concurrently; their durations cannot simply be added.” When native total duration differs from the locatable log observation window, label them separately; do not stretch the timeline or use an invalid coverage denominator. Use neutral texture and text for uncovered intervals, not labels such as waiting or model thinking. Explain facts, proxies, and missing values in user-facing terms such as “Recorded in the log,” “Estimated from records,” and a specific reason for absence; proxies must specify their method.

When reliable turn anchors are absent or most operations cannot be located in time, lead with the operation list and give specific reasons for missing times when that information affects understanding or decisions. Omit missing details that do not matter. Show isolated valid intervals only in on-demand details rather than drawing an apparently complete turn timeline. For many operations, group tracks by category and expand on demand instead of showing an unreadable screen of thin bars. Visual aggregation must not change counts or hide gaps that affect interpretation.

Selecting a summary metric or timeline segment locates related evidence from the same version. Evidence filters affect the list only; the summary remains labeled “Whole turn.” Sort Skill/MCP objects by descending use count, then name for stable ties, and show each operation in details. Failure describes the result without reducing use counts or automatically recommending disabling anything. When no use is observed, show the inspected scope without implying lack of value. Viewing evidence does not automatically reveal raw messages or tool output and introduces no arbitrary file-reading entry point.

For running turns, show “In progress” and the last update time without final duration or a countdown. Initially, explicit “Refresh data” updates the whole metric group. Retain existing content with an updating indicator during refresh, then switch together so numbers and evidence do not jump during reading. When the old version expires, ask the user to refresh rather than silently selecting a new version. Returning to the task list preserves filters and position; closing evidence details restores focus to the triggering control.

On narrow screens, stack the same information order and provide an equivalent operation list for the timeline. Keyboard users can expand, filter, refresh, and share. Do not rely on color alone for states or hover alone for key values and evidence; allow full long object names to be inspected. Sharing previews the exact safe summary and its data cutoff before copying. If copying fails, retain the preview and explain how to retry.

#### 16.3 States and user acceptance

| State | What users see | Available action |
|---|---|---|
| First load or initial scan | Records are being read; existing task information stays visible, unfinished metrics never appear as zero | Browse available content or leave the page |
| Partial data or computation limit reached | Available metrics beside gap explanations; affected totals do not claim precision | Inspect gaps; offer refresh only for recoverable reading problems |
| Required fields absent from source | “Not recorded” and the affected scope, distinguished from actual zero | Inspect definitions without suggesting repeated refresh can recover missing historical fields |
| No Skill/MCP use observed | Inspected scope and completeness | Inspect records or switch turns, without unsupported disable recommendations |
| Refresh failure or expired version | Keep old data and its update time on refresh failure; explicitly stop evidence queries for expired versions | Retry or refresh, without automatically widening read permissions |
| Inaccessible source or unsupported host | Explain inaccessibility or lack of support while keeping other available content | Use existing source management to resolve authorization, or return to available pages |

Production pages and shared synthetic previews reuse production components. Final automated browser acceptance covers finding a turn and returning, overlapping durations, three Skill reads counting as three uses, distinguishing zero/unrecorded/read failures, append refresh, and sharing the same batch. Chinese/English, 390/1440 widths, long names, many operations, and keyboard paths are checked. Rule journeys cover keep, redisplay, not applicable, comparable resolution, version-incomparable rechecks, and insufficient evidence. DOM and interaction evidence do not establish pixel-level visual or all assistive-technology acceptance.

MCP has a separate timeline category. Calls with reliable endpoints form tracks; calls without endpoints retain use records and available native duration. Timing summaries share localized gap, calculation-basis, and source-state explanations. CLI optimization text separates user decisions, current/original checks, and recheck comparability. Product translation checks and bilingual browser flows detect missing translations; source content and stable protocol values remain unchanged.

Shared previews provide complete, unrecorded-time, and running synthetic scenarios with production turn and rule components. Browser acceptance checks navigation hierarchy, list fallback, group refresh, expanded content, return, and focus restoration. Clickable controls and historical screenshots alone do not establish acceptance.

The old standalone prototype pages, styles and simulations have been removed. Production components and the shared preview are the only UI implementation. Old check archives remain historical evidence and do not establish U19 results.

#### 16.4 Visual direction and structural sketch

Retain Wombat's white canvas and restrained typography, making the execution timeline the primary visual focus. Use compact summary rows and lists for usage records rather than equal-sized cards everywhere, redundant borders, shadows, or decorative numbering. Left-align text and align tabular numerals within columns. Retain system fonts and Chinese fallbacks without adding remote font dependencies. Follow existing title, body, and supporting-text scales while keeping supporting text legible.

Reuse six existing light-theme colors: canvas #ffffff, text #0d0d0d, supporting text #6b6b6b, dividers #e6e6e6, selection support #507e95, and gap notices #875925. Implement with theme variables and corresponding dark-theme mappings; these values are not proof of contrast acceptance. Pair uncovered intervals with texture and text, and selection/focus with explicit outlines rather than color alone. Limit motion to user-triggered expansion, selection, and refresh feedback, respecting reduced-motion preferences.

This structural sketch specifies information placement only. Retain existing task navigation on wide screens; stack summaries and provide an equivalent operation list on narrow screens. Product locale dictionaries supply Chinese and English labels.

```text
Back to task                         Turn N
Completed                            Updated ...

Duration             Tokens
Value / unavailable  Value          [Usage breakdown]

Execution                            [Time distribution]
Commands   =======
MCP           =========
Compaction                 ===
[Select an activity to inspect its evidence]

Skills / MCP used
Name                                 Uses
[Expand individual operations]

[More metrics and calculation definitions]
[Share summary]
```


## Alternatives considered

The [overview](2026-10-04-codex-task-timing.en.md) owns shared rejection reasons for CLI-only raw-log calculation, additional telemetry as a prerequisite, and arbitrary exclusive causal allocation. Domain-specific distinctions above retain useful observed results without weakening identity, privacy, or user-decision protection.

## Consequences and verification

The consequences of this decision are verified through independent synthetic module tests and final real-chain acceptance. Current behavior belongs to module references; the [overview](2026-10-04-codex-task-timing.en.md) owns scope and platform/resource limits. Missing source evidence does not become zero or a successful check.
