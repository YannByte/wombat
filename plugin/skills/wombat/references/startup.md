# Startup and data readiness

Prefer the executable selected by the user, otherwise the managed user launcher `~/.local/bin/wombat` (`~/.local/bin/wombat.cmd` on Windows) when its `~/.local/lib/wombat/.managed-by-wombat` ownership marker exists, then the `wombat` launcher on PATH. A custom install prefix needs the user-selected path. Quote executable paths and pass arguments separately. Reuse the verified executable, capabilities and successful invocation within this conversation; do not repeat version/help probes for follow-ups. Run `setup --project PATH --json` only for installation or integration trouble, not before ordinary usage questions. Check `--version --json` and only the relevant command's `--help` before using an unfamiliar capability; never infer support from version alone. Source worktrees can use the documented built entry with their Node runtime, but users need no Node or Rust installation.

No runtime: explain what is missing and refer to the project's installation instructions. Do not download, upgrade, enable plugins or read login credentials without the user's request. A remote host without local logs cannot answer local usage.

Start with the smallest query for the original task. Live usage queries discover supported sources, restore the index and sync automatically. Do not run `refresh` first unless a saved snapshot is requested. Configuration static checks can run while history is incomplete. Account reads do not wait for logs. Source roots and current project roots are distinct; never search the disk for projects or infer one from a path substring.

Inspect returned freshness and coverage:
- Complete current/fixed results support the requested conclusion and pinned drill-down.
- initialScan or provisional history: explain incomplete totals; existing tasks can be inspected, but no complete rankings or unused assertions.
- SYNC_PENDING without data: explain preparation and retry the same query with `--fresh` at most once. Normal queries wait roughly 2 seconds; fresh waits at most 10 seconds and may return SYNC_TIMEOUT. Preserve the original question for “continue”; a timeout does not stop shared scanning.
- syncing/stale with committed results: state the data time and answer what is supported; wait only if latest data is needed.
- failed/partial: retain usable facts, disclose failed sources or coverage.
- Completed empty results: distinguish absent supported logs from filters matching nothing.
- Expired view: reread the same scope, announce the version change. Unknown formats: preserve data and report, never clear or migrate.

`--cached` and fixed snapshots stay offline. Missing cache returns NO_SNAPSHOT, not zero usage. Set `WOMBAT_AUTO_PRICES=0` when offline is requested. Do not invent a readiness flag, database query, project percentage or ETA. The existing Web may display ongoing preparation; opening it must not create another index.

`LOCAL_ACCESS_DENIED` identifies a local process or service permission failure, including account/setup section errors. Request only the access needed for the same read-only command, retain its executable, environment and scope, and retry once after access is granted. Do not restart scanning, reinstall, read credentials, broaden permissions for the whole task or substitute raw-log scans. A generic `CODEX_DISCONNECTED` is not proof of a permission failure. Keep the verified data directory on every isolated command when shell environments do not persist.

For the Agent interface, probe `api --json` once and reuse its runtime version and method list. Read `api --method METHOD --json` only for the needed input Schema; request `--output-schema` only for unfamiliar response fields. Prefer `call` with one stdin JSON object `{method,params}` for nested filters and follow-ups. `usage` params wrap `{query,mode,verify}` and return `{result,freshness}`; `snapshot` accepts the direct fixed-view request. Set `compact:true` and a small limit explicitly. Retain the original query's complete IDs, scope and version. Methods/actions that write still need user authorization; a recovery hint never grants it. On an older runtime without `api`, use the existing command examples. Read its relevant help once rather than repeatedly probing.
