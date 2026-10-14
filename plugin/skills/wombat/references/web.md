# Show Web details

Use charts for trends/contributor comparisons, task/turn detail for long sequences, and configuration/suggestion evidence for complex findings. Give the conclusion in the conversation. Tell the user what to examine in the browser. Browser failure still leaves a useful conversational answer.

Before context opening, read `web --help`. If context opening is available, supply `web --context FILE --json` with a generated-contract context. Include the page, corresponding usage/configuration/optimization request, selected IDs and returned versions. Supply both date boundaries or `allTime`.

Unsupported browser filters are rejected rather than silently widened. The browser may use its own pagination size. Source/project roots belong to startup flags, not arbitrary URL paths. The host validates scope/version before returning its effective context and session URL.

Never build a fake same-version link or use an expired connection.

Without context support, `web --json` can open the existing local page. Tell the user that exact object/version navigation was not preserved. Do not claim integration from an address alone. Keep exclusive CLI date boundaries when forming context. The product converts display dates.

Keep Web language preference by default. Add `--lang zh|en` only for an explicit page-language request. Language changes do not change date timezone, scope, targets or authorization. Manage this invocation's service lifetime.

Stopping this service must not stop other shared readers. Use the returned session link locally. Do not publish its connection token. Browser selection is not automatically conversational selection.

Before an edit to a changed target, ask the user to confirm the target.

For a selected task’s turns, write this shape with returned values. `turnId` belongs only to a `steps` request, not `turns`:

```json
{"page":"threads","usage":{"action":"turns","snapshotId":"RETURNED_SNAPSHOT_ID","threadId":"RETURNED_THREAD_ID","scope":{"allTime":true,"timezone":"USER_TIMEZONE","project":"SELECTED_PROJECT"},"sort":"tokens"}}
```

For a configuration item, use its own read view and authorized current project:

```json
{"page":"instructions","configuration":{"action":"detail","readView":"RETURNED_CONFIG_VIEW","itemId":"RETURNED_ITEM_ID","scope":{"project":"AUTHORIZED_PROJECT"}}}
```

For a reviewed suggestion, carry both configuration and decision versions:

```json
{"page":"optimize","optimization":{"action":"detail","readView":"RETURNED_CONFIG_VIEW","decisionRevision":"RETURNED_DECISION_REVISION","project":"AUTHORIZED_PROJECT","suggestionId":"RETURNED_SUGGESTION_ID"}}
```

Replace placeholders with returned values. For a dated scope, keep both boundaries. Keep root paths in the Web startup flags. Read the host’s returned effective context before claiming the browser matches. Do not use the illustrative placeholder strings as real IDs or invent extra fields.
