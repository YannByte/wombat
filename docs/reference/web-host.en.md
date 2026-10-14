# Local Web Host

[中文](web-host.md) | English

`@wombat/web` exports `startWebHost(options)`, returning the browser URL, origin, asynchronous `close()`, and restricted `openView(request)`. The CLI injects a Node client; the host does not own pricing, queries, or source parsing rules.

## Access and resource limits

The listener binds to 127.0.0.1 with an automatically assigned port by default. The host copies the startup scope and keeps later grant changes in its own state; it does not modify the supplied options or promote temporary grants to startup scope. Startup arguments and host-managed directory grants determine source scope; HTTP callers cannot provide arbitrary roots or snapshot paths. The host retains up to 128 returned snapshot identities. Core revision retention is independent; expired versions fail explicitly and require a fresh query.

Startup generates a random token in the URL fragment. The browser moves it into sessionStorage and clears the fragment. APIs require a Bearer token, exact Origin/Host, and JSON POST; CORS is disabled. The root page contains no business data, and CSP forbids remote scripts and embedding. The token grants local service access; it is not a source API key. A server restart requires a new link.

HTTP exposes only generated query, synchronization, catalog, configuration, review, grant, preference, Codex handoff, account, and timing operations. Both requests and responses are validated. HTTP uses NDJSON progress/result/error envelopes without redefining business DTOs. Limits are 64 KiB input, 16 MiB output, eight concurrent requests, and a default 120-second timeout. An explicit live synchronization wait gets its requested deadline plus 5 seconds, capped at 605 seconds; generated request validation still owns the business limit. Usage batches retain the same host source scope. Disconnects cancel the corresponding call; CLI exit signals close the listener and cancel its own requests. The shared core service follows its existing idle lifecycle; one departing Web client does not terminate another entry's service. Closing a browser tab does not exit the CLI.

Static files come only from the built asset directory. Startup loads allowed file types, with no directory listing or source access. Node and browser output is bounded; these limits do not verify million-record memory goals. The service does not support LAN, remote, or hosted deployment and exposes no generic file writes, shell execution, or core dispatch.

## Timing reads

`/api/timing` uses the generated timing protocol. Summary and evidence requests must explicitly provide a snapshot identity already published by this host; an absent or unpublished identity is rejected without selecting a new view. The host rejects browser `roots`, `projectRoots`, and snapshot paths, revalidates directory grants, and injects its authorized source roots. Rust verifies that the source, thread, and turn belong to that exact version. Expiry, revocation, or eviction fails explicitly without falling back to the latest version or restoring the old grant.

Local summaries publish `readView.snapshotId` within the same 128-identity bound. Share summaries publish no read identity; share evidence is rejected. Configuration read-view identities remain separate. Capabilities do not select a snapshot or read directory grants. Timing does not load configuration, project working directories, prices, hooks, or account data. Request cancellation also prevents late results from publishing identities; other readers continue independently.
## Context opening

openView uses Rust-generated WebViewRequest and accepts only corresponding read-only queries for the five pages. It reads actual results within startup authorization, pins versions, and returns effective context with a session URL. Roots do not belong in context; mismatched explicit sources, unauthorized projects, and unsupported browser filters are rejected. Dates, language, objects, and versions remain independent; the browser uses its own pagination size. See the [CLI guide](../guides/cli.en.md#codex-skill).

## Background prices

The host owns one background price task and returns basic usage first. Inject `createNodeClient({automaticPrices:false})` to avoid the client’s existing blocking updates. Host `automaticPrices:false` disables automatic networking; `close()` cancels and waits boundedly for its price task. Rust retains eligibility/cooldown/version ownership. `restartCommand?` is copyable material for authenticated Sources only; the browser cannot execute it or supply arbitrary root paths.

## Verification

After building the client, run `corepack pnpm --filter @wombat/web test`. Tests use only temporary synthetic directories and local listeners, covering authentication, scope, progress, cancellation, concurrency, and static resources. `tests/timing.test.ts` uses a mock client to verify timing authorization, paging, expiry, sharing, and independent cancellation; it does not exercise the real core or browser. `tests/e2e/web.test.ts` verifies the existing real-core host path after a full build; installation checks reuse the same black-box case.

## Setup and collection

The header opens setup and Skill guidance without creating Codex tasks. `/api/setup` provides bounded read-only native discovery and Hook registration checks. `/api/collection` provides generated receipt/status/event operations; raw event ingestion is unavailable over HTTP. The host fixes source roots, checks selected projects and revalidates directory grants before reading either interface. Choosing a browser filter does not authorize edits.

The setup panel separates preferences, native discovery, registration/trust, actual receipt and the displayed history/view. It shows only the invocation name returned by discovery and preserves ambiguous instances. Event details are loaded on expansion, paged independently, and cancelled when closed or scoped differently. Chinese/English share these states; source identities and event kinds stay untranslated. Local preferences are changed through explicit displayed CLI commands, not by executing commands from Web. Closing Web does not establish that collection stopped.

The authenticated `/api/monitor` route uses the shared typed client. Checks accept only snapshots observed by this host, validate selected projects and source identities, and recheck grants before evaluating. Web does not accept source-root paths in monitor requests. Statistical panels use the existing usage routes. [Core monitoring](core.en.md) owns storage and notification rules.
