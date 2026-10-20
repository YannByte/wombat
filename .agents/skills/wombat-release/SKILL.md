---
name: wombat-release
description: Build and release Wombat GitHub archives, run release gates and clean installation/update checks, and verify published assets; use for packaging, publication, and release failures.
---

# Wombat Build and Release

Prepare and verify a reviewable candidate within the requested scope. Builds and candidate preparation do not authorize commits, pushes, tags, publication, or visibility changes. Continue under existing explicit authorization; otherwise present the concrete candidate before requesting publication approval.

Select references by the requested operation:

- Local build, packaging, or candidate verification: read [candidate](references/candidate.md).
- Development Preview or version preparation: also read [preview](references/preview.md).
- Release-note writing or review: read [notes](references/notes.md).
- Public publication or recovery: read [publication](references/publication.md), plus candidate/preview details only for unfinished preparation stages.

Use the repository's primary TypeScript entries through `corepack pnpm`: `release:prepare`, `github:pack`, and, for authorized publication, `release:publish`. These entries deliberately remain in `scripts/`: packaging and CI consume the same implementation. Do not copy release mechanics into the Skill or load script source unless diagnosing a failure.

Preserve failed-stage evidence and follow the [failure investigation rules](../../../docs/development/workflow.en.md#failure-investigation) before another platform run. Resume with the same version only while published identities still match; never replace public tags or assets. Read current runtime/target requirements from source and report actual version, commit, gates, platforms, hashes, installation/update results, and publication state. Current behavior belongs to [distribution ownership](../../../docs/decisions/implemented/architecture/2026-10-04-github-release-distribution.en.md), not this workflow.

Finish local builds, candidates, and publication runs with [artifact cleanup](../wombat-verify/references/cleanup.md), after all dependent checks. Keep delivery archives, required resume inputs, reports, and verified external caches; remove disposable installations and build intermediates that no longer have consumers. Report retained paths and reasons if work stops before publication.
