# Rust Core Instructions

Follow [root instructions](../AGENTS.md) and [architecture](../docs/development/architecture.en.md). This is a modular single crate; planned modules are not delivered capabilities.

- Prefer maintained libraries for parsing, Unicode, dates/timezones, hashing, temporary files, and mapping. Check maintenance, suitability, and licenses for additions; avoid a runtime for a small wrapper.
- Keep Cargo.lock fixed and build with --locked. Own Wombat domain logic, not generic parsers or cryptography.
- Process valid large log lines completely, including hashes. Keep message bodies out of snapshots; temporary files must be private and automatically cleaned.
- Compute dates using record times/requested timezone and projects using cwd/path components. Unresolved targets cannot establish repeated reads or verified tool calls.
- Use explicit DTOs for requests, scope, identity, state, and results; reserve Value for open source fields. Separate complete snapshots from scan summaries, and protocol versions from snapshot versions.
- Report coverage, freshness, resource limits, and parsing gaps. Source, conversation, and file identities differ; identical output does not prove duplicate ledger events.
- Centralize filesystem/process/platform behavior, keep domain logic independent of UI, and clean complete subprocess trees on exit/cancellation. Verify each platform claim separately.
- Historical paths use recorded Unix/Windows grammar and bases; physical identity uses file IDs, not timestamps. Test new behavior on every target; earlier install/startup passes cannot validate new analysis.
- Persist only explicit user decisions independently of rebuildable indexes. Inspections and rechecks are read-only. Bind rechecks to supplied baselines, scope and rules; do not retain views or add a handling lifecycle.
- Avoid retaining whole log bodies, all events, or multiple complete snapshots. Document complexity/resource limits; mmap is not free memory.
- For indexes/caches, inspect duplicated fields, shared facts, and temporary collections; state lookup/append/rebuild complexity and budgets. Distinguish database, WAL, and post-exit sizes. WAL retention targets are not hard limits; preserve ledger, coverage, current snapshots, and user records.
- Test changed algorithms with focused accuracy/failure fixtures. Performance claims require fixed fixtures and release builds. Run cargo fmt and cargo clippy --locked --all-targets -- -D warnings in core or with its manifest. Cross-language/CLI tests require a root corepack pnpm build first.
