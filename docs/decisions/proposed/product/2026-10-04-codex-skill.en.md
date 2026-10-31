# Decision: Codex Skill entry and distribution

[中文](2026-10-04-codex-skill.md) | English

Status: proposed

The [CLI and task Skill decision](../../implemented/architecture/2026-10-31-cli-task-skill.en.md) supersedes this proposal’s Web, multiple-entry and mandatory setup flows. Retain the collection and evidence requirements below only where they have a current CLI consumer.

## Problem

The old technical draft placed removed Wombat generation, application, and recovery flows in a user Skill and copied runtime assets separately. It did not distinguish Web browsing from processing in a Codex conversation or cover current account and native handoff capabilities. The user requested deletion and redesign based on main.

## Proposal

The previous local implementation includes both entries, shared root Skill source, plugin assets, and managed standalone copies. Codex 0.160.0 discovery and persistent Skill queue acceptance, synthetic cross-module tests, and Chinese/English browser flows have been verified. This does not complete the agreed real-task and platform acceptance. Current operations belong in the [CLI guide](../../../guides/cli.en.md) and [user Skill](../../../development/plugin.en.md); this proposal owns rationale and outstanding acceptance. Current delivery focuses on log and Hooks collection, Skill conversations, and Web display and explanation. Extensions and full acceptance for Web-selected Codex handoff are deferred and do not block delivery. Preserve existing implementation and technical evidence while evaluating whether to remove support later.

### Product positioning

The main flow is background collection, Skill conversations, and Web display. Users start through `$wombat` for queries, explanation, authorized processing, and rechecks, or open Web directly to inspect collection status, trends, tasks, and evidence. Both viewing paths share core data; processing continues in the current Codex conversation.

The Skill starts with the user's goal. Web supports filters, details, and chart-based explanation; the Skill can open a matching view. Page selection is not a task-dispatch entry in this delivery and does not change conversational targets or authorization. Web-selected handoff adds context synchronization and task switching, so simplify the current flow and evaluate its value later.

The five tasks are usage, task location, account allowance, configuration checks, and processing with rechecks. Answer simple questions directly; for complex questions, lead with the conclusion, then evidence and next steps. The first interaction does not present a feature menu or run a comprehensive check by default.

Wombat supplies data, evidence, user decisions, and check results; the current Codex handles understanding, file changes, review, and recovery. The Skill coordinates those responsibilities. The first version adds no MCP service, model API, or separate execution job.

### Current delivery order

Deliver question → findings and advice → processing in the current conversation → observed changes. First improve Skill routing to existing usage investigations, input trajectories, resource hotspots, period reviews, and configuration checks. Then organize a few findings and concrete next steps into the answer. Finally choose configuration rechecks or usage comparisons for the question. Available partial facts can support analysis and advice. Codex separates observations, possible causes, and recommendations, with relevant limits beside each conclusion. Complete continuous observation is not a prerequisite for every answer.

Reuse Rust facts, generated contracts, and existing evidence references. Usage candidates and configuration recommendations can appear in one answer while retaining their identities and verification methods. A shared presentation does not require new problem storage, resource entities, a general rule framework, or an execution service. Web reuses existing details and header Skill guidance. Remove dispatch buttons from the primary overview, recommendation list, and detail interfaces while preserving existing interfaces and records.

The first acceptance flow starts with a usage question, locates a task, checks its project, handles an authorized object, and rechecks it. Verify that follow-up questions preserve targets and scope, that partial data still produces supported answers, and measure actual query counts, response sizes, and time to the first useful result. The [optimization proposal](2026-10-03-optimization-lifecycle.en.md) continues to own continuous coverage, inactivity checks, resource cleanup, and historical-content adoption. Accept them separately later. They do not block this common flow, and passing this batch does not complete the full scope.

### Log and Hooks collection

Collection is part of this delivery and shares the Rust core with Skill and Web. Existing log adapters provide historical collection, usage, and safe operation facts. New Hooks receive runtime events that the host actually supplies, improving freshness and adding observations missing from logs. Current native Hook registration observations establish registration, enablement, and trust only; they do not receive runtime events. See the [registration decision](../../implemented/architecture/2026-10-04-hook-registry-observation.en.md). Skill and Web add no separate parser, database, or accounting rules.

Prefer verified plugin capabilities for Hooks. Standalone wiring requires an explicit user choice and preserves existing configuration. One setup flow checks the runtime, collection wiring, host trust, historical preparation, and actual receipt. Skill calls deterministic interfaces and explains results; event receipt does not require an active model conversation. The receiver has timeouts, input budgets, and private storage. When the service is paused, bounded safe buffering and replay retain observations; overflow or missing events remain explicit coverage gaps. Collection failure must not alter prompts, tool inputs, or permission decisions, or block user tasks.

Associate logs and Hooks by source, session, and native event identity. Without reliable identity, retain unknown association instead of merging by text or nearby timestamps. Replay, historical collection, reordering, and duplicates must not add to the canonical Token ledger. Tool events receive no cost allocation. Retain only necessary safe observations, excluding full conversations, tool output, credentials, and arbitrary raw parameters, under the existing [privacy boundary](../../../reference/privacy.en.md). Report registration, actual receipt, historical coverage, and freshness separately. Receiving a tool event does not establish that a Skill or rule loaded.

