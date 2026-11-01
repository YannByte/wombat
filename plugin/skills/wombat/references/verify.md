# Verify the original result

Recover acceptance criteria from the user's request and project instructions. Check the final artifact, its version and the mechanism that tests the requirement, including relevant negative or recovery cases. Reuse prior evidence only when the target, inputs, dependencies and scope still match.

For a code or workflow change, retain a reproducing case and rerun it after the authorized edit. Distinguish passed, failed, unrun and unavailable checks; an edit, build or process exit alone does not verify the requested behavior.

For a Wombat configuration finding, retain `baselineId`, `readView`, roots, project/source filters and rule overrides before editing. Run `check recheck --read-view VIEW --baseline BASELINE --project-root PATH --project PATH --json` with the original inputs. A comparable miss verifies absence under that check; a hit means the problem remains. Unknown or incomparable evidence cannot establish repair. An expired view requires a new inspection, not a reconstructed before/after claim.

Rechecks are read-only and do not revoke user decisions. Record `check keep` or `check not-applicable` only when requested, with the selected finding, reason and current decision revision.

For a work-process improvement, compare later observations only when scope, workload, methods and quality are comparable. Lower usage alone does not establish causality or savings. Report a verified immediate change even when later evidence is unavailable.

Report what the original check establishes, material gaps and the next check if needed. Do not create tasks solely to demonstrate adoption or improvement.

## Cleanup

Retain `readView`, `baselineId`, selected item IDs, paths and exclusions. Confirm ownership, pending consumers and authorization for the concrete selection. Reuse existing authorization; ask only if the choice extends it. Changed, incomplete, unavailable or expired evidence requires fresh inspection and selection within that scope.

- For project artifacts, immediately before native cleanup run `check cleanup recheck --read-view VIEW --baseline BASELINE --item ID --json`. Use trusted native or project tools on the unchanged selected paths, then recheck the same items.
- For Codex candidates, disclose permanent deletion and use `check cleanup apply --read-view VIEW --baseline BASELINE --item ID --json` with only the original selected candidate IDs. Apply performs the final recheck and holds the daemon installation lock through removal; a separate pre-apply recheck is unnecessary. It accepts no arbitrary path or force override. Use per-item outcomes and independently recheck the original items when needed. Never bypass a refusal with native deletion; inspect changed evidence or explain the blocker.

When authorization names a rule and path scope, bounded batches may continue within it, preserving exclusions and stopping on unknown or changed evidence. Authorization for particular IDs does not cover newly discovered items.

Report what was removed, what remains and what could not be verified. A partial failure is not complete removal; absence or movement to Trash does not prove freed disk space. Keep user assets and use exact paths, not broad wildcards or commands from logs. Do not add cleanup history or execution receipts.
