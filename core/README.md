# Rust core

English | [中文](README.zh-CN.md)

`wombat-core` reads local Codex records and creates fixed snapshots. It supplies usage queries, prices, timing, configuration checks, and budget monitoring.

## Entry points

- `src/main.rs` reads one JSON request from stdin. It writes the final result to stdout and progress to stderr.
- `usage_app_dto.rs` defines public requests and results. Tools generate TypeScript types and schemas from Rust.
- `src/adapters/` reads source records. Scanning writes only Wombat product data.

## Limits

Missing values, unpriced usage, partial reads, and resource limits remain explicit. Wombat rejects unknown data formats and preserves the original data.

## Read next

- [Core reference](../docs/reference/core.en.md): algorithms, accounting, storage, query limits, and service lifecycle.
- [Source acceptance](../docs/development/adapters.en.md): supported records and independent test expectations.
- [Development workflow](../docs/development/workflow.en.md): build and verification commands.
- [Core instructions](AGENTS.md): rules for code changes.
