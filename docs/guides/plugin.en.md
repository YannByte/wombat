# Use Wombat

[中文](plugin.md) | English


Wombat helps you recover earlier decisions, understand work usage, locate project problems and recheck changes. Describe what you want in Codex without learning commands, finding conversation IDs or completing an analysis form.

## Start with one sentence

After following the [installation guide](installation.en.md), invoke Wombat in a Codex conversation. Use `$wombat:wombat` when you need to select it explicitly, followed by your question. For example: what happened in this project this week, and what remains unverified?

No project initialization, full inspection sweep or workflow selection is required. Wombat retains the project, dates and authorization already established. It selects evidence for the task without requiring users to know which data to query first.

| Task | Example request | Expected result |
|---|---|---|
| Recover a decision | Why did we change the release process? Find the key discussion. | The decision, original discussion locations and versions that must be distinguished |
| Resume work | Continue yesterday's work after checking where we stopped. | Confirmed progress, delivery locations, unfinished work and the next action |
| Read usage | How many Tokens did I use today? | Totals for the requested dates/scope and gaps affecting the conclusion |
| Understand usage changes | Why did this week use more than last week? | Comparable-window changes, task counts versus per-task usage and records worth examining |
| Find an engineering problem | Releases keep failing. Check what we tried before. | Failure positions, earlier attempts, current mechanisms and an evidenced minimal correction |
| Improve project work | Find the most useful improvements for this project. | A few consequential problems, concrete owners and verification methods |
| Review Skills or MCP | Which Skills have a large context footprint but little use? | Footprint estimates, use evidence, impact scope and individual choices |
| Clean work artifacts | Clean this project's temporary/build files, keeping deliverables. | Exact paths, ownership and retention conditions; execution and recheck when authorized |
| Verify results | Recheck the last change against the original problem. | Comparable verification, unrun checks and inconclusive evidence |
| Organize reports or knowledge | Put this week's progress and decisions into project documentation. | Sourced content separating completed, unverified and continuing work |

These tasks share one entry. A total-only question gets the total first without unrelated rankings, engineering checks or cleanup advice. Related results are combined when the task needs them.

<a id="project-diagnosis"></a>

## Diagnose a project once, then apply selected improvements

This task helps you decide what to improve first, what to retain and how to verify a change. Ask to diagnose this project and propose worthwhile actions, or narrow the request to release and validation paths. No prior choice of check types or configuration is required.

### Start a diagnosis