Receipt, paused buffering, status queries and exact log association are implemented locally. Isolated native installation, discovery, 10 untrusted registrations and removal have been verified on macOS. Native review in Codex 0.160.1 has trusted all 10 collection declarations in the current user profile while preserving other plugin declarations. A read-only task received SessionStart, UserPromptSubmit, PreToolUse, PostToolUse and Stop, and passed exact-session, supplied-turn-identity, source-epoch and pinned Token-ledger rechecks. Unexecuted lifecycle events, full real-project scenarios and other platforms remain unverified. Acceptance covers target Codex event support and trust, duplicates and reordering, concurrency and restart, paused-service replay, buffer overflow, historical collection, removal that preserves other hooks and data, and Chinese/English Skill/CLI/Web queries of the same observations. One platform or synthetic input does not establish support across hosts.

### Skill processing and Web viewing

The processing flow is explicit user goal → query data and evidence within the same scope → current Codex processes authorized changes → Wombat rechecks. Complex questions can open a matching Web view while explanation and follow-ups continue in the original conversation.

For example: “Why was usage high today?” The Skill identifies main tasks and models, opens usage charts for the same dates and project, locates the largest task and its turns, and explains related operations and attribution limits. A follow-up asking to check that task's project reuses the located project instead of restarting global selection.

Web independently displays collection status, usage, tasks, and configuration evidence. Deferral applies to extensions that dispatch selected objects to Codex for processing. It preserves page filters, detail selection, and contextual Web views opened by the Skill.

### Detailed explanation with Web from the Skill

Web is an explanation view the Skill can use. Answer simple numbers and individual issues in the conversation; trends, comparisons, long tasks across turns, instruction hierarchies, and complex suggestion evidence benefit from Web. Open the relevant view when the user requests details; complex questions may receive both a conversational conclusion and a local page entry without requiring repeated navigation.

Preserve project, sources, dates/timezone, filters, selected task/turn or configuration/suggestion, and versions. Explain what to inspect and which conclusion it supports. Closing the page or failing to open a browser does not prevent a useful conversational answer. Browser selection does not automatically update conversation context; verify any new explicit selection before processing.

This branch implements `web --context FILE --json` and host openView to validate source, project, and version through generated contracts and return the effective context. Chinese and English synthetic tasks passed actual Codex continuous conversations, same-version Web checks and authorized-edit rechecks. Full real-project and other-platform acceptance remain incomplete. URL composition alone cannot establish same-version navigation between entries. Exclusive CLI end dates and inclusive displayed Web end dates use product conversion logic.

The implemented restricted open-view operation accepts page type, sources/project, query conditions, selected IDs, and read versions. The host validates authorization and versions, initializes the view through the shared client, and returns a local URL and effective context. Generated contracts define fields; frontend-internal Route is not a public interface. Arbitrary paths, shell execution, and external URLs are excluded.

Expired versions reread the original scope with changes explicitly marked. If a fixed version cannot be confirmed within the host's authorized scope, explain why instead of silently displaying current results. Use the current local service session link without exposing connection tokens or reusing expired links. The initiating session manages service startup and shutdown; existing services can be reused only with matching source and project scopes.

### Web setup and Skill collaboration design

The user has authorized development. Setup panels, state interfaces and the local collection package are implemented locally. The following remains the full experience and acceptance scope; partial implementation does not establish acceptance. Preserve existing changes and user data.

#### Reference and choices

Use COT's collection-mode → connection → historical preparation → viewing flow as a reference. Its current source offers Hooks and transcript-only paths and can reuse discovered wiring after installation; see the [onboarding flow](https://github.com/cot-intelligence/cot/blob/6b51be8943371d20c45d71bff9cf13ebd3461605/src/components/onboarding/Onboarding.tsx). Wombat adopts one setup flow and recovery for the failed step while allowing immediate access to available data. Skill availability and live collection remain independent; installation does not establish data readiness.

