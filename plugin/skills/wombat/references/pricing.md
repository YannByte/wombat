# Estimated cost and model prices

For “how much did it cost”, query the selected usage. Use the core's returned valuation. Give the currency and dates/project. Identify official-standard API-equivalent cost as an estimate, not a subscription invoice. Keep source-reported amounts separate. Do not add them to the equivalent estimate or allocate a fee to a tool operation.

A null total is incomplete, not zero. Show the known subtotal as a subtotal. Identify unpriced models or usage. Amounts are decimal strings. Use the returned totals instead of adding rounded display values or recreating pricing formulas. For “which models cannot be priced”, examine model-level usage and its returned pricing reasons, not the catalog alone.

Read the catalog only when price availability or freshness answers the question. This Agent request is offline:

```json
{"method":"prices","params":{"action":"status"}}
```

Live usage can do an automatic check of the fixed official price source for eligible missing rates. Examine `priceUpdate` before retrying. Offline requests require `WOMBAT_AUTO_PRICES=0`. Cached and fixed usage never trigger automatic price downloads.

An explicit `prices update` accesses the network and changes the local catalog. Use this command only when authorized. No credentials, local usage or custom URLs are needed. On failure, keep known amounts.

Show the missing portion. Do not repeatedly download or invent a model alias/rate.

Updated prices apply on the next live synchronization. Old snapshots keep their amounts. If the user requests a new estimate, read the original scope again. Tell the user about the new version.

Do not claim an old fixed view changed. Read catalog details or the output Schema only when needed. Do not return every model price for an ordinary cost question.
