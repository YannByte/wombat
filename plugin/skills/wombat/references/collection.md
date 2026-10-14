# Setup and collection

Use `wombat setup --project PROJECT --json` for read-only checks of the runtime, actual Skill discovery and scoped Hook registration. Use `wombat collection status --project PROJECT --json` to examine actual receipt. These checks do not install, enable, trust or invoke a model. Use the startup reference for executable discovery. Keep discovery names exactly as returned: standalone `$wombat`, base plugin `$wombat:wombat`, or collection plugin `$wombat-collection:wombat`. Multiple enabled instances require the user's choice.

Prepare the smallest live usage query for the original question. Existing Rust log adapters handle history and later appends. For continuous read-only synchronization, the user can keep `wombat usage --watch --json` running. A one-time query does not establish a permanent background watcher.

If the user requests live collection, select `wombat collection mode hooks --json`. Then install the local collection plugin from the runtime's `lib/skill` marketplace. The base plugin contains no Hooks. Select one Skill installation mode rather than adding competing instances.

Codex owns plugin installation, removal and native `/hooks` trust review. The collector does not change the native trust store. Local collection wiring currently uses a POSIX shell. Windows acceptance is incomplete.

A custom runtime prefix needs its launcher on Codex's PATH. No public Hooks plugin listing is claimed.

`wombat collection mode logs --json` stops accepting new Hook events and preserves retained data. `pause` keeps a bounded safe buffer. `resume` marks those records available. These preferences are machine-wide, not project-specific. Viewing historical logs remains available. Pausing receipt processing does not pause Codex.

`collection events --project PROJECT --limit 50 --json` returns safe observations. Use `nextAfter` with `--after` for the next page. Native session/turn/tool identifiers are evidence. Use returned `association` IDs only when the core verifies matching committed log facts.

Native IDs alone are not Wombat public IDs. Do not associate by text, time or path similarity. `sourceEpoch` binds the current log projection, not a fixed usage snapshot. Obtain a normal fixed read view for comparisons.

`occurredAt` can be absent. `receivedAt` is collection time. Events without reliable native identity stay distinct and contribute to `identityUnknown`. Raw prompts, tool arguments, tool output and credentials are not stored.

Hook events never add Token measurements or costs.

Give separate details for mode, pause, actual receipt, registration/trust, history and view freshness. Waiting for a first event is not successful receipt. Retained events do not prove current registration or trust. `gaps` counts observed capacity/conflict losses, not all possible missing events.

Input rejection or an unavailable runtime can only be reported by the advisory bridge. Do not claim lossless collection. Storage is bounded to 100,000 retained observations and 4,096 paused observations. A filled store refuses new observations and counts known gaps.

Never delete user data as recovery. Unknown formats fail without migration or deletion.
