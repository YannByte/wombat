# Cost calculation

[中文](pricing.md) | English

The Rust core converts tokens using bundled or updated official-standard API rates. Accounting and calculations run locally; live queries automatically fetch prices when repairable missing rates are detected, without API keys. These amounts are standard API equivalents, not subscription invoices or spending reported by source logs. They exclude tool calls, storage, taxes, and regional surcharges.

## Catalog and sources

Bundled revision: `openai-standard-2026-10-12.1`; policy: `official-standard-api-equivalent-v1`; updated on 2026-10-12 to add GPT-6.1 Sol. Other models retain their previously verified rates. The [separate catalog](../../core/prices/openai-standard-2026-10-12.json) includes official links per entry. Snapshots retain the revision and SHA-256. Queries use saved component amounts rather than recalculating with new rates.

Units are USD per million tokens.

| Model | Uncached input | Cache read | Cache creation | Output | Official basis |
|---|---:|---:|---:|---:|---|
| gpt-5.3-codex | 1.75 | 0.175 | Not separately listed | 14 | [Model page](https://developers.openai.com/api/docs/models/gpt-5.3-codex) |
| gpt-5.4 | 2.5 | 0.25 | Not separately listed | 15 | [Model page](https://developers.openai.com/api/docs/models/gpt-5.4) |
| gpt-5.4-mini | 0.75 | 0.075 | Not separately listed | 4.5 | [Model page](https://developers.openai.com/api/docs/models/gpt-5.4-mini) |
| gpt-5.5 | 5 | 0.5 | Not separately listed | 30 | [Model page](https://developers.openai.com/api/docs/models/gpt-5.5) |
| gpt-5.6-sol | 4 | 0.4 | 5 | 20 | [Model page](https://developers.openai.com/api/docs/models/gpt-5.6-sol) |
| gpt-5.6-terra | 2 | 0.2 | 2.5 | 12 | [Model page](https://developers.openai.com/api/docs/models/gpt-5.6-terra) |
| gpt-6-sol | 2 | 0.2 | 2.5 | 10 | [Model page](https://developers.openai.com/api/docs/models/gpt-6-sol) |
| gpt-6.1-sol | 2 | 0.1 | 2.5 | 10 | [Model page](https://developers.openai.com/api/docs/models/gpt-6.1-sol) |
| gpt-6-astra | 10 | 1 | 12.5 | 50 | [Model page](https://developers.openai.com/api/docs/models/gpt-6-astra) |

For GPT-5.4, GPT-5.5, GPT-5.6 Sol/Terra, GPT-6 Sol/Astra, and GPT-6.1 Sol, request input above 272,000 tokens applies 2× input/cache-read and 1.5× output rates to the entire request. Where cache-creation prices exist, that rate also doubles. The threshold includes cached input; exactly 272,000 uses standard rates. Price each request before aggregation; neither daily/turn totals nor only the excess above the threshold determine the tier. Cumulative differences without a known request input size cannot use these conditional rates.

Explicit aliases are only the officially listed `gpt-5.4-2026-03-05`, `gpt-5.4-mini-2026-03-17`, and `gpt-5.5-2026-04-23`. There are no wildcard dates, substring matches, automatic third-party prefix removal, or similar-name guesses. Exact official names may map to this policy; an explicitly different model provider or API provider keeps cost unknown. Original names and matching basis are retained separately.

Checked pages for GPT-5.3-Codex, GPT-5.4, GPT-5.4 Mini, and GPT-5.5 do not list separate cache-creation rates. Zero creation tokens produce zero cost; nonzero creation retains known component subtotals while that component stays unknown. The checked GPT-5.6 Sol promotional rate is fixed in this catalog; an explicit update can create a later revision. `codex-auto-review` is a logged model name without a verified public standard API rate and remains unknown. Rates are not borrowed from another model, provider, or cache TTL.

<a id="联网更新价表"></a>

## Updating prices online

`wombat prices --json` returns the current catalog offline; `wombat prices update --json` explicitly updates online. The Web price page offers an explicit update action. Afterward, `wombat refresh` creates a snapshot with the new catalog; updating never changes old snapshots.

The fixed source is [OpenAI's official pricing Markdown](https://developers.openai.com/api/docs/pricing.md). This is a document endpoint, not a promised stable dedicated pricing API; `/v1/models` supplies no unit prices. Only standard text tables and Codex rows in dedicated tables are parsed. Batch, Flex, Fast, Ultrafast, multimodal, tool, and other dedicated models with future effective dates are excluded. The parser requires USD per million tokens, an explicit 272K long-context boundary, and complete supported-model coverage. Changes to format, units, thresholds, or required models return `PRICE_SOURCE_CHANGED` until adapted. Missing rates remain null; new models match exact names, while dated aliases retain only verified bundled mappings.

No API key is required and local logs are never sent. Node fetches only the fixed HTTPS document, disallows redirects, and enforces a 30-second timeout, 2 MiB limit, and cancellation. Rust validates, parses decimals, versions, persists, and prices. With an environment proxy on Node 26, set `NODE_USE_ENV_PROXY=1` and retain existing `HTTPS_PROXY`/`HTTP_PROXY`. Automatic downloads time out after 5 seconds; cached and fixed-snapshot queries, and queries with `WOMBAT_AUTO_PRICES=0`, do not trigger automatic network requests.

Downloads live under `prices/` in the product data directory: content-hash-named history plus an atomically replaced `active.json`. The interface exposes document SHA-256, fetch/validation time, and normalized catalog hash. `openai-standard-live-v1-<hash>` is independent of the bundled revision; repeating an identical download does not rewrite the revision or check time. Fetch/parse failure retains the previous catalog. Without an update, use the bundled table. Corrupt caches report errors and can be recovered by updating again. Catalog history is not automatically removed; this is neither an account invoice nor a historical effective-date service.

Live CLI/Web queries use the same download and validation path when nonzero usage for an identified OpenAI model lacks a rate. Missing model names, third-party providers, conflicting tokens, and unknown request lengths alone do not trigger it. Detection uses the full matching summary, not just the current page. After an update, the original filters are queried once more against fresh usage; fixed snapshots are never repriced. Failures retain usage and the previous catalog. JSON `priceUpdate` exposes status, attempt/retry times, and an error code; text and Web views show the same state.

A file lock and 60-second lease coalesce concurrent downloads within one product data directory. Failures retry after 15 minutes; successful checks retry after 24 hours. No permanent background process is added. Cooldown state persists in `prices/automatic.json` across process restarts. Manual `prices update` can retry at any time. Automatic checks cannot guarantee that every rate is public and never bypass format validation. An explicit refresh that resolves prices saves another revision; the earlier saved revision remains immutable.

## Categories and precision

Uncached input, cache read, cache creation, and output are non-overlapping categories. Reasoning tokens are already included in output and incur no second charge. Source `reportedCost` is stored separately, excluded from standard conversion, and not automatically treated as an invoice amount.

Amounts use pinned [rust_decimal 1.40.0](https://docs.rs/crate/rust_decimal/1.40.0), under MIT, with only standard-library features; runtime dependencies are arrayvec and num-traits. DTOs use decimal strings, Rust aggregates, and the interface handles display precision only. Measurement amounts are not rounded individually, preserving accumulated tiny costs.

- `priced`: every applicable category has a cost; `cost` is complete.
- `partial`: some nonzero usage can be priced; `cost` is empty and `knownCost` is the known subtotal.
- `unknown`: usage cost cannot be established; a zero known subtotal does not mean free.
- Explicit zeros in all four categories establish zero cost; missing tokens cannot become zero.

Aggregation retains the same policy and catalog basis. Only the current policy is accepted; aggregation rejects unknown policies. Invalid numbers, negative amounts, overflow, and conflicting token categories cannot become priced costs.

## Independent verification

[Pricing tests](../../core/src/pricing/tests.rs) use hand-calculated synthetic data for ordinary components, long-context boundaries and whole-request rates, reasoning inclusion, per-request aggregation, unknown cumulative-difference conditions, missing versus zero, unknown cache creation, exact aliases, provider isolation, tiny-cost accumulation, unknown-policy rejection, overflow, and replay of persisted costs. Tests never download catalogs, install/run ccusage, or read real conversations.
