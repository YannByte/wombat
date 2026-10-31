# Client Instructions

Follow [root instructions](../AGENTS.md) and the [delivery workflow](../docs/development/workflow.en.md). Public entries and limitations belong in the [module README](../docs/reference/client.en.md).

- Generate src/generated/ TS types, schemas, and validators from Rust DTOs. Change the source, never generated files to alter public semantics.
- Keep src/index.ts injectable and free of Node/terminal dependencies. src/node/ owns core paths, subprocess cancellation, timeouts, output bounds, and cleanup.
- Validate requests/responses at trust boundaries. Match Rust protocol versions, operations, and error codes; never disguise unknown fields or failures as success.
- For generated-contract changes, rebuild core/client before client tests, typechecks, and real-core integration tests. Do not test stale dist.
