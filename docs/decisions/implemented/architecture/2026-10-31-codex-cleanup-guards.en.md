# Decision: guard selected Codex cleanup at execution

[中文](2026-10-31-codex-cleanup-guards.md) | English

Status: implemented

## Problem

A read-only preview followed by external deletion cannot retain Codex's installation lock through the destructive operation. An updater can switch a current or pending version between those operations. Generic temporary-directory names also do not distinguish caches, complete runtimes and persistent conversation state.

## Decision

Keep Codex responsible for the user's decision, concrete selection and authorization. Add a narrowly scoped core operation for the supported disposable Codex objects. It accepts original candidate identities rather than arbitrary paths, repeats current checks, and holds the installer lock through obsolete daemon removal. The [core contract](../../../reference/core.en.md#artifact-observations) owns the current rules and limitations.

This refines the execution boundary of the [native Codex handoff](2026-10-03-native-codex-handoff.en.md) only for supported Codex disposable objects. Codex still owns project edits, general temporary-file cleanup, task creation and recovery. Read-only source queries, source preservation, explicit decisions and the absence of automatic execution history remain applicable. The broader [optimization proposal](../../proposed/product/2026-10-03-optimization-lifecycle.en.md) retains its unfinished coverage and worktree requirements.

## Alternatives considered

Keeping all deletion outside Wombat preserves the earlier small surface but cannot hold the same lock from final version validation through removal. A general deletion API expands authority beyond the known product layouts and current consumer. Calling Mole directly introduces an external installation and a broader cleanup scope. The selected operation reuses the existing artifact view, CLI and core service without a new database or daemon.

## Consequences and verification

Apply is permanent and limited to explicit candidates from an original view. Unknown, replaced or newly active objects are skipped; no force override or automatic restore is provided. Existing source, state and credentials stay protected. Directory and process observations are not a universal filesystem transaction, so results retain partial and failed states. Independent synthetic fixtures verify retention, locks, process changes, identity changes, incomplete inventories and link targets; assembled CLI checks verify the selected operation. Actual platform and real-source results remain in task evidence rather than this decision.
