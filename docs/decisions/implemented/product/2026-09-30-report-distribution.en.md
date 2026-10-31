# Decision Note: Report distribution and full-range queries

[中文](2026-09-30-report-distribution.md) | English

Status: implemented

The [contract](../../../development/contracts.en.md) owns field semantics; the [frontend guide](../../../development/architecture.en.md) owns current Web navigation.

## Problem

Distribution bars, shares, and peaks must describe the entire filtered range. Aggregating the current page changes the scale during pagination. Row pagination can also split a period subtotal from its model rows. Report-linked threads must describe both selected-range and complete-thread usage without sorting whole threads by their selected fragments.

## Decision

The core calculates distribution metadata, token share, and costShare before pagination, shared by CLI/Web. Cost shares use the complete range's priced subtotal; unknown costs are not zero and a zero denominator returns absence. Unpriced tokens count unknown-price components per measurement without counting reasoning tokens again; missing component facts return absence.

Explicit usage presentation distribution or details pages by period, with details retaining every model/effort row in selected periods. Omitting presentation retains row pagination for row-oriented and candidate queries. matchedUsage and threadUsage preserve the selected fragment and complete-task usage separately; the current query contract defines sorting. The presentation entry owns metric, presentation, and navigation state and queries generated contracts without aggregating hidden pages.

## Alternatives considered

Aggregating the current page in the UI was considered, but cannot provide hidden-page maxima, tied peaks, or denominators, so metadata covers the entire range. Retaining row pagination for every query was also considered, but explicit details would split period groups. Optional presentation distinguishes pagination units to support different query purposes.

## Impact and verification

Rust DTOs generate TS/Schema and validators without new dependencies. Synthetic queries verify stable scales and shares across pages, intact detail groups, unpriced components, and full-thread associations. Retired terminal rendering tests do not establish current-entry acceptance.
