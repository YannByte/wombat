# Local Web host

English | [中文](README.zh-CN.md)

`@wombat/web` connects authenticated local HTTP requests to `UsageClient`. The CLI supplies a Node client and manages host startup and shutdown.

## Public entry

`startWebHost(options)` returns the browser URL, origin, asynchronous `close()`, and restricted `openView(request)`.

## Limits

The host listens on loopback only. It does not support remote or hosted deployment. Requests require authentication and authorized source scope. A restart requires a new browser link.

## Read next

- [Web guide](../docs/guides/web.en.md): opening pages and recovering failed reads.
- [Web host reference](../docs/reference/web-host.en.md): authentication, grants, request limits, and cancellation.
- [Architecture](../docs/development/architecture.en.md): module boundaries.
