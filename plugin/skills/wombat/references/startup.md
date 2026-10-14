# Startup and data readiness

Select the executable in this order:

1. Use the executable selected by the user.
2. Otherwise, use `~/.local/bin/wombat` (`~/.local/bin/wombat.cmd` on Windows) if `~/.local/lib/wombat/.managed-by-wombat` exists.
3. Otherwise, use the `wombat` launcher on PATH.

A custom install prefix needs the user-selected path. Quote executable paths. Pass arguments separately. Use the same successful discovery and invocation within this conversation.

Use `setup --project PATH --json` only for installation/integration trouble. Source worktrees can use the built entry with their Node runtime. Installed users need no Node or Rust installation.

If no runtime is available, tell the user what is missing. Give the project's installation instructions. Do not download, upgrade, enable plugins or read login credentials without the user's request. A remote host without local logs cannot answer local usage.

Start with the smallest query for the original task. Live usage queries discover supported sources, restore the index and sync automatically. Unless the user requests a saved snapshot, do not run `refresh` first. Configuration static checks can run while history is incomplete.

Account reads do not wait for logs. Source roots and current project roots are distinct. Never search the disk for projects or infer one from a path substring.

Read the JSON body even when the command exits 2: it can contain usable partial facts or unconfirmed synchronization. Exit 1 is an error. Before another attempt, examine the stable error code and recovery instructions. Exit 130 means cancellation, not a request to restart. Keep progress on stderr separate from the final stdout JSON. Do not convert missing data into zero or discard all facts because one section failed.

Examine returned freshness and coverage:

- Complete current/fixed results support the requested conclusion and pinned drill-down.
- initialScan or provisional history: tell the user that totals are incomplete. Examine existing tasks. Do not claim complete rankings or unused resources.
- SYNC_PENDING without data: tell the user that preparation is in progress. Run the same query with `--fresh` at most once more. Normal queries wait roughly 2 seconds. Fresh waits at most 10 seconds and may return SYNC_TIMEOUT. Keep the original question for “continue”. A timeout does not stop shared scanning.
- syncing/stale with committed results: give the data time. Answer what the evidence supports. Wait only if latest data is needed.
- failed/partial: keep usable facts. Show failed sources or incomplete coverage.
- Completed empty results: distinguish absent supported logs from filters matching nothing.
- Expired view: read the same scope again. Tell the user about the version change.
- Unknown formats: keep the data. Show the format error. Never clear or migrate the data.

`--cached` and fixed snapshots stay offline. Missing cache returns NO_SNAPSHOT, not zero usage. If the user requests offline use, set `WOMBAT_AUTO_PRICES=0`. Do not invent a readiness flag, database query, project percentage or ETA. The existing Web may display ongoing preparation. Opening it must not create another index.

`LOCAL_ACCESS_DENIED` identifies a local process or service permission failure, including account/setup section errors. Keep the executable, environment and scope of the same read-only command. Request only the access that command needs. After access is granted, run the command once more.

Do not restart scanning, reinstall, read credentials, broaden permissions for the whole task or substitute raw-log scans. A generic `CODEX_DISCONNECTED` is not proof of a permission failure. Keep the verified data directory on every isolated command when shell environments do not persist.

Call `api --json` once. Use its runtime version and method list for later requests. Read `api --method METHOD --json` only for a needed input Schema. Request `--output-schema` only for unfamiliar response fields.

Prefer `call` with one stdin JSON object `{method,params}`. Send JSON through a file or a literal quoted here-document. Avoid shell interpolation and extra escaping. Examples in references also show human CLI syntax.

Use the generated method Schema for Agent calls rather than combining CLI flags with params. Set `compact:true` and a small limit where supported. Keep IDs, scope and versions. On INVALID_ARGUMENT, read the needed Schema.

Correct the request once. If the runtime has no `api`, read `--version --json` once. Read the relevant command's help once. Version alone does not establish support.

A recovery hint never authorizes a write retry.
