# Typed client

English | [中文](README.zh-CN.md)

`@wombat/client` supplies one `UsageClient` to CLI and Web callers. Rust defines the business contracts; the client validates requests and results.

## Public entries

- `@wombat/client` exports `createUsageClient`, types, and `CoreError`. This entry loads no Node or terminal libraries.
- `@wombat/client/node` exports `createNodeClient` for the local core process and shared service.
- `@wombat/client/node/runtime` enables optional Node module compilation reuse for managed CLI installations. Its cache belongs to the installed version, contains no query data, and is removed with that version. Read-only installations and Node cache overrides retain normal behavior.
- `@wombat/client/http` exports `createHttpClient` for the local Web host.
- `@wombat/client/locale` supplies presentation language support.

## Limits

The client exposes only generated operations. It provides no arbitrary shell execution or generic file writes. A cancelled wait does not prove that a write did not occur.

## Read next

- [Client reference](../docs/reference/client.en.md): transports, deadlines, cancellation, setup, and collection.
- [Public contracts](../docs/development/contracts.en.md): generated types and independent versions.
- [Client instructions](AGENTS.md): maintenance and verification rules.