COT maps no_events and stale to installed before displaying Connected; see its [state mapping](https://github.com/cot-intelligence/cot/blob/6b51be8943371d20c45d71bff9cf13ebd3461605/src/components/onboarding/steps/ConnectHooks.tsx). Wombat reports declarations, trust, actual receipt, and historical coverage separately. Events, historical content, or resource loading absent from the source remain unknown. Full conversation bodies are not stored.

#### Page structure and visuals

Retain Overview, Tasks, Instructions, Extensions, and Suggestions. Add header entries for “Setup and collection” and “Use with Skill”. The first shows the local service, collection wiring, and historical preparation; the second explains installation, invocation, and follow-up questions. Overview shows one dismissible notice when preparation or recovery is needed and displays analysis directly when data is available. Dismissal changes presentation only, not connection or data state.

Use the existing tool interface: background #ffffff, text #0d0d0d, secondary text #6b6b6b, dividers #e6e6e6, information #507e95, and reminders #875925. Retain system Chinese and English fonts with 27px page titles, 17px section titles, and 14px body text. Desktop setup places steps on the left and a vertical status list with one primary action on the right. Narrow screens use one column with wrapping paths and status text. Dark mode reuses semantic variables rather than fixed light backgrounds. Preserve keyboard focus, readable state text, and reduced-motion preferences.

#### First-time setup

The local Wombat runtime must be installed and able to start before Web opens. Installation instructions lead into the same setup flow; a page that is not running cannot install its own service. First entry identifies missing steps from actual state. Available history does not require restarting setup, and browser first-run flags do not determine readiness.

| Step | What the user sees | Action and completion basis |
|---|---|---|
| Choose collection | “Live collection with historical catch-up” or “Read-only log collection”, with observation scope | Recommend the first on supported hosts and disclose unsupported Hooks. Read-only mode preserves native Agent configuration and still supports Skill use |
| Complete connection | Separate local-service, Skill discovery/enablement, and Hooks declaration/trust states | Show reviewed wiring, concrete changes, and versions; reuse valid wiring. Recheck after native trust review in Codex; a missing Skill does not block data viewing |
| Prepare history | Sources, authorized projects, available results, and failed items | Reuse incremental indexes and source synchronization without a separate import database. Use explicit scope and publish available results first; distinguish no history from read failures |
| Start using | “View data”, confirmed Skill invocation, and natural-language examples | Viewing requires no extra model task; Skill use and event receipt are independently verified, and missing runtime events display “Waiting for first event” |

Read-only log collection reads history and later appends; actual synchronization state establishes freshness.

One proposed deterministic local entry coordinates checks, installation preparation, and recovery through existing installation and collection modules. Freeze its command name and protocol fields after capability verification. Web provides steps and copy actions without executing arbitrary commands. Codex owns plugin management and the native host owns Hooks trust. Copying a command or opening a page does not establish connection success. Distribution restrictions for the public base plugin and local collection package follow the installation proposal.

#### Daily state and recovery

| Observation | States that must remain separate | Page actions |
|---|---|---|
| Skill | Unchecked, missing, disabled, available, multiple instances, check failed | Recheck for the current project and show installation instructions; use the native invocation name rather than assuming $wombat |
| Live collection | Unwired, trust needed, registered/waiting for events, events received, paused, failed or unknown check | Show check time and actual last receipt; repair or resume for the specific cause, without inferring disconnection from temporary silence |
| Historical data | Preparing, available, partly available, confirmed absent logs, read failed | View existing results, inspect failed sources, and retry the relevant scope; a waiting limit does not turn preparation into no history |
| Current view | Fixed or current, updating, newer data available, old results, expired version | Show source scope and actual data cutoff; explicitly apply new data while retaining the question and object location |

Shared interfaces provide configuration, receipt, coverage, and freshness rather than a generic frontend “Connected” label. Future continuous collection owns its lifecycle: closing Web does not establish stopped collection. Paused collection preserves historical viewing and exposes replay and gaps separately. Status refreshes are bounded. Leaving a page stops its requests; collection does not require an active model conversation.

#### Skill and Web round trips

When starting from Skill, Codex first answers the question and opens a matching Web view for charts or long-task evidence. Show project, sources, dates/timezone, located objects, and data time; put technical versions in expandable evidence. Users browse while processing and follow-ups continue in the original Codex conversation. Web filter changes affect browsing without changing conversational authorization.

When starting from Web, users can view data directly or open “Use with Skill” for invocation guidance. Offer generic questions for the page topic, such as explaining today's main usage contributors or checking the current project's configuration suggestions and their evidence. Show the exact invocation name only after confirming an available instance. Examples may be copied; they do not bind selected page objects into processing tasks, send to Codex, or create conversations.

The [UI README](../../../development/architecture.en.md) owns current recommendation actions and presentation. Processing continues in the current conversation; Web retains viewing and rechecks. Preserve legacy dispatch interfaces and records while evaluating removal separately. Details still show issues, locations, evidence, methods, user decisions, and recheck results without assuming successful changes. Instructions and Extensions retain filters/details; Tasks retains turns, events, and same-version drill-down.

#### Languages, failure, and design acceptance

Provide setup steps, states, recovery, Skill examples, and explanations in Chinese and English. Language changes retain collection mode, step, project, sources, and read versions. Preserve source text, model names, stable IDs, paths, and protocol values. Examples use actual instances and distinguish plugin invocation from standalone Skill names.

Acceptance covers first entry without data, direct viewing of existing history, read-only mode, Hooks awaiting trust, registered Hooks without events, the first real event, service pause/resume, partial source failure, missing or multiple Skills, browser failure, and view expiry. Complete Skill question → Web viewing → original-conversation continuation → recheck in both languages, checking that browser filters do not widen authorization. Verify narrow layouts, dark mode, keyboard interaction, and status changes. Copying examples triggers no scan, installation, dispatch, or model request. State interfaces and safe Hooks receipt now have a local implementation. The common native events above have passed actual execution acceptance. Other lifecycle events and full real-project conversational acceptance remain unfinished; a design or synthetic test does not establish complete delivery.

### Chinese, English, and language across entries

Web already supports Chinese and English through shared messages and a local language preference. Existing Web handoff requests carry the page language to select the handoff prompt template; this does not establish acceptance of subsequent Codex answers in that language. Follow [product language](../../../i18n/product.en.md) for current precedence and translation boundaries, reusing these capabilities in the Skill.

Skill explanations follow an explicit user language request first, otherwise the current conversation. For an existing Web handoff without a new explicit request, retain the page language at handoff as the answer preference; extensions to that path are deferred and full acceptance is outside this delivery. Chinese and English share one Skill and the same query and processing flows, without separately maintained business instructions. Display language does not determine time zone, sources, or project.

When the Skill opens Web, retain existing Web language preferences by default. For an explicit request for a Chinese or English page, use the supported web --lang zh|en; the proposed restricted context-opening interface should support the same language choice. Reusing an open page preserves the user's current selection rather than forcing a language switch on conversation continuation or object location. Switching page language does not reinitialize data or change filters, selections, evidence versions, or authorization scope.

New pages, preparation notices, recovery messages, and handoff copy provide both languages through @wombat/client/locale. Codex generates Skill explanations according to the answer preference. Preserve JSON fields, status codes, stable IDs, model names, paths, and source text; do not implicitly translate quoted user content. Accept both languages separately for first-time preparation, detailed explanation, continued questions after language switching, and processing/rechecks in the current Codex conversation, checking that data and objects remain identical.

### Skill entry and data initialization

Initialization prepares the requested task. The Skill invokes the current CLI and reuses Rust source discovery, index restoration, and incremental synchronization, without another database, first-run flag, or log parser. See [initialization and continuous discovery](../../rejected/product/2026-10-02-progressive-initialization.en.md) for the product flow.

Determine intent and required data before checking the executable and relevant capabilities. With only a Skill name and no task, explain executable availability and offer one natural-language example without scanning complete history. Reuse a successful executable check within the conversation and recheck when its path or capabilities change.

| Request | Initialization action | Preparation it must not require |
|---|---|---|
| Usage, tasks, related history | Run one small-page live query for the original question; discover sources, restore the index, and synchronize automatically | No mandatory refresh/snapshot export or whole-disk project search |
| Current configuration and static suggestions | Read inventory/suggestions for the current or explicit project; explain static facts while history is incomplete | Complete history is not a prerequisite for static checks; unknown association is not zero use |
| Account allowance | Independent account read, or refresh when requested | No log synchronization, project discovery, or index initialization |
| Existing Web handoff processing (extensions deferred) | Verify incoming targets, scope, and content version, then query and recheck as needed | No global reselection or scope expansion after initialization failure |
| Offline or fixed snapshot | Use existing cache or the selected snapshot, retaining its version | Missing cache cannot silently initiate scanning or pricing downloads |

Actual query states determine subsequent behavior:

| Observed state | Conversation behavior |
|---|---|
| Complete current or fixed | Answer the original question and retain the version for drill-down |
| Initial preview, initialScan=true, or incomplete coverage | Locate available tasks while explaining incomplete totals; avoid global ranking conclusions and no-use judgments |
| SYNC_PENDING without usable results | Explain that initial history is being prepared; allow at most one further fresh wait for the same question, then stop waiting and retain the question |
| Committed results with syncing/stale | Answer established facts with data time and update status; wait within bounds when current results are needed |
| failed or partial | Separate usable sources, old results, and failures; preserve facts and provide error-specific recovery |
| Completed synchronization with no data | Distinguish missing supported logs from no filter matches before adjusting dates or requesting explicit sources |
| Expired view or unknown format | Reread the original scope after expiry and explain changes; preserve unknown-format data and report it without clearing or migrating |

Ordinary queries currently wait about two seconds at most; fresh waits up to about ten seconds and can return SYNC_TIMEOUT. A timeout does not establish that scanning stopped or failed. Without a committed index, --cached returns NO_SNAPSHOT; an empty cache cannot complete the task. Chat does not follow synchronization indefinitely by default; explicit continued waiting or monitoring requires a separately bounded policy.

After initial waiting, offer the local Web entry for existing synchronization state and temporary tasks. A later “continue” retains the question, sources, dates, and selection and queries again. Opening Web does not rebuild the index; stopping page waits or canceling one CLI request does not establish cancellation of the shared service. Project-level progress events are not implemented, so do not invent percentages, remaining time, or project completion counts.

Interpret product JSON only. If trials establish that combined states are difficult to explain reliably, add a narrow readiness projection in the shared interface for CLI/Web. Do not invent Skill-specific readiness rules based on inspecting databases, index files, or service processes.

### User tasks and answers

| User expression | Default action | User receives |
|---|---|---|
| “How much did I use today, and where?” | Query totals using the user's local date and timezone, then choose task, model, or project rankings as needed | Total tokens, official-standard API-equivalent cost, main contributors, unpriced usage, and coverage gaps |
| “Find yesterday's task and its most expensive turn” | Search or locate by full ID, then query turns and steps within the same read version | Exact task and turn, matched scope, related operations, and limitations when body evidence is unavailable |
| “How much Codex allowance remains, and when does it reset?” | Read the account independently; refresh when the user asks for current information | Account status and read time, actual windows and reset times, balances and restrictions; missing or stale values remain unknown |
| “Check this project's AGENTS and extensions” | Default to the current working directory; read inventory, suggestions, and selected evidence | Problems, locations, evidence, processing order, and items that cannot be determined |
| “Handle the first item, then check again” | Reuse the preceding object and scope; the current Codex edits and rechecks each rule | Actual file changes, recheck results, and remaining issues; later use observations are explained separately |
| “Keep this item and stop prompting” | Identify the suggestion and record an interface-supported reason | An explicit user decision; check facts remain independently recorded |

Default answers use natural paragraphs for conclusion, evidence, and next steps. Use tables for comparisons and mention pagination only when more tasks exist than shown. All totals use the kernel's complete-scope summary, never a sum of the first ranking entries.

“First item,” “continue,” and “that project” reuse full IDs, scope, and versions from this conversation; do not rerank and guess. Adopt inferable dates, timezone, and current project with a brief explanation. Ask only when ambiguity changes the conclusion or write targets; allowance queries need no project question.

### Division with Web

| Scenario | Web | Skill |
|---|---|---|
| Understand usage | Trends, filters, composition, lists, and details | Choose relevant views, explain main contributors, and drill down when needed |
| Check configuration | Browse instruction trees, extensions, suggestions, and history | Connect evidence and suggestions around the current project or selected object |
| Start processing | Display Skill guidance and rechecks; retain dispatch interfaces and records for evaluation | Handle authorized objects in the current conversation; hand off only when a new task is explicitly requested |
| Determine resolution | Display per-rule rechecks and subsequent observations | Run the same recheck after changes and explain passed, remaining, and unavailable checks |
| Read allowance | Persistent summary and detail window | Explain account state, windows, and restrictions for the question |
| Continue browsing | Preserve filters, expansion, language, and page state | Preserve objects and evidence context needed by this conversation |

Tasks sent by Web continue in the receiving Codex task. The Skill does not send another request to itself or add a generation step. The sender includes targets, evidence versions, answer language, and the native Skill name; the queue also carries an explicit Skill reference. Acceptance confirms reception of the bound request, not model use or completed processing.

### Reusing current capabilities

Use the [CLI guide](../../../guides/cli.en.md) for current business entries and limits. The proposal coordinates existing facts and rules rather than introducing another accounting DTO, execution service, or log parser. Contextual Web opening shares those contracts; extensions to existing handoff are deferred.

### Shared processing and recheck flows

Direct check: establish the current project and sources → read inventory and suggestions → inspect locations and evidence as needed → recommend processing order. A check-only request is complete at the conclusion.

Process in the current conversation: reuse selected suggestions → verify applicable project instructions, current content, and shared-file scope → the current Codex performs authorized changes and relevant validation → Wombat rechecks each rule → report changes, rechecks, and remaining issues. An explicit processing request needs no repeated approval for the same scope; expanding targets or changing behavior requires a concrete reviewable choice first.

Existing Web handoff (extensions deferred, safety constraints retained): read incoming project cwd, targets, content fingerprints, and evidence versions → verify current targets → process in the receiving task → recheck. Expired versions or changed files require a reread of the original scope with the change explained. Explain unmodified items as well; request acceptance cannot establish completion.

Use existing preview/send when the user explicitly requests another task. Present projects, files, shared impact, and actual allowance checks, then bind `selectionVersion`. Omitting suggestion includes every pending suggestion in the selected scope, not just the displayed page, so batch actions must preserve explicit selection. Unknown delivery requires inspecting existing tasks first, never automatic resend; closing Wombat does not cancel Codex.

The current Codex undoes changes using actual diffs and current contents while preserving later user changes. Wombat history and decisions support explanation and rechecks, not file backups, rollback, or execution receipts.

### Evidence and states

The usage chain retains sources, dates, timezone, filters, snapshot ID, and full task/turn IDs; the configuration chain retains project, readView, decisionRevision, rule parameters, and suggestion IDs. Short-lived versions returned by ordinary queries can support subsequent drill-down; fixed snapshot queries omit source roots and refresh options. Reread the original scope and explain changes when versions expire.

Current files cannot explain tasks with unknown historical configuration versions. Describe native AGENTS load evidence, Skill availability, approximate observed use, directed file reads, MCP outcomes, and Hook registration separately. Lack of an observation does not establish idleness; availability does not establish use, and registration does not establish execution. Related-turn tokens are not an extension's exclusive cost.

Exit code 2 with valid results supports a qualified conclusion that discloses partial source failures or synchronization state. An initial `SYNC_PENDING` may receive one bounded wait, then retain status and offer a retry path instead of indefinite polling. Distinguish missing, zero, unpriced, unknown, and hidden values. Offline requests use existing cache/fixed snapshots or disable automatic pricing; ordinary live queries may download missing official prices.

Account identity, allowance, and activity have independent status and read times. Preserve actual windows, buckets, full-precision balances, and limits rather than fixed five-hour/seven-day assumptions. Window model is display information; existing handoff checks determine applicability. Distinguish low-balance warnings from explicit blocking and add no blocks for unknown states. Passing a reset time does not establish restored allowance; read-only entitlements offer no reset action.

A passed recheck establishes only that currently checkable rules no longer match. Approximate use in later natural tasks remains separate from compliance, quality improvement, and savings; do not create extra consuming tasks to validate adoption.

### Skill structure and installation proposal

After research, use a Skill to author workflows and a skills-only plugin for formal distribution. OpenAI supports this minimal plugin shape; MCP remains conditional on need. A local collection package adds verified Hooks resources. Official support currently limits Hooks plugins to manual installation in Codex desktop and excludes them from the public directory, so the two packages cannot claim one publication channel. Users install the local Wombat release, then install the lightweight workflow package through Codex's plugin interface. Use a local marketplace for initial trials; the public directory requires later submission and review. [Official plugin packaging](https://developers.openai.com/plugins/build/plugins)

The root [plugin/](../../../development/plugin.en.md) directory is the sole source for user plugin resources; the Wombat Skill lives in plugin/skills/wombat/. Repository .agents/skills/ retains development workflows. Standalone installation and plugin packaging reuse that source, generating the host-standard skills/wombat/ resource layout in plugins without another maintained Skill copy. The build generates standalone assets, a plugin, and a local marketplace from this source. Use root plugin.json for the current standard. If supported Codex versions require .codex-plugin/plugin.json, generate that compatibility manifest from the same metadata instead of maintaining two descriptions. [Official formats](https://developers.openai.com/plugins/build/plugins)

Use a short entry with references loaded as needed. The entry handles intent, executable discovery, and essential context; startup/initialization, usage/tasks, account, configuration evidence, processing/rechecks, and Web explanation have relevant references. Helpers are reserved for repeated mechanics needing deterministic execution; do not copy the entire CLI manual into instructions. [Official Skill organization](https://learn.chatgpt.com/docs/build-skills)

The base plugin includes only its manifest, Skill resources, necessary icons, and licenses. The local collection package adds reviewed Hooks declarations and minimal wiring scripts that reuse the same runtime without copying log parsers. Reuse the local Wombat CLI, Rust kernel, and bundled Node; plugin installation does not install the local product. Releases and plugins have explicit versions and required capabilities. Check protocols and features rather than requiring identical versions or equating installability with local-data access.

A distributable plugin embeds no machine-specific absolute path. Prefer an executable explicitly selected by the user, otherwise discover the installed stable launcher. Verify capabilities and retain that path within the conversation. Explain missing capabilities and installation/upgrade paths without automatic downloads, upgrades, credential reads, or data-scope changes. A remote host without this machine's logs cannot provide local capabilities.

Codex's plugin manager owns formal plugin installation, enablement, updates, and removal. Wombat neither edits its cache nor duplicates installation records. Wombat updates its product runtime without overwriting host-managed plugins. Different versions are accepted according to capabilities; missing ones produce explicit limitations.

Use narrow wombat skill install / status / uninstall operations for local offline and development trials. install registers an independent Skill only from reviewed assets in the installed release, without reproducing GitHub cloning, marketplace discovery, or cross-agent installers. Change the default user path to ~/.agents/skills/wombat and verify discovery for custom destinations. [Official discovery locations](https://learn.chatgpt.com/docs/build-skills)

Locations differ: installers in openai/skills and vercel-labs/skills still use ~/.codex/skills. This does not establish obsolescence or a universal default for new hosts. status or handoff should use cwd-scoped skills/list to confirm actual enabled instances, paths, duplicate names, and parsing errors. Support in the target native protocol requires separate verification.

Local installation records source, fixed version, content hashes, and ownership in .wombat-install.json within the managed copy, without editing third-party locks. Only managed copies are supported; development links are not provided. Linked directories and their targets are protected. Preserve custom same-name Skills; stage and verify before explicitly replacing a managed installation, switch atomically, and retain the previous installation on failure.

status reports file installation, host discovery/enablement, CLI capability match, and data readiness separately; these four states cannot collapse into installation success. uninstall removes only the unchanged owned Skill copy, preserving link targets, product data, and other managers' installations. If plugin and standalone copies coexist, explain duplicates, preserve configuration, and bind the specific enabled instance in handoff.

### Deferred Web-selected handoff

This section preserves existing technical evidence and safety constraints rather than setting current delivery requirements. Before removing support, separately evaluate entries, contracts, and existing records; narrowing this scope does not delete user data.

Official App Server guidance recommends a Skill invocation marker in text together with a skill input item carrying name/path, so the host injects instructions directly. First use skills/list to obtain the enabled instance for the target cwd. Use returned names and paths rather than guessing plugin cache locations. [Official interface](https://learn.chatgpt.com/docs/app-server)

Persistent handoff retains thread/queue/add. An isolated native probe on Codex 0.160.0 confirmed text plus skill references. The native plugin name is wombat:wombat; standalone instances use wombat. Discovery and sending use returned names/paths, retaining the persistent flow rather than replacing it with turn/start. The probe did not execute a model task and does not establish completed edits.

For missing, disabled, ambiguous, or unsupported Skills, disclose before sending that handoff is available but Skill adoption is unconfirmed, with an installation or instance-selection entry. If the user chooses existing Codex handoff, retain original targets and evidence and explain the limitation. Do not automatically install, enable, or send a second task. Claim Skill adoption only after explicit support is established.

### Official and open-source management practices

The following primary sources were read on 2026-10-04. They illustrate practices, not a universal standard; current code, host versions, and actual acceptance outweigh repository reputation.

| Source | Observed practice | Wombat choice |
|---|---|---|
| [openai/plugins](https://github.com/openai/plugins/blob/main/README.md) | Packages under plugins/ with manifests, optional skills/ and other capabilities; a marketplace points to standard plugin locations | Formal distribution uses a skills-only plugin, separate from repository development Skills |
| [vercel-labs/skills](https://github.com/vercel-labs/skills/blob/main/README.md), [lock implementation](https://github.com/vercel-labs/skills/blob/main/src/skill-lock.ts) | Installation scopes and agent targets, links/copies, source/ref/content hashes, update and removal | Adopt source identity and diagnosable updates without copying a cross-agent package manager |
| [anthropics/skills](https://github.com/anthropics/skills/blob/main/README.md), [skill-creator](https://github.com/anthropics/skills/blob/main/skills/skill-creator/SKILL.md) | Self-contained Skills, resources on demand, realistic requests, baseline comparison, and iterative evaluation | Short entry, task-driven initialization, behavioral and false-trigger testing; template length is not quality |
| [obra/superpowers](https://github.com/obra/superpowers/blob/main/README.md), [visual companion guide](https://github.com/obra/superpowers/blob/main/skills/brainstorming/visual-companion.md) | Host-specific plugin distribution, task workflows separate from optional browser assistance, explicit service lifecycle and session connection | Skill can explain through Web using product views and states without copying its whole development method or HTML server |

The [current openai/skills README](https://github.com/openai/skills/blob/main/README.md) marks that repository deprecated and points to openai/plugins. Its installer helps explain existing local behavior, but formal design follows current documentation and plugin examples. OpenAI recommends concise descriptions, progressive loading, and tests for direct, indirect, incorrect-trigger, and edge requests. Skill-assisted Web explanation and initialization policies are Wombat product judgments, not an official universal best practice. [Official evaluation requirements](https://developers.openai.com/plugins/build/skills)

### Implementation work packages

These packages divide implementation and acceptance responsibilities; task evidence owns locally delivered scope and unverified boundaries. Keep the independent worktree; fetch main before implementation, review baseline differences, and preserve this branch's design and user changes. Prefer one independently reviewable PR per package. A shared interface and its applicable entries ship in the same package.

| ID | Package and primary changes | Dependencies | Acceptance and delivery evidence |
|---|---|---|---|
| S1 | Verify the host and freeze the minimum contract: check target Codex plugin formats, actual Skill discovery locations, and cwd-scoped skills/list; verify CLI JSON states, executable discovery, and collection event capabilities | None | Version/capability records for synthetic targets; restricted view-opening fields and fallbacks without guessing cache paths; preserve existing queue probes without making new handoff acceptance a delivery prerequisite |
| S2 | Author plugin/skills/wombat/: short SKILL.md, host metadata, and references for startup/preparation, usage/tasks, account, configuration, processing/rechecks, and Web explanation | S1 CLI capability review | Direct calls, indirect triggers, incorrect triggers, and follow-up questions; Chinese/English answers; empty indexes, provisional/partial results, independent account reads, missing offline cache, and bounded waiting. Every claim maps to executable CLI behavior without restoring old execution flows |
| S3 | Package formal distribution: generate a skills-only plugin from plugin/skills/wombat/, derive target manifests from one metadata source, and add local marketplace trials, licenses, versions, and required capabilities | S1 format/discovery verification, S2 | Install, discover, enable, and remove in a clean Codex environment or isolated configuration; packaged resources match source and contain no machine-specific paths. Reuse installed Wombat without equating plugin installation with product installation or data readiness |
| S4 | Add standalone local trials: wombat skill install/status/uninstall consumes reviewed release resources; include those resources and record source/ref/hash/ownership | S2; reuse S3 version/capability definitions | First installation, custom same-name protection, explicit replacement, retention on failed updates, and data-preserving removal in temporary directories; report actual host discovery separately from file existence. Codex still manages formal plugins; entry instructions and messages support both languages |
| S5 | Web setup and Skill collaboration: add the designed setup flow, separate states, and invocation guidance; retain restricted context opening and generated contracts; CLI assembly, Web scope/version checks, and UI page/object location; optional explicit language with Web preferences preserved by default | S1 minimum contract, S2 | Usage to task/turn and configuration to recommendation round trips; date boundary conversion, expired/unrecoverable versions, browser failure, service lifecycle, and cross-project rejection. Language switching preserves objects, filters, and data; arbitrary URL construction does not establish success |
| S6 | Defer extensions to Web-selected handoff and evaluate removal later; preserve existing preview/send, Skill references, and technical evidence | Not a current prerequisite | Deferral establishes neither removal nor full acceptance; future removal must identify interface, user-flow, and historical-record impacts |
| S8 | Collect logs and Hooks: verify host events and trust, add bounded receipt, safe buffering, and Rust association, unify setup/status/repair, and build the local collection package from one source | S1, existing log core, S3 packaging | Actual receipt, replay, and historical collection without duplicate accounting; failures do not block tasks; privacy, identity gaps, overflow, removal protection, and bilingual CLI/Web queries; untrusted or event-free wiring cannot report collection success |
| S7 | Accept and deliver the current flow: synthetic end-to-end scenarios, installation acceptance, natural-language trials, and bilingual instructions and decision status where passed | S2–S5, S8 | Complete five tasks in both languages, checking collection, preparation, Web explanation, processing in the current conversation, and rechecks; prepare a release candidate after acceptance and retain platform limits; Web task dispatch is not required |

S2 Web references describe available capabilities and restrictions first, then change with S5 so the Skill never promises future interfaces. S6 is deferred and adds no current completion gate for Web-selected handoff. Existing paths preserve original targets, evidence, and failure constraints without claiming full behavioral acceptance.

Close three milestones: A covers S1–S3 for an installable Skill plugin that queries and explains; B covers S5 for detailed explanation through Web; C covers S4, S8, and S7 for standalone offline trials, log and Hooks collection, and current-flow acceptance. S4 does not block plugin trials but remains necessary for complete installation acceptance. S6 blocks no milestone.

Keep current ownership: Skill content in plugin/skills/; argument and presentation assembly in cli/; native Codex discovery/queue communication and process lifecycle in client/src/node/codex/; Web authorization and connections in web/; page selection and translation in ui/ and client/src/locale/. Maintain public DTOs/states through the existing generation pipeline, with core business validation in core/. S1 evidence constrains protocol commands, input JSON, and unsupported cases rather than UI guesses.

Preparation and language are acceptance criteria in every relevant package. Reuse existing states; add a shared readiness projection only if trials demonstrate unreliable interpretation, shipping it in noninteractive CLI/Web together. Installation and sending retain distinct failure/cancellation semantics. Browser language changes no scan scope; unknown allowance and provisional history do not justify broader permissions or automatic installation.

Choose validation per package: S2 metadata, references, and natural-language behavior; S3/S4 isolated installation, update ownership, and packaged resources; S5/S8 build first when shared interfaces change, then validate types/contracts, targeted cross-module tests, and actual browser/native Codex behavior. S7 runs full tests and applicable release checks on the final build; add Rust formatting, clippy, and license checks when Rust or dependencies change. Each PR records tested scope, evidence locations, and remaining boundaries. Static checks do not replace host or human acceptance.

### Delivery order and acceptance

| Phase | Delivery | Completion evidence |
|---|---|---|
| Current design | Remove the old draft and dead entries; complete capability mapping, flows, and installation proposal | Bilingual, link, and repository checks; not behavioral acceptance |
| First implementation | Short Skill, task-driven initialization, a skills-only plugin, and local trial installation | Actual host discovery, directory protection, capability matching, clear update ownership, and data-preserving uninstall |
| Collection and Web explanation | Historical logs and Hooks receipt share the core; Skill opens contextual Web views | Event association avoids duplicate accounting; recovery and actual receipt status; views retain scope/version and processing/rechecks continue in the current conversation |
| Human trial | Complete all five tasks and follow-up questions in both Chinese and English | Actual outcomes, error recovery, call counts, and output sizes; synthetic or reviewed material only |

Human acceptance must cover empty indexes and previews, old-result updates, allowance independent of log waits, incomplete configuration history, offline missing cache, continuation after initial waiting, missing or incompatible executables, duplicate/disabled Skills, and Skill opening a selected Web view and continuing the original conversation, conversational answers after browser failure, version-mismatch notices, local day boundaries, same-version ranking drill-down, expired-view recovery, multiple allowance windows and read failures, project roots distinct from log roots, shared configuration, keep/not-applicable decisions, changes that still match or cannot be checked, and protection of custom same-name Skills.

If trials reveal significant context or latency overhead from repeated queries, design a shared bounded summary interface for CLI/Web; do not add statistics or suggestion rules inside the Skill. Record check coverage separately from actual outcomes, without claiming validated savings, platform support, or automatic loading.

## Alternatives considered

- Copy Web navigation: conversations must establish goals first; traversing every page adds irrelevant queries, so use task organization.
- Extend the old draft and copy runtime assets: responsibilities have changed and duplicate assets increase version maintenance, so use the installed product.
- A custom installer for formal distribution: workable for local trials, but current official guidance prefers plugins for reusable Skills, so the host owns formal installation lifecycle without another cross-agent package manager.
- Add MCP immediately: CLI already covers business queries and plugins can contain only Skills; revisit MCP for multi-host needs or measured overhead.

## Acceptance criteria

Current delivery follows S1–S5, S8, and S7 responsibilities and acceptance conditions. S6 is deferred and is not a completion gate. Real conversational trials must cover usage/task follow-ups, independent accounts, configuration evidence, authorized processing/rechecks, and contextual Web explanation in both languages. Distinguish trigger behavior, partial/empty/offline results, bounded waiting, modified installations, native acceptance, actual edits, and rechecks. Local macOS evidence does not establish other platforms or a public marketplace release. Partial delivery remains proposed.
