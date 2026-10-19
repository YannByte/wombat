# Current configuration

Default to this Codex workspace only when the request means “this project”. Otherwise keep the selected project's exact cwd. Use the discovered Agent API. Read each needed method Schema once.

`optimize` lists suggestions with a scalar `project`. `config` reads inventory with `scope.project`. Request at most three initial items. For several configuration kinds, use one `config` request with `kinds`.

Do not run native capture and configuration scans again for each kind. Read selected items or pages at the returned `readView`.

Replace the two root placeholders with authorized paths. This request reads four configuration kinds in one view. Its page is not the complete inventory:

```json
{"method":"config","params":{"action":"list","roots":["SOURCE_ROOT"],"projectRoots":["PROJECT_ROOT"],"scope":{"project":"PROJECT_ROOT"},"kinds":["rule","skill","mcp","hook"],"limit":3}}
```

Keep version domains separate: usage `snapshotId` fixes usage. Configuration `readView` fixes inventory. Optimization also binds `decisionRevision`. Start current-file checks without a usage snapshot. If historical usage evidence is needed, a first inventory query can use a `live:` usage `snapshotId`, but cannot combine it with `readView`. Saved disk snapshot IDs are not configuration views.

For later inventory calls, keep the original `roots`, `projectRoots` and `scope`. Select `readView` instead of usage `snapshotId`. Suggestions do not take `snapshotId`. Fixed-turn `optimize activity` instead takes the nested `activity` selection. There is no inventory `activity` action.

On invalid arguments, read that method's Schema. Correct the request once. Do not try more variants.

For item detail/evidence/related scopes use `config` with the returned item ID and `readView`. For suggestion detail use `optimize` with the returned suggestion ID, `readView` and `decisionRevision`. Keep the original source and project selection on every call. Without the Agent API, use the documented `optimize inventory` and `optimize detail` CLI entries after reading their help once.

Current configuration does not establish historical contents or loading. Suggestions do not accept usage date/model filters such as `--all-time`. Inventory date filters narrow historical evidence, not current-file checks.

Separate declared configuration, native loaded observations, runtime attempts and actual successful use. Static measurement is not actual injected Token consumption. Unknown MCP text, historical versions or incomplete history cannot prove absence or inactivity. Examine findings, exact target paths, shared project impact and coverage before suggesting a change.

Skill naming or frontmatter findings are format reminders unless native discovery proves a loading problem. Keep native enabled/parse observations separate. Unicode punctuation or nested metadata alone does not prove that the host failed to load. Large-body text estimates support a concrete split into task-specific references.

These text estimates do not measure actual injected tokens or prove savings. Give the top three applicable findings. Then read the selected details at the returned read view.

Inventory `extensionActivity` summarizes scoped Skill/MCP observations and returns the current page's `noObservedUseDays`. Use this summary to find items for review. Examine the selected evidence. These days run from the latest recorded use or the selected window start.

Null means no known start. These day counts do not prove continuous observation, enablement or inactivity. `absenceObservable=false` prohibits an inactivity or deletion conclusion while still allowing a useful summary of recorded use. Exact native content hashes can identify a matching read result.

Generic historical content and MCP configuration-version adoption remain unavailable.
