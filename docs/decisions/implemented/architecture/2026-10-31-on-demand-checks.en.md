# Decision Note: read-only checks and one plugin

[中文](2026-10-31-on-demand-checks.md) | English

Status: implemented

## Problem

Task sending, durable review observations, adoption tracking and alternate installation modes repeat work already owned by Codex. Unimplemented rules and optional setup probes obscure actionable evidence.

## Decision

Ship one main Skill and one plugin. Codex owns interpretation, authorized edits, tasks and recovery. Wombat supplies bounded facts, implemented checks and explicit decisions. Recheck uses the retained original view and an opaque scope/rule-bound baseline; an expired view requires a fresh inspection. It does not reconstruct history or infer adoption. Keep the current decision format and applicable identities without migrations, while removing automatic writes and display history. Preserve the accounting pipeline.

This replaces Wombat-owned task sending and automatic review/adoption lifecycle in the [native handoff decision](2026-10-03-native-codex-handoff.en.md), [optimization proposal](../../proposed/product/2026-10-03-optimization-lifecycle.en.md), and [Skill proposal](../../proposed/product/2026-10-04-codex-skill.en.md). Their source-read-only, evidence and native execution rationale remains applicable. Other unfinished evidence requirements remain proposals, not shipped capabilities.

## Alternatives considered

Retaining the lifecycle and multiple installation modes preserves more state and ambiguity. Returning only current checks loses the original comparison condition. The retained-view baseline keeps that condition without another persistent store. Its cost is explicit expiry, rather than an unsupported repair claim.

## Consequences and verification

Checks no longer accumulate automatic handling records. User decisions retain current storage format 4. An expired original view requires a new inspection; no replacement baseline can establish the earlier comparison. Default setup needs no optional Hooks; only explicit requests read registration evidence. Unknown versions reject with data retained, without migrations or retired interface aliases.

[Core tests](../../../../core/src/optimize/tests/stateless.rs) cover read-only behavior, scope binding, expiry, incomparability and decisions. [Real flows](../../../../tests/e2e/static-rules.test.ts) cover original-condition rechecks and relationship evidence gaps. [Delivery checks](../../../../scripts/check-delivery.test.ts) reject removed hosts, installation entries and collection plugins. Generated contracts, complete product tests, and fresh installation, native discovery and upgrade on macOS arm64 pass. Frozen real-source usage and task statistics match field by field. Windows, Linux and complete model advice quality were not requalified in this change.
