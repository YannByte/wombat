# Decision Note: Report defaults follow grouping granularity

[中文](2026-09-30-report-ranges.md) | English

Status: implemented

The [CLI guide](../../../guides/cli.en.md) owns current query ranges and the [frontend guide](../../../development/architecture.en.md) owns Web navigation. This note retains the rationale for shared date defaults without maintaining retired terminal behavior.

## Problem

The seven-day default applied to all groups. The terminal also copied response dates into requests, limiting weekly and monthly reports to the latest week.

## Decision

The shared core resolves daily, weekly, and monthly defaults into bounded windows ending today in the selected timezone; the [CLI guide](../../../guides/cli.en.md) owns exact ranges. Explicit dates and complete-thread or undated scopes take precedence. Resolved response dates must not permanently replace implicit requests with explicit conditions.

## Alternatives considered

Only the current week/month limits comparison with preceding periods; all history expands routine queries. We therefore use bounded windows of multiple periods. Computing dates only in the UI would diverge from the CLI, so the core owns date calculations.

## Impact and verification

The choice affects only default ranges for requests without dates, not collection or fixed snapshots. Verify timezone boundaries, explicit-date precedence, shared CLI queries, and current Web date intent; historical terminal tests do not establish current Web behavior.
