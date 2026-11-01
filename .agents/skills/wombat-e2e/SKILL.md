---
name: wombat-e2e
description: Run Wombat real-core integration and CLI end-to-end checks, or prepare a clean first-install acceptance run.
---

# Wombat E2E acceptance

Use `corepack pnpm verify:e2e -- --output-dir /tmp/wombat-e2e` for integration and CLI/core E2E checks. Default scope is `all`; choose `--scope integration` or `--scope e2e` for a focused run. The required output directory must be outside the repository; it retains the report and bounded stage logs.

For an ordinary suite run, use its disposable fixtures. Do not reset the active user installation. For first-install acceptance, complete this preparation before building, installing or querying:

1. Disable Wombat automatic startup and stop its processes using the selected test directories before removing files. Confirm that the native user-session manager cannot restart the old service. Identify Wombat-owned data, caches, plugin registrations and installation versions; keep source logs read-only.
2. Delete all historical Wombat product data and versions in those directories, including indexes, snapshots, configuration and handling records. Do this during preparation, not after a failed test. For an authorized real-machine reset, remove Wombat native plugin and marketplace registrations before deleting their version directories, then remove the old managed launcher and Wombat plugin cache. A stale marketplace path can make native plugin listing fail before a new registration is added. Never delete Codex source logs or other plugins. Without authorization to erase an active user store, use a dedicated disposable workspace and clear its Wombat paths.
3. Confirm that the selected product-data and installation paths are absent. Build the current source, install the candidate through the real installer, and start the initial scan. Do not seed the run with an old index, checkpoint, snapshot or installed runtime to repair failures.
4. Record the fresh preparation and first-install results. Run restart, restore and upgrade scenarios separately, using only state created by their own fixture; they cannot substitute for the first-install run.

The unified verifier clears its disposable product-data path during preparation. Archive verification separately clears candidate installation/data paths and invokes the real installer before product queries. These steps do not flush operating-system file caches or establish physical-cache-cold performance.

The verifier checks the current build and runs the standard build when stale. It stops on the first failed stage. Inspect `e2e-report.json` and that stage's log, fix the responsible implementation or fixture, then rerun the narrowest scope that covers the change. Use `--resume` only to reuse successful stages with matching source, acceptance fixtures, workspace artifacts, parameters; failed or mismatched evidence is rerun.

Use synthetic sources for the unified suite. A passing run covers only the selected scope and platform. It does not establish individual real-history behavior, semantic recommendation quality or later adoption. Verify authorized real sources separately in a fresh isolated product home, without changing the original logs.

This entry does not run core/unit suites, benchmarks, or installation checks. Select those separately with [wombat-verify](../wombat-verify/SKILL.md) when the change requires them; reuse unaffected passing evidence.

After the selected scenarios and dependent checks finish, apply [artifact cleanup](../wombat-verify/references/cleanup.md) to test processes, copied sources, indexes, isolated installations, and build intermediates. Save reports first. On interruption, name the artifacts still required for reproduction or valid `--resume`; clear the rest and report any cleanup failure.

Run performance acceptance only when requested. Select its scope, fixtures and targets separately from functional E2E checks; follow the selected benchmark entry's gates and preserve complete samples. Do not add performance runs as prerequisites for an ordinary functional test.
