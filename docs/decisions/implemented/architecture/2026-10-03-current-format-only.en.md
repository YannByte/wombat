# Decision Note: Maintain only current formats before the first release

[中文](2026-10-03-current-format-only.md) | English

Status: implemented

## Problem

The product had not been released when this decision was made. Old snapshot readers, database migrations, identity remapping, page links and generation protocols add maintenance branches and resource costs. The user requested no compatibility logic and clean code.

This supersedes compatibility choices in [snapshot storage](2026-09-30-snapshot-storage.en.md), [index migration](2026-10-02-compact-live-index.en.md) and [review migration](2026-10-02-rule-review-integrity.en.md).

## Decision

Maintain only current snapshots, database layouts, generation protocols and page routes. Remove old snapshot readers and amount policies, index/review migrations, identity-registry mapping, page parameters and whole-batch generation branches. Operations use current physical-object identities; per-source contexts still attribute evidence.

Reject unsupported layouts or versions while retaining original data, without automatic conversion, deletion or empty-record substitution. Immutable history, usage, review evidence and recovery materials in the current format remain. Cumulative and per-response measurements are genuine source facts and remain supported by source adapters.

## Alternatives considered

Keeping read-only legacy readers and transactional migration retains branching and validation costs, so it was not adopted during that pre-release phase. Deleting unsupported data would lose records and was also rejected.

## Impact and verification

Earlier development layouts may no longer be readable. Each release still requires verification of clean installation and complete journeys for the current version. The [core reference](../../../reference/core.en.md) owns current formats and recovery behavior, and the [development workflow](../../../development/workflow.en.md) owns verification requirements. Tasks, CI, or verification-tool artifacts retain actual run evidence; historical tests or builds do not replace current acceptance.
