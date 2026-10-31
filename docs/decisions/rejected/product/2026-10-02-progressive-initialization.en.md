# Decision Note: progressive project initialization and failure acceptance

[中文](2026-10-02-progressive-initialization.md) | English

Status: rejected — mandatory initialization and browser flow removed

The [CLI and task Skill decision](../../implemented/architecture/2026-10-31-cli-task-skill.en.md) supersedes this proposal. Its rationale and original acceptance ideas remain historical. No initialization or browser workflow is scheduled.

## Problem

First reads of large histories may lack complete results. Existing provisional task views are not project-level progress. Without real project events, reliable project/worktree attribution, and individual retries, copy or prototypes cannot establish complete initialization. This proposal owns remaining A5 acceptance and the former initialization design, without repeating delivered A1–A4 and static B branches.

## Proposal

The [core guide](../../../reference/core.en.md) owns current discovery, incremental indexing, recovery, and commits. The [UI README](../../../development/architecture.en.md) owns current Web query lifecycle, and the [project-discovery decision](../../implemented/architecture/2026-10-04-observed-project-discovery.en.md) owns authorization rationale. Extend this flow with project events, progressive commits, independent failures, and retries, without a separate one-time import state or button-click completion heuristic.

### Real events and project identity

Events require read-cycle/source identities, reliable project or working-directory identity/path, waiting/reading/committed-usable/failed states, an openable task/version, coverage gaps, cancellation scope, and error codes. Derive events incrementally from scanning without reparsing history to count it. One completed file does not establish a completed project. Projects may read concurrently; elapsed time does not establish size or percentage.

Project/worktree attribution requires evidence. Disambiguate names with directories and represent working-directory-only evidence as such rather than treating each worktree as a project. Creating a folder does not produce a project record. Do not scan entire disks or execute project files. Historical sources and current configuration retain separate semantics.

### Progressive availability and interaction

Render the shell first with independent connection, preference, and account loading. With no results, display “Preparing history” and actual activity; without project evidence, display “Finding record locations.” Then show a stable list with waiting, organizing, usable, or failed project states rather than guessed ETA, percentage, or per-file logs. Roughly one-second pending feedback and a ten-second longer-wait explanation are interaction suggestions, not performance promises.

Only committed openable records enable task entry; incomplete totals explicitly indicate that history is still being read and usage is incomplete. Completion must not navigate away from current content. One project's failure does not clear others. Individual retries reread only the failed object; continue-viewing reuses work rather than clearing the index or unconditionally starting another scan.

Stopping waiting cancels only this page's request/follow-up, not shared scanning. First cancellation without any committed result explicitly has no result; windows cannot cancel each other. Coalesce screen-reader announcements for changes in identified counts, active projects, or availability.

## Alternatives considered

Waiting for full history blocks already usable results. Guessing progress from time, file counts, or design samples misrepresents partial data as complete. A separate import flow duplicates index lifecycle, so reuse the service and transactions and expose project states only with sufficient real events.

## Acceptance criteria

Project events, commits, attribution, failure presentation, and retries remain unimplemented/unverified; generic reading states and provisional tasks are insufficient substitutes. Existing module tests carry delivered-behavior regressions. Former B01–B26 below retain the scope needing full-journey evidence without a duplicate pass-status table.

| IDs | Data/actions | Key assertions |
|---|---|---|
| B01–B05 | New data directory; active/archive, older history only, no logs, unmetered tasks, cancellation during large history | Readable normal results without duplicate archive billing; empty dates offer all dates; unknown is not zero; pending differs from no records, and first cancellation has no older result |
| B06–B10 | Invalid/denied directory, unknown format, offline missing prices, one failed source among several, unmatched combined filters | Accurate scope/capability/reasons; prices do not block tokens; healthy sources and retained contributions are accurate; clearing filters does not change authorization |
| B11–B16 | Time-zone midnight/unknown dates, multiple models/missing turns, same names across sources/archive, unauthorized/deleted configuration, limits/encoding, reads/failed MCP calls | Layered conservation without guessed identity/dates; current unknowns do not explain history; reads are not calls, and failed attempts remain |
| B17–B19 | Imported old history, no recommendations/all ignored/unavailable checks, deletion/method changes after manual edits | No invented continuous inactivity; three states remain distinct; retain decisions/baselines and never resolve with insufficient evidence |
| B20–B23 | Late responses after scope changes, cancellation in one of multiple windows, restart/expired versions, cross-day preferences/manual dates | Scope/version isolation without canceling others; recover through a new link; relative dates roll/fixed dates remain; preference failure does not block |
| B24–B26 | Unsupported index formats/full storage, two languages at 390px with keyboard, fixed-version CLI/HTTP | Storage failure is not success and retains user state; reachable actions/identities; equal values/counts/recommendations |

Also cover new projects while the host runs, direct configuration entry, identical directory names, unattributed records, unreadable projects, and forged browser paths. Bind older results to scope/version and protect user selection. Reject unknown formats while retaining data; storage failures never claim saved success. Use synthetic sources and external temporary directories rather than real conversations.

Measure URL readiness, shell, first valid usage, and section completion separately. Record release, corpus, cache, platform, Rust/Node/browser memory, and temporary/settled disk. Historical CLI measurements cannot be added into GUI ETA. The [live-index proposal](../../proposed/architecture/2026-09-30-live-usage.en.md) alone owns million-record, service-RSS, and 24-hour targets.

## Risks

Project commits may conflict with source-wide reconciliation, cross-file inheritance, and single-writer transactions; establish accounting conservation and version consistency first. Events and openable records must share a version. Cancellation, late responses, sleep recovery, and faults cannot publish partial results. Static pages, controlled quota exhaustion, and local tests do not establish real disk failure, every permission case, or platform acceptance.
