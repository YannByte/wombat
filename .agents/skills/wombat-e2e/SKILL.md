---
name: wombat-e2e
description: Run and interpret Wombat's real-core integration, end-to-end, and browser acceptance checks through the unified verifier.
---

# Wombat E2E acceptance

Prepare first-install runs before building, installing, or querying:

1. Stop Wombat processes using the selected test directories. Identify Wombat-owned data, caches, plugin registrations and installation versions; keep source logs read-only.
2. Delete all historical Wombat product data and versions in those directories, including indexes, snapshots, configuration and handling records. Do this during preparation, not after a failed test. For an authorized real-machine reset, also remove the old managed launcher and Wombat plugin cache/registration. Never delete Codex source logs or other plugins. Without authorization to erase an active user store, use a dedicated disposable workspace and clear its Wombat paths.
3. Confirm that the selected product-data and installation paths are absent. Build the current source, install the candidate through the real installer, and start the initial scan. Do not seed the run with an old index, checkpoint, snapshot or installed runtime to repair failures.
4. Record the fresh preparation and first-install results. Run restart, restore and upgrade scenarios separately, using only state created by their own fixture; they cannot substitute for the first-install run.

The unified verifier clears its disposable product-data path during preparation. Archive verification separately clears candidate installation/data paths and invokes the real installer before product queries. These steps do not flush operating-system file caches or establish physical-cache-cold performance.

Use the real verifier for acceptance. From the repository root, run `corepack pnpm verify:e2e -- --output-dir /tmp/wombat-e2e --playwright-module /absolute/path/to/playwright/index.js`; the default `all` scope runs integration, CLI/Web E2E, and the event-upgrade browser checks. Pass `--scope api` or `--scope browser` for a focused rerun. Browser runs require `--playwright-module PATH` or `WOMBAT_PLAYWRIGHT_MODULE`; pass `--browser-executable PATH` when needed. Reports and bounded per-stage logs stay in the required repository-external output directory.

The verifier checks the current build and runs the standard build when stale. It stops on the first failed stage. Inspect `e2e-report.json` and that stage's log, fix the responsible implementation or fixture, then rerun the narrowest scope that covers the change. Use `--resume` only to reuse successful stages with matching source, acceptance fixtures, workspace artifacts, parameters, and external browser identities; failed or mismatched evidence is rerun.

Do not substitute UI preview checks for E2E acceptance. Preview journeys use typed synthetic transports and validate UI behavior only; they do not establish real-core behavior. The browser acceptance runner uses synthetic source and data, records no screenshots, and covers only the selected platform. A human must still review product claims, privacy implications, and any platform or visual behavior outside the automated checks. Do not use real Agent logs or treat a passing synthetic run as evidence about an individual user's data.

This entry does not run core/unit suites, benchmarks, or installation checks. Select those separately with [wombat-verify](../wombat-verify/SKILL.md) when the change requires them; reuse unaffected passing evidence.
