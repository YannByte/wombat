# Build and test cleanup

Finish build-dependent checks, packaging, and installation verification before removing their inputs. Compilation success alone does not mean downstream consumers have finished. Apply this workflow after successful work and when stopping after a failure.

1. Identify exact task-owned paths: build intermediates, disposable workspaces, copied test sources, indexes, isolated installations, and incomplete downloads. Confirm ownership before including older artifacts. Do not sweep shared temporary directories or follow symlinks into unrelated paths.
2. Stop task-owned processes and confirm that no active consumer needs each path. Preserve requested deliverables, source changes, original conversations, user configuration, live installations, dependencies, and complete external downloads verified for size, hash, and signature where applicable.
3. Save necessary reports, measurements, and failure evidence outside the repository before deletion. Keep only inputs and artifacts needed for an unfinished authorized stage or reproduction, recording their paths and reasons. Removing artifacts required by a resume fingerprint invalidates that stage's reuse; do not claim the report alone permits resumption.
4. Proactively remove unneeded intermediates, including task-generated `target/` and `dist/` contents once their consumers finish. Use the owning entry's cleanup when available; do not assume that a successful build or check cleaned its outputs. Confirm removal and that preserved files remain intact. Report cleanup failures and retained exceptions; never report attempted removal as success.

Do not rebuild merely to prove cleanup or add builds to source-only checks. A rules or Skill edit needs no new compilation artifacts.
