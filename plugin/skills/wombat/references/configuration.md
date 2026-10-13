# Current configuration

Default to this Codex workspace only when the request means “this project”; otherwise keep the selected project's exact cwd. Use the discovered Agent API and read each needed method Schema once. `optimize` lists suggestions with a scalar `project`; `config` reads inventory with `scope.project`. Request at most three initial items. For several configuration kinds, use one `config` request with `kinds`, rather than a new native capture and configuration scan for each kind. Follow selected items or pages at its returned `readView`.

After replacing the two root placeholders with the authorized paths, this request reads the four configuration kinds in one view. Its page is not the complete inventory:

```json
{"method":"config","params":{"action":"list","roots":["SOURCE_ROOT"],"projectRoots":["PROJECT_ROOT"],"scope":{"project":"PROJECT_ROOT"},"kinds":["rule","skill","mcp","hook"],"limit":3}}
```

Keep version domains separate: usage `snapshotId` fixes usage; configuration `readView` fixes inventory; optimization also binds `decisionRevision`. A first inventory query may use `snapshotId` for historical usage evidence, but must not combine it with `readView`. Later inventory calls reuse only `readView`. Suggestions do not take `snapshotId`; fixed-turn `optimize activity` instead takes the nested `activity` selection. There is no inventory `activity` action. On invalid arguments, consult that method's Schema and correct the request once instead of trying variants.

For item detail/evidence/related scopes use `config` with the returned item ID and `readView`. For suggestion detail use `optimize` with the returned suggestion ID, `readView` and `decisionRevision`. Keep project roots and scope on every call. Without the Agent API, use the documented `optimize inventory` and `optimize detail` CLI entries after checking their help once. Current configuration does not establish historical contents or loading. Suggestions do not accept usage date/model filters such as `--all-time`; inventory date filters narrow historical evidence, not current-file checks.

Separate declared configuration, native loaded observations, runtime attempts and actual successful use. Static measurement is not actual injected Token consumption. Unknown MCP text, historical versions or incomplete history cannot prove absence or inactivity. Inspect findings, exact target paths, shared project impact and coverage before suggesting a change.

Skill naming or frontmatter findings are format reminders unless native discovery proves a loading problem. Preserve native enabled/parse observations separately; Unicode punctuation or nested metadata alone does not prove the host failed to load. Large-body text estimates support a concrete split into task-specific references; they do not measure actual injected tokens or prove savings. Give the top three applicable findings before reading their selected details at the returned read view.

Inventory `extensionActivity` summarizes scoped Skill/MCP observations and returns the current page's `noObservedUseDays`. Use it to find items worth reviewing and drill into their evidence. These days run from the latest recorded use or the selected window start; null means no known start. They do not prove continuous observation, enablement or inactivity. `absenceObservable=false` prohibits an inactivity or deletion conclusion while still allowing a useful summary of recorded use. Exact native content hashes can identify a matching read result; generic historical content and MCP configuration-version adoption remain unavailable.
