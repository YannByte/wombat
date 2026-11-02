# CLI Instructions

Follow [root rules](../AGENTS.md) and the [delivery workflow](../docs/development/workflow.en.md). Public behavior and [Agent principles](../docs/reference/cli.en.md#agent-interface-principles) belong in the module README.

- Own arguments, help, JSON/text, progress and exits; use public typed clients for business rules. Agent and human entries share contracts and exit policies.
- Default to usage text. Begin with the requested task, without mandatory initialization. Cancellation stops the owned foreground core process tree. No terminal renderer or FFI.
- JSON uses generated contracts; progress uses stderr. Preserve partial, unknown, error and cancelled states. Agent Schema discovery is local and read-only; recovery grants no mutation or scope expansion.
- Rebuild before affected CLI/unit and real-core tests. Update both languages; test invalid inputs, bounded IO and installed paths.
