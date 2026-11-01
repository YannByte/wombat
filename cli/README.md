# CLI entry

English | [中文](README.zh-CN.md)

`@wombat/cli` reads arguments and writes text or JSON results. It connects typed queries to the local core.

## Public entries

- The package exports argument parsing and `runUsageCli`. `./format` exports text formatting.
- `history`, `usage`, `check`, `clean` and `system` organize user tasks. `system api` discovers generated contracts; `system call` reads one bounded JSON request from stdin.

## Limits

JSON results use stdout; progress uses stderr. User decisions remain separate from check results. Unknown storage formats require explicit recovery; do not delete the original data.

## Read next

- [CLI guide](../docs/guides/cli.en.md): commands, filters, budgets, and errors.
- [CLI reference](../docs/reference/cli.en.md): discovery principles, exit codes, timing, and format recovery.
- [CLI instructions](AGENTS.md): maintenance and verification rules.
