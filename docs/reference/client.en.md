# Typed client

[中文](client.md) | English

`@wombat/client` validates requests and results and gives the CLI one `UsageClient`. Business DTOs come from Rust-generated contracts; the [architecture](../development/architecture.en.md) shows the cross-module data flow.

## Public entries

- The package root exports `createUsageClient({ query, ...transports })`, types, and `CoreError`. Named optional transports include `timing` and `account`; the root loads no Node or terminal library.
- `createNodeClient` from `@wombat/client/node` manages the local core subprocess, cancellation, timeouts, and response limits.
- `UsageClient.live` supplies foreground auto/fresh/cached queries and last-check metadata; `query` reads current usage facts.
- `UsageClient.timing` supplies bounded whole-turn summaries and evidence pages. Opaque cursors bind targets and selections. Capability queries perform no scan.
- `UsageClient.prices` provides offline inspection, explicit updates, and `auto_update` checks for missing prices. The Node live client triggers fixed-HTTPS downloads; Rust owns eligibility, persistent throttling, validation, storage, and pricing.
- Generated files live in `src/generated/`; field and version authority remains with the Rust DTO and [contracts](../development/contracts.en.md).

- Presentation language uses `@wombat/client/locale`; see [product language and copy](../i18n/product.en.md).

## Limits and verification

The client validates generated product requests and results at the boundary. Node uses a bounded foreground core subprocess for usage, conversation, configuration, checks, timing, monitoring and cleanup. It does not discover a socket or start a service. The default process deadline is 600 seconds; `timeoutMs` and response limits remain host options. Cancellation stops the owned process tree and rejects late reads. A cancelled or timed-out write may already have committed; inspect current state before retrying.

The core owns source synchronization, SQLite transactions, cursor validity and baseline comparisons; see [internal storage](core.en.md#foreground-queries-and-internal-storage). Up to eight additional usage queries share the base request's source scope and revision, within a 2 MiB batch. `query` reads current usage facts; `live` supplies explicit auto/fresh/cached collection controls. Neither accepts a public view selector.

Native process access failures preserve `LOCAL_ACCESS_DENIED` when an OS denial is observed. Native diagnostics are not returned as source content. The transport does not grant access or automatically replay writes. Follow the [client rules](../../client/AGENTS.md) and run generated-contract and real-core checks after changes.

## Configuration and timing


The CLI exposes timing summaries, evidence and capabilities. Use `corepack pnpm verify:e2e` for real-core and CLI acceptance on the selected platform.

`UsageClient.config` supplies measurements and use evidence. `optimize` supplies read-only checks, original-condition rechecks and explicit user decisions; `preferences` gets/sets zh/en. Configuration `useBasis` retains fixed scope, method, source completeness and coverage. Locale explains facts without recalculation or adoption inference. See [public contracts](../development/contracts.en.md) for fields and versions.

`createNodeClient({automaticPrices:false})` disables only the client’s automatic-price decorator; explicit prices operations remain available. CLI live queries retain their existing default behavior.


## Setup and runtime observations

`UsageClient.setup` and `collection` use Rust-generated version 1 contracts. Setup checks cwd-scoped plugin discovery, resource hashes and runtime capabilities by default without collecting source logs. Only explicit `includeHooks` queries optional registrations. Completed results and errors remain independent within the eight-second overall bound. Setup does not install, grant trust, read account credentials or call models. Node `receiveCodexHook` sends bounded input through the private core boundary. Collection has a five-second deadline and 2 MiB output bound; Rust owns safe facts and associations.

Setup returns the complete native Codex version, including prerelease/build identifiers, the running Wombat bundle version, and checks of each enabled discovered copy against its content hash and declared capabilities. The shipped plugin declares one main Skill. Checks bind its directory and content hash. Different plugin/runtime versions can be compatible; matching versions alone do not establish compatibility. Missing declarations remain unmanaged, and modified or unsupported copies have separate error codes. Setup also returns the validated bundled marketplace path for explicit native installation commands. Missing resources remain a partial setup error; native discovery and collection facts remain independently readable.

`UsageClient.monitor` supplies budget settings, explicit foreground checks, notifications and acknowledgement. Statistical queries use the usage entry. See the [core](core.en.md) for populations and storage and the [CLI guide](../guides/cli.en.md) for procedures.
