# Web frontend

English | [中文](README.zh-CN.md)

`@wombat/ui` contains the React, TypeScript, and Vite presentation code. `App` receives an injected `UsageClient`; the browser entry connects the HTTP client.

## Responsibilities

The UI shows usage, tasks, timing, configuration, checks, and account allowance. Rust supplies totals, grouping, prices, and business sorting. Views preserve unknown values and partial coverage.

## Development preview

From the repository root, run:

```sh
corepack pnpm --filter @wombat/ui preview
```

Open `http://127.0.0.1:5173/preview.html?page=threads&allTime=1`. The preview uses synthetic data through production components. It reads no source logs or native accounts.

## Read next

- [Web guide](../docs/guides/web.en.md): product startup and recovery.
- [Frontend development](../docs/development/frontend.en.md): pages, query lifecycle, preview scenarios, and verification limits.
- [Product language](../docs/i18n/product.en.md): locale and copy ownership.
