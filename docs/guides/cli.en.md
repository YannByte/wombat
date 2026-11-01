# Complete Wombat CLI guide

[中文](cli.md) | English


Wombat helps you find work records and usage, locate opportunities for improvement, inspect cleanup candidates and recheck changes. Run commands directly, or describe your task in Codex. The Wombat Skill selects commands, explains evidence and assists with the next action.

For your first use, start with [Quick start](#quick-start). To find an operation, see [All commands](#commands). For precise queries, scripts or Skill integration, see [Advanced options](#options) and [Automation interfaces](#automation). For problems, see [Results and recovery](#recovery). Look up unfamiliar words in [Terms](#terms).

<a id="quick-start"></a>

## Quick start

### Check installation

Install Wombat with the [installation guide](installation.en.md), then enter this in your terminal:

```sh
wombat --version
```

A version number means the program can start. If the terminal cannot find the command, check its location through the installation guide instead of repeatedly reinstalling. Local record queries need no model API key or initialization command.

### Start with one task

Choose one line to run at a time. Ordinary queries and inspections do not change original work records.

| Your task | Enter |
|---|---|
| See today's usage | `wombat usage` |
| See this week's usage | `wombat usage week` |
| See recent work | `wombat history` |
| Find an earlier discussion about release failures | `wombat history search "发布失败"` |
| Inspect the current project | `wombat check` |
| Review Skill catalog footprint and use records | `wombat check skills` |
| Inspect cleanup candidates in the current project | `wombat clean` |
| Check installation and plugin discovery | `wombat system doctor` |

Run `wombat` to see the five entries and common examples without starting a query or scan. Run `wombat COMMAND --help` for that command's help, for example `wombat history --help`. AI callers can use `--help --json` for task examples and next steps in one read. Known commands run directly; only unfamiliar exact fields need the selected method's Schema.

### Understand a result

Results state the project, dates, timezone and data status first. They then show totals or relevant entries and the next action. Check the scope before interpreting the result.

No matching records means none were found in the completed scope. Data not ready or unreadable records mean a complete conclusion is not yet available. Unavailable metrics stay unknown instead of becoming zero. A partial page states whether more content remains and supplies a command to continue.

An ordinary usage query can first return the prepared portion of the data. Use `--fresh` to wait for this update. If waiting times out, check the data status rather than treating provisional output as final statistics.

### Select a project

`usage` and `history` default to all projects in authorized record sources. `check` and project-mode `clean` default to the project directory where the terminal is running. Results state the selected scope in both cases.

Use `--project` when running elsewhere or selecting a single project. Replace the example path with the actual project folder and retain the quotes around it.

```sh
wombat usage week --project "/path/to/My Project"
wombat history --project "/path/to/My Project"
wombat check --project "/path/to/My Project"
wombat clean --project "/path/to/My Project"
```

A Windows path can be written as `"C:\Work\My Project"`. One `--project` selects both inspection scope and related records; you do not repeat the read directory and filter directory. Projects come from explicit records or selected directories, never guesses from keywords in folder names. History and usage filter recorded project identities even if the original directory no longer exists. Current inspection and cleanup require an existing readable directory.

If the current project cannot be identified, inspection and cleanup explain the missing scope instead of scanning the entire home directory. An interactive terminal can ask for a folder; scripts should supply `--project` directly.

<a id="commands"></a>

## All commands

The five top-level entries are `history`, `usage`, `check`, `clean` and `system`. This table lists the guide's complete set of operations. Detailed data capabilities remain available through `system api` and `system call` without more top-level entries.

| Entry | Operations | Default result |
|---|---|---|
| `history` | List, `search`, `show`, `read` | Recent work, matching positions, record information or source fragments |
| `usage` | Time selection, `top`, `stats`, `compare`, `alerts` | Totals, task rankings, task distributions, scope comparisons or budget alerts |
| `check` | Project inspection, `skills`, `mcp`, `rules`, `hooks`, `--task`, `verify` | Problems, observations, review leads or original-condition comparisons |
| `clean` | Project preview, `--codex`, `verify`, `--apply` | Candidates and protection reasons, or explicitly selected cleanup results |
| `system` | `doctor`, `update`, `api`, `call`, `hook codex` | Maintenance, interface discovery or integration input |

## Find work records: history

### List recent work

```sh
wombat history
wombat history --project "/path/to/project"
```

The default lists work records active within the last 30 local calendar days including today, with 20 entries per page. Activity evidence includes conversations without Token records, without changing usage accounting. Each entry shows its topic, project, latest activity and how to inspect it. A work record represents a source conversation. Its title, recorded operations or record count do not prove completed work.

### Search earlier discussions

```sh
wombat history search "发布失败"
wombat history search "验收" --project "/path/to/project"
```

Query text is a search term, not an instruction to execute. Results show matching records, source positions and read cursors to help identify the target. Search uses the same default 30-day window. Specify dates or `--all-time` for older content.

An unfinished scan states the visited scope and how to continue. It cannot establish that no other matches exist. Pages retain a fixed query view so updates do not mix different versions of the records.

### Inspect a record

```sh
wombat history show "发布流程"
wombat history read "发布流程"
wombat history read "发布流程" --find "验收"
```

`show` has the help description “Show record details” and returns record information, turns, recorded operations and available evidence. These do not establish verified delivery. `read` has the help description “Read conversation text” and returns source text with speaker, time and neighboring context. `--find` locates relevant fragments in the selected record, which is useful for long conversations.

Select a record with its full ID or a title query within the current scope. One match opens that record. Multiple matches first show time, project and topic. An interactive terminal lets you choose one before reading. Choice numbers exist only within that invocation and are not saved as cross-command shortcuts.

Scripts should use full IDs. An ambiguous title returns an error and candidates instead of reading the first result. Returned continuation commands bind the selected record and original view, so later reads cannot silently select a different title match.

After explicitly selecting a record without date bounds, you can inspect its complete available history beyond the discovery list's default 30-day window. Explicit dates, sources and projects always remain in force; `show` and `read` cannot silently broaden them. Source text returns in fragments with continuation instructions instead of dumping a long conversation. `--turn` selects an exact turn. Finer positions, surrounding context and cursor controls remain in the automation interface.

### Resume work or prepare a report

To resume work, use `history search` to find the record, `show` to locate operations, then `read` when needed to confirm the original discussion, delivery locations and unfinished work. A recorded completion claim is evidence to check. Actual files and tests still need verification.

Daily and weekly reports combine work records and usage under the same dates and project. In Codex, ask for this week's project work, Tokens, unverified items and record locations. The Skill reads relevant fragments and separates user requests, recorded actions and verified outcomes. Knowledge documents use the same evidence and a user-selected destination rather than automatically creating another knowledge base.

## Inspect usage: usage

### Common time windows

```sh
wombat usage
wombat usage yesterday
wombat usage week
wombat usage last-week
wombat usage month
```

| Time selection | Window |
|---|---|
| Omitted or `today` | Today |
| `yesterday` | Yesterday |
| `week` | Monday of this week through today |
| `last-week` | The previous complete calendar week, Monday through Sunday |
| `month` | The first day of this month through today |

Dates use the local timezone, and results state actual dates and the cutoff. Use `--timezone` for another zone. Original event timestamps do not change.

Totals include recorded usage across the complete matching scope rather than adding the few displayed entries. Input, output and caches retain separate meanings. Missing source fields or incomplete reads state which values are unavailable or only known portions.

### Find higher-usage tasks

```sh
wombat usage top
wombat usage top week
wombat usage stats
```

`top` defaults to the five tasks with the most Tokens today. Add a time selection to change the window. Results state complete totals, the ranking scope and omitted entries. A higher rank does not by itself establish waste.

`stats` defaults to task distributions over the last 30 local calendar days. Mean, median and P90 use tasks with complete totals; incomplete tasks and unassigned usage remain separate. P90 is a point at or below which about nine tenths of that population falls, not a budget or efficiency score.

Use advanced options or automation to filter by project, model, reasoning effort, source or record. Tasks can use several models, so model-group task counts are not simply additive.

### Compare changes

```sh
wombat usage compare
```

The default compares the two most recent completed calendar weeks rather than an unfinished current week against a complete previous week. Results state both date ranges, timezone, totals and distribution changes. Supply both sets of boundaries to compare current-week progress or other windows.

Custom periods must have equal lengths and no overlap. Use `--task RECORD --against RECORD` for two tasks, adding `--family` when recorded descendants should be included. Record selection follows `history show`; scripts use full IDs.

Changes can be examined alongside task counts, per-task usage and model distribution. These statistics do not automatically establish causes or work output. Cost estimates and account allowances remain auxiliary observations, separate from Token budgets.

### View budget alerts

```sh
wombat usage alerts --once
wombat usage alerts
wombat usage alerts --id PLAN_ID --interval 60
```

Save a budget plan through the Skill or automation first. With no enabled plan, the command states that no budget is configured instead of implying success through silence. By default, `alerts` monitors continuously and emits only new notices. `--once` checks once and exits. The interval defaults to 60 seconds and accepts `--interval 5..3600`.

Ctrl+C stops this alert-monitoring invocation. Monitoring runs only while the command runs; it installs no scheduled task and sends no messages. Select plans with repeatable `--id`. Token budgets, account allowances and cost estimates remain separate. Partial usage can prove a crossing, not remaining allowance.

`alerts` uses saved budget-plan scope rather than the ordinary today default of `usage`. Time words, dates and project filters cannot override a plan. Select plans with `--id`.

## Inspect problems: check

<a id="project-diagnosis"></a>

### Diagnose the current project once

```sh
wombat check
wombat check --project "/path/to/project"
```

This is the combined project-fact entry. Separate check commands are not required. Results combine project rules/configuration checks, relevant Skill/MCP footprint and use evidence, recent work leads and read-only project temporary/build candidates. Leads guide further detail reads without reading every conversation or expanding every Skill body.

The fact report starts with project, actual dates, timezone and coverage for each part, then shows deterministic findings, investigation leads and evidence locations. Invalid configuration, broken references, a declared copy differing from its source or a verifiable target problem in an effective Hook can yield concrete findings. Long files, repeated blocks, high usage or absent use records remain review leads rather than automatic corrective advice.

The default scope is the current project and configuration explicitly applicable to it. History covers the last 30 local calendar days including today. `--project` selects both the current directory and records with explicit project identity. `--since`, `--until` and `--timezone` adjust history without changing the version represented by current files. User caches and unrelated global configuration are outside default cleanup scope.

Unavailable history does not block independent current checks. Results identify missing or truncated portions; no history does not mean no project problems. Each part retains its own read version and recheck basis without claiming a single complete project snapshot. Unclear project identity, invalid arguments or inability to execute the whole request produce explicit errors.

Combined reports reuse existing accounting rules and core totals. Combining leads does not change Token or cost results, and selected records are not summed as the whole project's usage.

The command does not edit project files or enablement, install dependencies, build, start MCP services or clean artifacts. Wombat can update its derived data for evidence reads. `--json` returns structured facts while progress uses stderr. Noninteractive execution does not wait for choices, and insufficient evidence retains gaps.

CLI does not call a model to judge code mechanisms or apply corrections. For the same task through the Skill in Codex, Codex traces necessary code, proposes selectable actions and performs authorized edits and verification after confirmation or under existing explicit authorization. See [project diagnosis](plugin.en.md#project-diagnosis) for report, selection and recovery. No extra top-level command or general automatic-fix switch is introduced.

### Inspect Skills, MCP, rules and Hooks

```sh
wombat check skills
wombat check mcp
wombat check rules
wombat check hooks
```

| Target | Result focus |
|---|---|
| `skills` | Origin, project scope, native enablement, catalog Token estimate, recorded use and review leads |
| `mcp` | Configuration origins, identifiable tool calls and resource reads, last use and evidence gaps |
| `rules` | Instruction files, references, repeated blocks and drift through explicit relations |
| `hooks` | Observed native registrations, enablement and trust, and verifiable target problems |

The use window defaults to the last 30 local calendar days including today. Order frequency and footprint only when units are comparable and evidence permits it. No observed use does not mean no value: rare release or recovery capabilities can remain enabled. Partial records show confirmed counts and gaps rather than complete zero use.

Skill catalog estimates encode reference names, descriptions and locators. On-demand body quantities remain separate. The estimate is not all context actually visible to the current model. MCP definitions without reliable payloads remain unmeasured; Hook script size is not context occupancy. Current enablement, historical loading, actual use and service reachability are distinct facts.

### Review a work process

```sh
wombat check --task "发布失败"
```

Select the target record, then inspect evidenced leads such as retries after failure, repeated reads or requests and input changes. Select a turn when prompted or use `--turn` for an exact target. CLI supplies positions and evidence. The Skill uses project code to propose a specific change and verification method.

Expected negative tests, required validation, reasonable retries or long waits do not automatically establish defects. Missing times, outcomes or target identities retain gaps instead of invented complete durations or causality.

### Verify a change

An inspection returns a complete command to copy for recheck. Copy it from the original result instead of reconstructing the read view, baseline, scope and rules. Through the Skill, ask to recheck the original problem under the same conditions.

Recheck binds the original problem and conditions. Results distinguish a remaining hit, a comparable miss, incomparable conditions and insufficient evidence. Only a valid comparison under the same conditions establishes that this check no longer observes the problem. Engineering fixes also require the original project tests.

Original views are temporary and are not guaranteed across service restarts. Reinspect when a view expires; a new inspection cannot recover the old before/after conclusion. Keep and not-applicable are user decisions, not repair results, and recheck does not revoke them automatically.

## Preview and perform cleanup: clean

### Inspect candidates first

```sh
wombat clean
wombat clean --path target --keep dist
wombat clean --codex
```

The default only previews, with output starting “Preview only — no files deleted.” Project mode inspects regenerable build outputs and explicitly selected temporary paths. Repeat `--path` to select paths resolved from the current project. `--keep` preserves pending outputs or other selected material. Tracked files, user assets, source records and required state do not become deletion targets merely because their names resemble caches.

`--codex` selects supported caches and old stages for the current user's Codex installation across projects, currently targeting macOS Codex layouts. It does not mean every platform, every AI product or the entire home directory. This mode rejects `--project`; use explicit absolute paths with `--path`. Explicitly add `--wombat-temp` for Wombat's own pending publication material. It cannot combine with `--codex` and excludes published indexes, saved snapshots and recovery generations.

Results separate candidates, review, protected, unknown and absent-at-path entries. They state ownership, regeneration basis, observed size and conditions that affect selection. Review deliverables, pending consumers and current use before choosing candidates. Logical size is not actual reclaimable space.

### Confirm Codex cleanup in the terminal

This operation permanently deletes selected supported Codex candidates. Do not select material you need to keep. Codex native tools perform project-output deletion; this entry does not.

```sh
wombat clean --codex --apply
```

The command inspects current candidates again, shows full paths and evidence, then asks for specific selections and confirmation. Enter alone never selects everything or confirms deletion. Cancellation or an empty selection starts no removal. Selection exists only within this invocation; it does not read an implicit previous cleanup list.

Before removal, it checks selected identities, current use, versions and protection conditions again. New candidates are not added. Changed entries, protection or unknown evidence stop subsequent work, reporting removed, unprocessed and failed entries separately. Cancellation does not undo completed permanent deletion.

Without an interactive terminal, the command requires an explicit original inspection and selection instead of waiting for confirmation. See advanced options for scripts. No force removal bypasses unknown or protected results.

### Verify cleanup

Copy the `clean verify` command returned by cleanup to inspect the original entries. It compares the original selection without expanding cleanup scope. For project artifacts, the Skill checks ownership and pending consumers, uses trusted tools for authorized paths, then rechecks the same scope.

Absence at the original path describes that path's current state, not actual freed disk space. Permanent deletion has no general restoration mechanism. Kept and unprocessed entries remain separate in results.

## Check and maintain installation: system

### Check the runtime

```sh
wombat system doctor
wombat system doctor --project "/path/to/project"
wombat system doctor --hooks
```

`doctor` reports installation, plugin discovery, runtime-resource matching and current-project identification separately. Add `--hooks` for native Hook registration evidence. It installs nothing, invokes no model and builds no conversation-usage index.

An unknown or failed check does not erase completed siblings. Results explain affected capabilities and concrete next actions, such as choosing a project, repairing discovery or resolving a resource-version mismatch. Not every check failure requires reinstallation.

### Check or install updates

Check for updates first. This command installs nothing.

```sh
wombat system update --check
```

When you decide to upgrade, this command installs the latest release.

```sh
wombat system update
```

For a specific release, replace `VERSION` with an actual version and run this separately:

```sh
wombat system update --version VERSION
```

`update --version VERSION` selects the version to install, unlike `wombat --version`, which reads the installed program version.

Updating the runtime and matching plugin resources are separate results. When plugin synchronization is needed, output states the next action. A successful runtime update does not establish matching plugin discovery in the current project. See the [installation](installation.en.md) and [plugin](plugin.en.md) guides for prerequisites and recovery.

### Interfaces and integration entry

`system api` and `system call` serve Skills, scripts and users needing detailed control. They retain service status/start/pause/resume/stop/autostart, account, budget, prices, source-directory authorization and preference capabilities.

`system hook codex` is integration input, not a daily inspection command. Its separate advisory receipt protocol accepts at most 64 KiB within four seconds, emits no stdout and does not block host work on receipt failure. Receipt does not prove native registration, loading or complete collection, and cannot change permissions or model context.

<a id="options"></a>

## Advanced options

### Shared conventions

Options are accepted only where applicable. Unsupported options, repeated single-value arguments and conflicting combinations return an error and a correction example rather than being silently ignored. Use complete command names, not automatic abbreviations. Quote paths and query text containing spaces.

| Option | Purpose and scope |
|---|---|
| `--help`, `-h` | Help for the current entry or subcommand, without queries or changes |
| `--version` | Installed program version at the top level |
| `--lang en|zh` | Presentation language; also available through `WOMBAT_LANG` |
| `--json` | Structured output; watches emit one JSON object per line |
| `--no-input` | Disable interactive questions; missing selections or confirmation conditions fail |
| `--project PATH` | Explicit directory for queries, checks and cleanup supporting project scope |
| `--since DATE --until DATE` | Paired custom `YYYY-MM-DD` dates, inclusive start and exclusive end |
| `--timezone ZONE` | Date interpretation zone, such as `Asia/Shanghai` or `UTC` |
| `--all-time` | All available dates including undated records; incompatible with date bounds |
| `--root PATH` | Repeatable record-source directory, defaulting to `CODEX_HOME` or `~/.codex`, distinct from a project folder |
| `--source ID` | Explicit source-instance filter in supported queries |
| `--limit N --offset N` | List paging without changing complete-scope totals |
| `--fresh`, `--cached` | Wait for a usage update or read only prepared data; mutually exclusive |

Uppercase terms such as `DATE`, `PATH`, `RECORD`, `VERSION` and `PLAN_ID` are placeholders, not literal input. Prefer complete next-step commands from actual results. The Skill retains the target of requests such as “that earlier item”; independent CLI invocations do not depend on hidden last-result state.

### Dates, scope and paging

```sh
wombat usage --since 2026-10-26 --until 2026-11-02 --timezone Asia/Shanghai
wombat history search "验收" --all-time --project "/path/to/project"
wombat usage top week --limit 10
```

Explicit time words cannot combine with `--since`, `--until` or `--all-time`. Without a time word, paired custom dates replace the command's default window. Missing bounds, invalid dates and reversed ranges fail. Dates resolve once at invocation in the selected timezone and remain fixed when paging across midnight.

Ordinary lists default to 20 entries; task rankings default to five. `--limit` accepts 1..200 and `--offset` is a nonnegative integer. Source reads retain byte boundaries rather than using entry counts. Text fragments and position lists each keep their own continuation cursors. Continue through returned complete commands or structured cursors, preserving scope and view.

Full IDs, fixed views and cursors cannot be mixed with incompatible projects, sources or read versions. Reacquire the same scope after expiry or source-file changes. A new read does not pretend to continue an invalid old comparison.

### Usage filters and comparisons

| Option | Purpose |
|---|---|
| `--model MODEL --effort LEVEL` | Model and reasoning-effort filters |
| `--task RECORD` | Select a task; automation uses full identity |
| `--against RECORD` | Compare another work record with `--task` |
| `--family` | Include recorded descendants in task comparisons |
| `--baseline-since DATE --baseline-until DATE` | Baseline dates alongside custom `--since` and `--until` |
| `--group day|week|month` | Calendar grouping without expanding selected dates |
| `--by project|model` | Usage or task-distribution dimension; period comparison also accepts `task` |
| `--sort time|tokens|cost` | Usage-list ordering; `top` accepts only `tokens|cost`, defaulting to Tokens; unknown cost is not zero |
| `--compact --json` | Complete totals and requested entries with fewer quality examples and facets |
| `--snapshot ID` | Explicit fixed usage view, incompatible with refresh |
| `--wait-ms N` | Usage wait deadline, 1..600000 milliseconds, implying `--fresh` |
| `--watch` | Changing live usage, incompatible with fixed views and `--cached` |
| `--rebuild`, `--include-history` | Explicit derived-data rebuild or historical-source inclusion; advanced data operations |

Period and task comparisons cannot combine. Unknown-date/model filters, precise navigation, input trajectories, resource hotspots and risk leads remain in generated automation. Statistics preserve missing, partial and true-zero values without allocating Tokens or costs to tool operations.

### Checks, rechecks and explicit decisions

Combined `check` supports date, timezone, project and source selection. Dates filter related history without treating current configuration as a historical version. `check skills` and `check mcp` also support paging. Skills support `--sort review|context_tokens|least_activity|recent|name`. MCP uses the same options except `context_tokens` without definition payloads. Entries missing footprint or frequency evidence are marked separately rather than sorted as zero.

`check --task RECORD [--turn TURN]` inspects the selected complete turn and cannot combine with date or configuration-check arguments. Date-bounded excerpts use `conversation` date/turn selection. `check verify --read-view VIEW --baseline BASELINE` retains original conditions and accepts the source, project and rule parameters required by the original result. `--agents-bytes` and `--description-characters` adjust only the corresponding product reminders, not format-specification thresholds.

`verify` is the ordinary command name. Automation retains the generated `recheck` action. The naming change does not alter the original problem, read view, scope or comparison conditions.

Use the Skill or `system call` method `optimize` to explicitly keep an item or mark it not applicable, including reason and decision revision. These writes are separate from derived indexes. Inventory details, evidence, related scopes and rule discovery remain available through structured `config` and `optimize` actions.

### Cleanup selection and unattended execution

```sh
wombat clean verify --read-view VIEW --baseline BASELINE --item ITEM
```

The following is an explicit deletion request. Use it only after reviewing the original candidates and supplying actual selections.

```sh
wombat clean --apply --read-view VIEW --baseline BASELINE --item ITEM
```

Replace uppercase placeholders with original inspection values; do not run a deletion template literally. Recheck can omit `--item` to inspect the original scope. Deletion requires explicit entries and accepts repeatable `--item`. Only supported original Codex candidates can be removed.

Apply with an original view rejects new `--project`, `--path`, `--keep`, `--codex` or `--wombat-temp` and does not discover candidates again. It does not wait for confirmation: the explicit apply request and selection authorize the operation, subject to the same final protections. Read per-item outcomes; partial success is not complete removal.

Project mode cannot add `--apply` to delete files directly. Codex uses trusted tools on concrete authorized project paths. Before retrying, establish which entries completed. Cancellation does not undo permanent deletion already performed.

<a id="automation"></a>

## Skills, scripts and automation

### Discover methods as needed

```sh
wombat system api
wombat system api usage --json
wombat system api usage --output-schema --json
```

Without a method, list methods and purposes. Select a method for its input description; `--output-schema` adds the output description. Discovery is local and read-only, with no conversation scan or model call. Known requests can run directly without repeatedly fetching every Schema.

### Execute structured requests

`system call` reads one JSON object from stdin and always returns the method's structured result. Requests can be saved in a file. Directories, dates, full identities and versions should come from explicit selection or earlier results, not title guesses.

```sh
wombat system call < request.json
```

In PowerShell 7, use UTF-8 file input as follows.

```powershell
Get-Content -Raw -Encoding UTF8 request.json | wombat system call
```

For example, put this in `request.json` to request three higher-usage tasks:

```json
{"method":"usage","params":{"query":{"action":"threads","sort":"tokens","limit":3,"compact":true},"mode":"auto"}}
```

Structured requests are independent of ordinary commands' natural-date defaults. This example supplies no dates, so interpret the returned `scope`. Supply explicit dates and timezone for today or a selected period. Retain returned fixed-view identities; an ordinary fresh read does not save a snapshot.

Input is one object up to 1 MiB, read within ten seconds; output is at most 2 MiB. Ordinary commands can prompt only when both stdin and stdout are interactive terminals and neither `--json` nor `--no-input` is set. Other cases and `system call` never wait for record selection or deletion confirmation. Ambiguity, missing params and scope conflicts return errors in the selected format without broadening scope or selecting the first match.

### Complete method scope

`history read` JSON contains `result` with bounded text, speaker, time and fragment cursors, plus `nextLocations`, a generated request for the next location page. Retain the two cursor types separately. Continue the latter through method `conversation` with `system call`; text paging never changes complete usage.

| Method | Actions | Purpose and boundary |
|---|---|---|
| `usage` | `usage`, `threads`, `turns`, `steps`, `statistics`, `compare`, `investigate`, `trajectory`, `resources`, `review`, `context`, `refresh` | Current or fixed usage/analysis, returning `result` and data status |
| `snapshot` | Corresponding fixed-view queries and explicit `refresh` | `refresh` saves a snapshot, unlike ordinary freshness |
| `project` | Assemble current project evidence | Configuration, checks, records, original usage and cleanup candidates; independent status/read versions without project edits |
| `conversation` | `records`, `activity`, `locate`, `search`, `read`, `changes` | Activity, locations, source text and source changes; changes are not accepted code delivery |
| `config` | `list`, `detail`, `evidence`, `related_scopes`, `capabilities` | Read-only configuration, measurements, native observations and associated use |
| `optimize` | `list`, `detail`, `checks`, `activity`, `capabilities`, `recheck`, `keep`, `not_applicable` | Checks and comparisons; only explicit decisions write user storage |
| `artifacts` | `inspect`, `recheck`, `apply` | Cleanup observations and protected Codex removal, without conversation indexing |
| `timing` | `summary`, `evidence`, `capabilities` | Exact task/turn process evidence, without replacing missing time with zero |
| `setup` | Observe runtime and native integration | Compatibility and discovery, not installation or model work |
| `service` | `status`, `start`, `pause`, `resume`, `stop`, `enable`, `disable` | Local query service and explicit autostart control |
| `account` | `read`, `refresh`, `history` | Account and allowance observations, not a project's inferred subscription balance |
| `monitor` | `list`, `upsert`, `remove`, `check`, `acknowledge` | Budget plans and notices; saving a plan does not start alert monitoring |
| `prices` | `status`, `update`, `auto_update` | Auxiliary pricing; failures preserve prices and do not change raw Tokens |
| `directories` | `list`, `authorize`, `revoke` | Explicit path/purpose directory grants |
| `preferences` | `get`, `set` | Supported preferences such as presentation language, without source-text changes |
| `collection` | `status`, `events`, `configure`, `pause`, `resume` | Optional receipt observations/control, not proof of complete history |

Generated Schemas own concrete fields, combinations, versions and limits. Do not invent params from this table. Selection structures for `usage`, `snapshot`, `conversation` and `timing` are distinct. Retain full identities, snapshots, original filters and each cursor independently.

### Retain scope and handle failures

The Skill translates questions into scoped queries, selects evidence and uses native tools for authorized changes or recovery. Daily reports, resuming work and knowledge organization combine these capabilities rather than adding a report database, automatic execution history or default daily delivery.

Scripts inspect exit codes, data status and per-item outcomes. A query timeout can release this caller while shared indexing continues. A write timeout does not prove no write occurred. Inspect current state before retrying; do not automatically repeat removal, grants or decision writes.

<a id="recovery"></a>

## Results, errors and recovery

### Data status

| Observation | Meaning | Next action |
|---|---|---|
| Complete result | A usable complete result for this scope | Inspect detail or continue your task |
| No matches in scope | The completed query found no match | Check terms, project and dates; expand scope explicitly if needed |
| Preparing data | This update has not completed | Use the known portion or wait with `--fresh` |
| Partial result | Some sources, times, identities or operations are unresolved | Use known facts, select a precise record and retain gaps |
| Unmeasured metric | No reliable observation for that metric | Do not fill zero or infer disabling, deletion or improvement |
| Expired original view | The old evidence conditions can no longer be used | Requery or reinspect without reconstructing prior success |

### Common problems

| Problem | Response |
|---|---|
| `wombat` command not found | Check command location and terminal environment with the installation guide |
| A historical record is missing | Check source, project, dates and terms before explicitly requesting older data |
| Several records share a title | Choose by time/project interactively or use a full ID, never the first match by default |
| Current directory is not the target project | Supply `--project` or an explicit folder at the terminal prompt |
| Installed program but missing plugin discovery | Run `system doctor` and follow the specific discovery/resource result |
| Unsupported config format or arguments | Read command help or method Schema and correct input for the original task |
| Read scope or budget exceeded | Narrow project, task, dates or turn; continue the original cursor instead of repeatedly broadening reads |
| Source changes during reading | Reacquire the same scope and continue with a new view |
| Unsupported index or snapshot format | Preserve original data, explicitly rebuild derived indexes or recreate the needed snapshot |
| Cleanup changed, protected or unconfirmed | Retain completed outcomes and reinspect concrete remaining objects without force removal |

### Rebuild derived indexes

Use this command only when diagnosis explicitly requires a derived-index rebuild. It rereads selected sources and can require waiting. It does not repair an unsupported old saved snapshot. Retain the original `--root` options when using selected sources.

```sh
wombat usage --rebuild --wait-ms 600000
```

The valid generation remains until the new index succeeds; user decisions and grants remain separate. For a new saved snapshot, inspect `system api snapshot --json`, then explicitly request `snapshot` action `refresh`. Do not delete source conversations to resolve index problems.

### Output and exit codes

Ordinary results use stdout; progress and diagnostics use stderr. `--json` and `system call` return structured results and errors. Original content and protocol identifiers are not translated. Piping output does not silently change its format.

| Exit code | Meaning |
|---|---|
| `0` | Completed according to the command's contract, including complete empty results |
| `2` | Usable output with partial, provisional or qualified state |
| `1` | Invalid input or execution failure |
| `130` | User cancellation of this invocation |

An unknown optional metric does not automatically fail the entire task; consider scope coverage and result state. `system hook codex` retains its separate advisory receipt protocol. Its exit code does not establish a successful business check.

### Source and recovery boundaries

Work records remain read-only. Indexes are rebuildable derived data; explicit user decisions, identities and grants remain separate. Rebuild retains existing data until a new generation is ready and switches only after success. Failure preserves the valid generation. Unknown formats do not trigger implicit migration or deletion.

History and statistics come from local records; queries invoke no model. Program updates and price completion can use the network. Set `WOMBAT_AUTO_PRICES=0` to disable automatic price completion when needed. Fixed views retain their pricing basis. Sending record content externally requires separate authorization.

Cache hits, lower usage, an absent candidate path, a keep decision or a successful check command do not automatically establish delivery or improvement. Return to the original problem, the same conditions and actual project validation when a conclusion is needed.

<a id="terms"></a>

## Terms

| Term | Meaning in this guide |
|---|---|
| Work record | A source conversation, not necessarily completed or accepted work |
| Turn | An identifiable interaction span within a record, not each individual tool operation |
| Token | A model content-processing unit, distinct from characters or words |
| Source | Selected local record directory and its identity, distinct from the inspected project directory |
| Read view | A fixed data version for consistent paging and follow-up evidence |
| Saved snapshot | Explicitly saved query data, not automatically created by ordinary reads or waiting for updates |
| Baseline | The original problem and comparison conditions needed for a valid before/after check |
| Cleanup candidate | An object with cleanup evidence that still needs purpose/protection review, not deletion authorization |
| Schema | Machine-readable descriptions of request and result fields |
| ID, cursor | Program identifiers for records, objects or continuation pages; ordinary users should copy complete returned commands |

## Continue using Wombat

Remember only the command for your current task. Use its `--help` for more, or describe your goal in Codex and let the Wombat Skill select scope, locate evidence and retain recheck conditions. Generated descriptions own detailed fields. Sources, accounting and protection conditions stay consistent across human and automated calls.
