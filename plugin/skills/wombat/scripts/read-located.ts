import type { ConversationRequest, ConversationResult } from "@wombat/client";

/** Host orchestration only. The CLI/core still validates each original contract. */
export async function readLocated(
  call: (request: {
    method: "conversation";
    params: ConversationRequest;
  }) => Promise<ConversationResult>,
  request: ConversationRequest,
  maxBytes = 8192,
  maxTargets = 3,
  maxLocationPages = 1,
): Promise<readonly [ConversationResult, ConversationResult | null]> {
  if (!["locate", "search"].includes(request.action))
    throw new Error("Use a locate or search request");
  if (
    !Number.isSafeInteger(maxBytes) ||
    maxBytes < 1 ||
    maxBytes > 262144 ||
    !Number.isSafeInteger(maxTargets) ||
    maxTargets < 1 ||
    maxTargets > 20 ||
    !Number.isSafeInteger(maxLocationPages) || maxLocationPages < 1 || maxLocationPages > 20
  )
    throw new Error("Invalid evidence read budget");
  let locations = await call({
    method: "conversation",
    params: {
      ...request,
      limit: Math.min(request.limit ?? maxTargets, maxTargets),
    },
  });
  for (let page = 1; page < maxLocationPages && !locations.items.some(i => i.hasTextLocator)
    && !locations.scanComplete && locations.nextCursor; page++) {
    const cursor = locations.nextCursor;
    locations = await call({method:"conversation",params:{...request,
      ...(locations.sourceScope ? {sourceScope:locations.sourceScope} : {}),
      limit:Math.min(request.limit ?? maxTargets,maxTargets),cursor}});
    if(!locations.scanComplete && locations.nextCursor===cursor)throw new Error("Location cursor made no progress");
  }
  const targets = locations.items
    .filter((item) => item.hasTextLocator)
    .slice(0, maxTargets)
    .map((item) => ({ evidence: item.evidence, ...(item.readCursor ? {cursor:item.readCursor} : {}), maxBytes }));
  if (!targets.length) return [locations, null];
  const text = await call({
    method: "conversation",
    params: {
      action: "read",
      roots: request.roots,
      selection: request.selection,
      ...(locations.sourceScope ? {sourceScope:locations.sourceScope} : {}),
      maxScanBytes: request.maxScanBytes,
      targets,
    },
  });
  // Keep both responses intact: location and content cursors have different bindings.
  return [locations, text];
}