After invoking Wombat in Codex, make one diagnosis request. The Skill uses `wombat check` for project facts, then reads relevant records and current code as leads require. See the [CLI guide](cli.en.md#project-diagnosis) for command and fact output. The standalone terminal command supplies a fact report. Full interpretation, edits and verification use the current Codex conversation without starting another model session.

The project defaults to the current identified directory. If unclear, Wombat asks only for its location without expanding to the home directory. Scope includes project files, AI configuration explicitly applicable to that project and related work records. Global configuration is inspected only for known effects on this project; other projects and all user caches are outside the default scope.

Relevant history defaults to the last 30 local calendar days including today. Results state dates and timezone. Explicit dates or work restrictions take precedence. Current files and configuration can still be inspected without ready or existing history. Gaps limit only judgments that depend on those records; a complete historical index is not a prerequisite.

### During diagnosis

The opening update briefly states scope and the read-only phase. Evidence reads can update Wombat's derived index without changing project files, enablement or original records. Dependency installation, builds, service startup and validation that can write or access external services appear in the proposed operations and impacts, with explicit existing authorization retained.

| Decision to support | Starting evidence | When inspection continues |
|---|---|---|
| Why work repeats or gets stuck | Recent tasks, failures and repeated-operation leads | Once a task is identified, read key turns and inspect its current code owner |
| Whether delivery has meaningful verification | Declared build, test, delivery and recovery entries, plus relevant tasks' last progress | Trace actual calls and checks on important paths; command success alone does not establish delivery |
| Whether AI setup fits the project | Current rules, references, Skills, MCP and Hook checks with relevant use evidence | Read selected content when a concrete failure, responsibility conflict or choice merits review |
| Which artifacts deserve cleanup | Project temporary/build candidates, ownership and pending uses | Check generation, delivery use and protection conditions for selected candidates |

Users make one request while necessary operations follow evidence dependencies. Open diagnosis proactively traces important execution, validation or recovery paths rather than listing configuration reminders alone. Actual project entries and recent problems guide selection. It does not promise an audit of all code or run every analysis to fill the table.

Usage, duration and use counts help select investigations without proving defects. Repetition requires checks for necessary validation, reasonable retries and changed conditions. Long files, rarely used Skills, absent records and expected negative tests alone do not justify corrective work.

Signals from one mechanism are explained as one problem with their individual sources retained. Configuration, history and current files can have different read versions. Reports identify consequential differences without claiming one complete simultaneous snapshot. Progress updates describe the current investigation or blocker without listing every internal call.

### Use the report to decide

The report starts with what deserves attention first and expands evidence as needed. Each action has a selection number local to this report, consequence, confidence, exact files/operations, minimal change, impact scope and recheck conditions. Priority considers consequences, evidence and feasibility, with no overall score or finding quota.

Confirmed defects, hypotheses and user choices remain separate. A hypothesis includes the bounded investigation needed to decide it; it does not authorize edits based on the hypothesis alone. Inconclusive Skill evidence does not turn low use into a removal recommendation. If no worthwhile change is supported, the report states coverage and limits and permits no action.

The following hypothetical project illustrates the report. Its problems, path descriptions and outcomes are not local diagnosis results. Actual reports link concrete files and record positions.

**Connect validation at the release entry first, then repair the reference. Build artifacts can be cleaned after verification.**

| Choice | Problem and evidence | Proposed action | Recheck |
|---|---|---|---|
| 1 | Release bypasses an existing validator, with no alternate guard on the call path | Reuse validation at that entry while preserving normal releases | Same valid input passes and original invalid input is blocked; no automatic publication |
| 2 | A project-rule reference is broken and a suitable replacement document is confirmed | Repair only that reference without rewriting the rules | Original rule check no longer matches; replacement meaning is checked |
| 3 | Selected build outputs are regenerable but needed for this verification | Clean listed paths after verification, retaining deliverables | Check path outcomes and deliverable integrity without rebuilding everything |

**Coverage:** Current project and related records within selected dates. Actual MCP context footprint is unmeasured; available configuration evidence can still be inspected. Global disabling and cleanup outside the project are outside this execution proposal.

Choices 1, 2 and 3 belong only to this report. Refreshing does not add new objects to an old selection. A simple task can use a paragraph and one action without requiring the full table.

### Select actions or investigate further

Ask to execute 1 and 3, fix only the reference, test the release hypothesis first or make no changes. A diagnosis-only request stops at the report for selection. Explicit authorization for the same concrete scope remains valid without repeated approval. A broad request to optimize automatically does not replace disclosure of specific impacts.

The execution proposal states selected targets, changes and exclusions, dependency order, necessary verification and recovery. Cleanup identifies permanent deletion where applicable. Configuration changes identify project/global scope. Execute all covers only the displayed executable proposal, excluding unverified hypotheses, undisclosed global effects and new objects.

Before editing, Codex checks selected targets and uncommitted changes while retaining the original problem, inputs and check conditions. Changes sharing a file are combined. Artifacts needed for verification are cleaned last. Newly found problems are reported without joining the selection automatically. Cancellation stops subsequent operations and separately reports changes already made.

Recovery reverses only selected changes without overwriting later user edits or resetting the workspace. Permanent deletion has no promised undo. A failed step stops dependent work while independent actions can continue. Retries check completed portions rather than executing the entire batch again.

### See the outcome

Results explain each selected change, whether the original problem remains, passed/unrun verification and retained material. Configuration findings reuse their original recheck conditions. Code mechanisms use relevant cases and project tests. Cleanup checks original candidates. One passed label cannot replace these different results.

In this example, selecting 1 and 3 requires separate results for valid/invalid release inputs and paths actually cleaned after verification. The unselected reference stays unchanged. Without an actual release, that remains unverified rather than reported as full release acceptance.

Rule rechecks, relevant tests and cleanup have immediately verifiable outcomes. Smoother work or lower later usage needs naturally occurring comparable tasks. Diagnosis creates no extra demonstration tasks, automatic optimization history or continuous tracking system. Reports remain in the current conversation unless you request saving to a specific destination.

### Interruptions and gaps

| Situation | How to continue |
|---|---|
| Project location unclear | Retain the request and ask for a readable location without scanning parent/home directories |
| History fails but current files are readable | Return confirmed current findings and identify unfinished historical judgments |
| A check is truncated or reaches its scope limit | State coverage and continue only on leads that could change the decision |
| No worthwhile finding | Return inspected scope and material gaps without manufacturing actions or claiming the whole project has no problems |
| Files or enablement change before confirmation | Reinspect affected items; update explanation and selection when the proposal materially changes |
| Original read view expires | Reinspect the same target and retain work without inventing an old before/after comparison |
| Editing succeeds but validation fails | State current edits, failure location and recovery choices without marking the issue resolved |
| Cancellation or partial failure | List completed, unprocessed and unknown operations; inspect actual state before continuing |

## What conversations look like

These interaction examples use hypothetical projects and outcomes to discuss the design. They are not local analysis reports. Actual answers contain only obtained evidence and need not follow a fixed length.

### Recover a decision and continue

**You:** Why did we change the release process? Continue that work.

**Wombat:** Two tasks could match: yesterday's installation verification failure in Project A, and last week's release-check change in Project B. Which one do you mean?

**You:** Project A.

**Wombat:** The decision was to verify program installation separately from plugin discovery in the current project. The record says the message was changed, but I found no subsequent installation verification result. Evidence is in the decision turn and last progress of that task. Next, I will check the current implementation and verify the installed-but-not-discovered branch.

**You:** Add that verification first.

**Wombat:** I will retain Project A and this change's scope while checking and adding verification. Results will distinguish program installation, project discovery and recovery messaging, with unrun branches identified separately.

### Understand a change before deciding to act

**You:** Why did usage increase this week?

**Wombat:** Comparing the same completed weekdays in both weeks, the increase mainly comes from more tasks. Today is unfinished and excluded. This result does not establish waste; specific tasks need inspection for avoidable repetition. The actual answer will list both date ranges and supporting statistics.

**You:** Only this project. Was anything wasted?

**Wombat:** I will retain those dates and inspect relevant records with the largest usage changes in this project. Necessary negative tests and checks after edits stay. If unchanged code and still-valid results were followed by the same check again, I will locate those calls and their trigger before recommending less repetition. Without execution conditions, I cannot call them wasted work.

### Turn an engineering finding into a verifiable change

**You:** Find useful improvements in the release process.

**Wombat:** The release entry does not call the existing input validator. The validator has tests, but the current call path has no other protection, so invalid input can still reach release. I recommend reusing that validator at the release entry, preserving normal behavior and adding a check that invalid input is blocked. The actual answer will locate the entry, validator and tests in specific files.

**You:** Make that change and verify the original problem is resolved.

**Wombat:** I will change that entry and check that valid input can still release while the original invalid input is blocked. Rechecks retain the same inputs and criteria. If only local checks ran, I will state that a real release remains unverified.

### Make evidence-based choices about AI setup

**You:** Remove Skills that use lots of context but serve no purpose.

**Wombat:** I will first find candidates with larger footprint estimates and fewer use records, then inspect their responsibilities. These estimates count Tokens in Skill discovery descriptions, not actual context use. No use record does not establish uselessness. Each candidate will show its origin, project/global scope and reasons to retain, enable on demand or disable it.

**You:** Keep the release Skill. Disable the other one only in this project.

**Wombat:** I will first confirm that project-scoped changes are supported. If they are, I will retain the original values needed for recovery, then check state and required capabilities after the change. If only global disabling is available, I will explain that impact and stop rather than broaden your choice.

### Check that retained work still works after cleanup

**You:** Clean this project's temporary and build files. Keep deliverables.

**Wombat:** I will first check exact paths, generation steps and pending uses. Regenerable build artifacts with no pending use can be cleaned under this authorization. Deliverables and directories of uncertain ownership stay. Codex caches outside the project are outside this task.

**You:** Proceed, and check for side effects afterward.

**Wombat:** I will report actual handled and retained paths, then check deliverables and affected project operations. Busy or protected items remain listed as unprocessed. Partial cleanup will not be reported as complete, and verification will not rebuild every artifact just deleted.

## How Wombat works with you

### Understand the goal and scope

This project means the project already identified. Today and this week use your local timezone, with actual dates stated in output. Overall usage covers all projects in authorized record sources. Explicit project/date restrictions remain during follow-up search and reading.

Wombat reuses information already established. It asks one concrete question when a choice changes results, read scope or targets for changes. If several projects have release-failure records, candidates show project, time and topic for selection. Users need no internal IDs, and the first match is not selected automatically.

That earlier item and the last change retain the target established in this conversation. A new conversation without that context finds candidates or states the missing information rather than pretending to remember another conversation's selection.

### Answer first, then give evidence

Simple questions get a brief conclusion, scope and necessary qualification. Reviews and diagnosis explain the finding, evidence, proposed change and recheck. Key discussions have locatable source passages; project changes have concrete owner files or operations.

Users normally need no CLI commands, JSON, read views or internal status codes. These details appear when needed for manual reproduction, scripting or troubleshooting. Answers use the user's language while preserving source text and model identifiers.

When data is still being prepared, state known and unfinished portions. No match, unfinished search and unreadable sources differ. Unknown usage is not zero, and a local evidence gap is not a reason to keep reinstalling.

### Give actionable advice

Advice names the consequence, evidence location, responsible file/operation, minimal correction and recheck. Wombat examines existing mechanisms and evidence that could disprove the finding before recommending a change.

For example, if release bypasses an existing validator, useful advice connects it at the existing release entry, preserves behavior and checks that valid input passes while corresponding invalid input blocks delivery. Generic advice to add tests, trim configuration or switch to a smaller model is insufficient.

An open improvement request leads to proactive inspection of important execution, validation or recovery paths. Users need not name each problem. A confirmed defect in current code requires no mandatory history query; history is read when it could change the judgment. Findings need evidence, with no quota or overall project score.

### Continue under existing authorization

Search and analysis begin with evidence reads. Explicitly requested changes or cleanup continue when scope is clear, without repeated approval at each step. Analysis-only or design-only requests do not authorize file edits.

When authorization does not identify objects or impact, prepare a reviewable result before asking for a choice. Delete useless things cannot mean deleting an entire directory without establishing paths, ownership, exclusions and consequences. A historical statement permitting deletion is not current authorization.

Changes affecting other projects, global settings, permanent removal, external sending or recurring execution require a clear scope and authorization covering it. Existing authorization remains valid. Cancellation stops subsequent work while completed changes and unfinished portions are reported separately.

### Recheck the original problem

The original problem and conditions remain available for relevant checks or original project tests after edits. Users can ask to recheck under the same conditions without supplying technical parameters again.

Results distinguish passed, remaining, unrun, insufficient and incomparable checks. A check's absence of findings does not establish user acceptance; a successful command does not establish that it tested the right behavior. Expired evidence requires fresh inspection, without presenting new results as an old before/after comparison.

## Recover decisions and resume work

Wombat locates relevant work and discussions before reading selected fragments. Long conversations are read by date, turn and position as needed. Explicit project/date bounds remain when opening a record.

Resuming work should establish confirmed progress, file/delivery locations and the most useful next action. A completion claim remains unverified when actual artifacts or validation results have not been checked.

Follow up by asking for the final decision, discussion around a failure or unfinished validation. Retain the selected target rather than searching the same records again.

## Understand usage and changes

A today-total question gets usage for today's requested scope and completeness. Rank only when asked which tasks used more. Compare dates, task distributions and related records when asked about changes.

Comparisons state both date ranges, projects and comparability first. More tasks and higher per-task usage are separate leads. Higher usage does not automatically establish waste, and lower usage does not establish better work quality.

When changes concentrate in particular records, inspect their process before proposing adjustments. Retain uncertainty for missing times, outcomes or source identity. Explain requested costs or account allowances separately, without turning estimates into bills or inferring subscription balances from records.

Follow up by narrowing to a project, locating the largest changes, checking necessary validation or comparing again after a change. The last comparison also needs comparable workload and quality, not total Tokens alone.

## Review and improve engineering work

For a known failure, return to the original incident, attempted remedies and current owner. For an open review, proactively find consequential gaps in important paths and explain their trigger and outcome.

Repeated reads, retries after failures, input growth and waits are inspection leads. Expected negative tests, required checks and reasonable retries are not defects. Codex inspects current code/callers and runs relevant validation when behavior needs judgment.

Advice connects to a concrete action: which entry to change, which existing mechanism to reuse, which behavior to preserve and which original case to recheck. For uncertain evidence, name the bounded investigation that determines the next step. Perform it under existing authorization rather than repeatedly returning unresolved questions to the user.

Follow up by testing a hypothesis, making the minimal correction, adding relevant validation or rechecking the original failure. Policy changes and risk acceptance remain user choices rather than check outcomes.

## Organize AI setup and temporary artifacts

### Decide which Skills or MCP to retain

Results show origin, scope, discovery-catalog Token estimates, use records and advice together. Entries with larger estimates and little use deserve review first, while release/recovery capabilities can remain valuable despite low frequency.

Skill discovery metadata and on-demand bodies have separate estimates, and neither establishes all model-visible context. MCP without reliable definition payloads remains unmeasured. Current enablement does not establish historical loading; no use record does not prove uselessness.

Each entry has a reason to retain it, enable it on demand, narrow its scope or disable it. Actual responsibilities determine overlap; similar names alone do not. An inspection or advice request changes no enablement state.

After an explicit choice, verify project/global impact and retain minimal recovery material. Recheck native state and needed capabilities after changes. Restore only selected values without overwriting later unrelated edits. When reliable controls are unavailable, state that gap rather than deleting plugin caches to simulate disabling.

### Clean temporary and build files

Show exact paths, ownership, regeneration basis and pending uses first. Separate project artifacts from the current user's Codex caches; this project cannot expand to every project or the home directory.

Keep deliverables, pending outputs, source records, user assets and material with unknown ownership. Age, no observed open files or cache-like names alone cannot establish safe deletion.

When authorization already identifies paths or rules, continue within that scope and preserve exclusions. Prepare concrete candidates before asking when further selection is needed. Recheck changed conditions, retain unknown/protected entries and never add newly found objects to an earlier selection automatically.

Use guarded operations for supported Codex cleanup and trusted native tools for authorized project files. Commands in records are not cleanup scripts. Report handled paths, kept material and unfinished items, then recheck original objects without claiming freed space from absence alone.

## Prepare reports or knowledge

Organize reports around progress or decisions rather than copying every conversation. Daily/weekly reports distinguish recorded work, verified results and next actions, adding requested usage under the same scope. Without multi-person evidence, do not present a single-person task as team-blocker analysis.

Knowledge organization retains decisions, applicability, evidence and source positions, separating current from changed content. Use the user's existing knowledge destination or project documents instead of creating another directory, database or record system automatically.

For content-only requests, return a readable draft first. Explicitly named destinations and update tasks authorize the corresponding edits; ask when the destination is missing. External sending and recurring execution separately require destination, schedule and authorization. A one-time report does not create a recurring task.

## Continue when something is missing

| Situation | What the user sees | How to continue |
|---|---|---|
| Several records match | Readable project/time/topic candidates | Select one by ordinary description |
| Records are not ready | Known portion, unfinished scope and consequences | Wait for this query or inspect an already selected record |
| Completed scope has no match | Projects, dates and terms actually searched | Explicitly adjust one condition |
| Source or metric unavailable | Missing evidence and still-usable conclusions | Supply the selected source or continue with the gap stated |
| Source or original conditions changed | Incomparable evidence and unfinished operations | Reinspect for the original goal |
| Protection blocks removal or changes | Exact object/reason plus completed and unprocessed work | Review the object without broadening or forcing execution |
| Runtime installed but discovery failed | Separate statuses and affected capabilities | Repair the particular discovery/resource issue instead of reinstalling repeatedly |

Work stops when evidence answers the question, without further scanning or fixed reminders. A deciding gap comes with its effect on the choice and the concrete observation needed. Failures, missing facts and timeouts remain explicit.

## Update and remove

Use the [installation guide](installation.en.md) to update runtime and plugin. `wombat system update` updates only the runtime; program, plugin resources and project discovery require separate confirmation. Users can ask whether Wombat works, then follow the concrete result.

Codex manages the plugin. When removal is intended, use its plugin-management entry or this command.

```sh
codex plugin remove wombat@wombat-local
```

Removal retains product data. Wombat registers no automatic Hooks and installs no separate Skill copy. Ordinary history and usage need no Hooks; configure special observations only when requested.

## Results and boundaries

Wombat local queries make no model calls; analysis, edits and validation in the current Codex conversation use the host model. Original work records stay read-only. Fact reads, user decisions, authorized mutations and verification remain distinct.

The Skill organizes evidence and actions without promising records for every question, measurements for every metric or complete acceptance from static checks. See the [complete CLI guide](cli.en.md) for precise scopes and automation. Ordinary use does not require learning them first.
