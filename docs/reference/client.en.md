# Typed client

[中文](client.md) | English

`@wombat/client` validates requests and results and gives the CLI one `UsageClient`. Business DTOs come from Rust-generated contracts; the [architecture](../development/architecture.en.md) shows the cross-module data flow.

## Public entries

- The package root exports `createUsageClient({ query, ...transports })`, types, and `CoreError`. Named optional transports include `timing` and `account`; the root loads no Node or terminal library.
- `createNodeClient` from `@wombat/client/node` manages the local core subprocess, cancellation, timeouts, and response limits.
- `UsageClient.live` provides auto/fresh/cached queries and freshness; the Node host manages a shared local service, while `query` retains the fixed-snapshot interface.
- Optional `UsageClient.timing` provides whole-turn summaries, fixed-snapshot evidence pages, and offline capabilities through Rust-generated contracts. Node selects a live view when no snapshot is supplied; an explicit stored snapshot stays fixed. Timing does not trigger price downloads, Hook capture, configuration scans, or account observations.
- `UsageClient.prices` provides offline inspection, explicit updates, and `auto_update` checks for missing prices. The Node live client triggers fixed-HTTPS downloads; Rust owns eligibility, persistent throttling, validation, storage, and pricing.
- Generated files live in `src/generated/`; field and version authority remains with the Rust DTO and [contracts](../development/contracts.en.md).

- Presentation language uses `@wombat/client/locale`; see [product language and copy](../i18n/product.en.md).

## Limits and verification

The client allows only operations in the generated request union; it exposes no arbitrary shell, file write, or generic dispatch. It validates requests and results at the process boundary; build and run focused tests after changes, following the [client rules](../../client/AGENTS.md).

The Node client and local service use protocol 2 with a separate endpoint and service lock; protocol 1 is rejected. Ordinary live reads wait up to 12 seconds by default. Explicit `refresh` also writes and syncs a fixed snapshot, so it waits up to 120 seconds by default. `waitMs` sets one core synchronization wait up to 600 seconds; its default remains 10 seconds for fresh reads. The Node transport allows that wait plus 2 seconds. `timeoutMs` overrides the client deadline. Timing validates the response action, privacy profile, method, and local target and fixed snapshot identity. Share output omits local identities, so Rust owns its target binding. Live cancellation sends a bounded waiter-cancellation marker and rejects late results. The core releases that synchronization waiter; shared synchronization continues. A completed write can still have taken effect. Batched results must match the requested actions and base revision.

Read operations reject late transport results with `CANCELLED` when their signal has been cancelled. When a transport returns a completed preference write, directory authorization change, price update result, the client retains that result even if cancellation arrived late. A cancelled wait does not prove that a write did not occur.

Local socket and native process access failures return `LOCAL_ACCESS_DENIED` when an OS denial is observed. Native diagnostic text and paths are not returned. Account section errors preserve this cause when the connection cannot be established; missing values remain unknown. The caller can request narrow access and retry the same read, but the transport does not grant permission or automatically resend writes.

`UsageClient.service` uses generated version 1 requests and responses over Node. Rust owns readiness, progress, pause state and instance identity; the Node host owns current-user service registration. Managed launchers publish a private endpoint-file path. Clients validate its protocol, executable identity and instance before reuse, avoiding a core endpoint subprocess per installed query. Status reads do not start a stopped service. An ordinary query restarts the registered native job when its enabled registration matches the requested data directory; otherwise it starts an on-demand process.

## Configuration and timing


The CLI exposes timing summaries, evidence and capabilities. Use `corepack pnpm verify:e2e` for real-core and CLI acceptance on the selected platform.

`UsageClient.config` supplies measurements and use evidence. `optimize` supplies read-only checks, original-condition rechecks and explicit user decisions; `preferences` gets/sets zh/en. Configuration `useBasis` retains fixed scope, method, source completeness and coverage. Locale explains facts without recalculation or adoption inference. See [public contracts](../development/contracts.en.md) for fields and versions.

`createNodeClient({automaticPrices:false})` disables only the client’s automatic-price decorator; explicit prices operations remain available. CLI live queries retain their existing default behavior.


## Setup and runtime observations

`UsageClient.setup` and `collection` use Rust-generated version 1 contracts. Setup checks cwd-scoped plugin discovery, resource hashes and runtime capabilities by default without starting the index service. Only explicit `includeHooks` queries optional registrations. Completed results and errors remain independent within the eight-second overall bound. Setup does not install, grant trust, read account credentials or call models. Node `receiveCodexHook` sends bounded input through the private core boundary. Collection has a five-second deadline and 2 MiB output bound; Rust owns safe facts and associations.

Setup returns the complete native Codex version, including prerelease/build identifiers, the running Wombat bundle version, and checks of each enabled discovered copy against its content hash and declared capabilities. The shipped plugin declares one main Skill. Checks bind its directory and content hash. Different plugin/runtime versions can be compatible; matching versions alone do not establish compatibility. Missing declarations remain unmanaged, and modified or unsupported copies have separate error codes. Setup also returns the validated bundled marketplace path for explicit native installation commands. Missing resources remain a partial setup error; native discovery and collection facts remain independently readable.

`UsageClient.monitor` provides typed budget settings, explicit checks, notifications and acknowledgement over Node. Live checks stay in the shared service with their selected snapshot. Statistical queries use `action: statistics` through existing usage/snapshot methods. See the [core](core.en.md) for populations and persistence, and the [CLI guide](../guides/cli.en.md) for procedures.
