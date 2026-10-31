# Decision Note: One task Skill and a local CLI

[中文](2026-10-31-cli-task-skill.md) | English

Status: implemented

## Problem

Separate task Skills, Web startup and browser-specific delivery duplicated the user path. A diagnosis could spend work on setup before reading the relevant project evidence.

## Decision

Ship one main Wombat Skill with five on-demand task flows: resume, investigate, delivery verification, period review and project improvement. Reuse existing generated CLI contracts for exact facts and bounded history. Static project inspection uses Codex file tools directly, without an initialization phase. Keep accounting, prices, scope and version rules in the core.

Remove Web, UI, HTTP transport, browser-only schemas, build assets and acceptance stages. Do not add old command aliases or format migration. Keep the local core query service and optional collection, budgets, native handoff, user decisions and original-issue rechecks used by CLI consumers.

Diagnosis initially focuses on disagreement between instructions and actual entry points, checks bypassed by ordinary delivery, and failures without bounded handling or recovery. Recommendations require an observed consequence or deterministic defect, exact owner, smallest action and original verification. No finding quota, overall score or fixed model-role pipeline is required.

Do not create a second task database or automatic analysis records. Existing evidence identities and configuration handling records serve their current consumers. Semantic work links remain reviewed interpretations. Ordinary queries save no diagnosis. A selected follow-up retains its original scope, target version, evidence and success criterion through the existing check record or authorized project note. Static repair, later adoption and outcome improvement are distinct.

## Alternatives considered

Retaining Web or four task Skills preserves duplicate maintenance. Adding a universal task model or autonomous diagnosis service adds state before a current CLI consumer needs it. Copying a fixed multi-role scoring workflow raises evidence and model costs for simple questions.

## Impact and verification

Source-only repository guards reject removed delivery dependencies and enforce one shipped Skill. Product tests use real native clients and assembled CLI entries with independent synthetic accounting expectations. Real-source acceptance uses a frozen read-only corpus, separate empty product homes and the same queries before and after the change. Functional outcomes and partial coverage are reported outside the repository. Performance is outside this acceptance scope. These checks do not establish causal savings or later adoption.
