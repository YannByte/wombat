# Installation and optional collection checks

Use `wombat setup --project PROJECT --json` for installation trouble. It checks the runtime, the single formal plugin `$wombat:wombat`, and its actual discovered resources. It does not scan history or probe Hook registration. Use `doctor --json` for the installation, PATH and core alone.

Use `setup --hooks --project PROJECT --json` only when the question needs native Hook registration or trust. Codex owns those settings. Missing or partial optional Hook evidence does not establish that the main Skill installation failed.

Log adapters provide history and later appends. Explicit optional collection interfaces remain available for user-managed native wiring: `collection status|events`, `collection mode logs|hooks`, `collection pause|resume` and `hook codex`. No collection plugin or automatic Hook registration is shipped. Change mode, startup or trust only with the user's authorization.

Hook receipt is advisory metadata, not Token accounting. Raw prompts, arguments, output and credentials are not persisted. Native identities require a verified log association; receipt time is not event time. Retained observations do not prove current registration, and gaps count known losses only. Receipt is bounded to 100,000 observations and 4,096 paused observations. Unknown formats fail without migration or deletion.

Use the smallest usage query for a usage question. Inspect service state only to investigate runtime trouble. Queries prepare data on demand; an explicitly enabled resident lifetime does not establish background refresh or installed budget scheduling.
