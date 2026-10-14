# Decision Note: Bounded conversation access for Codex

English | [中文](2026-10-14-conversation-access.md)

Status: proposed

## Problem

Daily work reports and follow-up questions need conversation content as well as usage facts. Existing queries locate tasks, turns and operations within Wombat. Internal events retain source, file, generation and byte positions, but public queries do not expose an original-content reader. Usage-filtered task lists also cannot establish a complete activity inventory when usage records are absent.

## Proposal

Reuse existing safe event positions and source ownership in Rust. Separate activity discovery, candidate message locations and explicit bounded original-content reads. Do not create another accounting ledger, copy conversation bodies into the index, or add a scheduler for this capability. This proposal supplements the [Codex Skill proposal](2026-10-04-codex-skill.en.md); it does not supersede existing usage, privacy or storage decisions.

Selection must compose source, project, task, turn, message type and calendar scope. Keep source-native identities distinct from internal identities. Default context stays within one turn; broader context requires an explicit selection. Support bounded batches, fragment continuation and keyword search within selected sources. Bind cursors to selection, read version and ordering. Return original-content locations and evidence with the answer. Recorded message types and failures identify candidates; they do not establish semantic importance or completed work.

Keep metadata discovery separate from content access. Validate source identity and selected content before reading, and preserve explicit changed, missing, unsupported and incomplete outcomes. Appends beyond an indexed boundary cannot silently enter a fixed result. Use the existing Rust timezone and attribution rules. Public DTOs and validators must derive from Rust. Deliver through the authorized Web host and non-TTY interfaces with cancellation and response budgets.

The [synthetic experiment](../../../../scripts/benchmark-conversation-access.ts) tests a SQLite position index and direct seeks. The [prototype](../../../../scripts/conversation-access-prototype.ts) is research code with a fixture-only grammar, not a production adapter or public contract. Its complete-record fingerprints remain experimental. Production uses bounded text-block fingerprints under the [privacy owner](../../../reference/privacy.en.md); the [core reference](../../../reference/core.en.md) owns delivered behavior and bounds. Current snapshots and regular query responses must continue to exclude conversation bodies. An explicit content reader needs a separately documented boundary.

## Alternatives considered

Direct original-file reads remain a useful lower bound when the caller already knows an exact position. Native keyword search avoids an index build and suits small sources. Repeated full parsing provides flexible semantic filters but revisits unrelated content. A second full-text database would add body retention, storage and synchronization costs. The experiment tests metadata indexing plus scoped original-file search; it does not establish a need for embeddings or a full-text copy.

## Acceptance criteria

- Compare identical synthetic results against complete parsing, native keyword location in a known file, and a known-offset read. Report first-process and repeated query time, startup, read bytes, response bytes, index construction and size, peak process memory, and cache conditions separately. An already-known position can be faster than the tool.
- Target under 100ms for indexed location and bounded snippet requests at the actual assembled entry. Prototype in-process timings do not validate CLI, Web, cold OS cache, or target platforms. Do not claim a universal speedup for short conversations.
- Verify composed filters, batches, same-scope continuation, Unicode, byte envelopes, source identity collisions, absence of usage, open tails, append, truncation, replacement, timestamp-preserving rewrite, cancellation and resource exhaustion. Missing results and unfinished keyword scans must remain distinct.
- Avoid repeated full-record parsing during large-message continuation. Bound keyword work and return resumable coverage instead of treating an unfinished scan as no matches. Verify resources at the production source adapter and transport, including messages larger than the ordinary snippet budget.
- Add source-backed activity discovery, version validation, supported message mappings and generated contracts before calling this delivered. Verify real-core, Web, non-TTY and target-platform behavior independently. Keep real-source output outside the repository.
