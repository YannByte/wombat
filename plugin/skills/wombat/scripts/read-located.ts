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
): Promise<readonly [ConversationResult, ConversationResult | null]> {
  if (!["locate", "search"].includes(request.action))
    throw new Error("Use a locate or search request");
  if (
    !Number.isSafeInteger(maxBytes) ||
    maxBytes < 1 ||
    maxBytes > 262144 ||
    !Number.isSafeInteger(maxTargets) ||
    maxTargets < 1 ||
    maxTargets > 20
  )
    throw new Error("Invalid evidence read budget");
  const locations = await call({
    method: "conversation",
    params: {
      ...request,
      limit: Math.min(request.limit ?? maxTargets, maxTargets),
    },
  });
  if (locations.snapshotId !== request.snapshotId)
    throw new Error("Conversation view changed");
  const targets = locations.items
    .filter((item) => item.hasTextLocator)
    .slice(0, maxTargets)
    .map((item) => ({ evidence: item.evidence, maxBytes }));
  if (!targets.length) return [locations, null];
  const text = await call({
    method: "conversation",
    params: {
      action: "read",
      snapshotId: request.snapshotId,
      roots: request.roots,
      selection: request.selection,
      maxScanBytes: request.maxScanBytes,
      targets,
    },
  });
  if (text.snapshotId !== request.snapshotId)
    throw new Error("Conversation view changed");
  // Keep both responses intact: location and content cursors have different bindings.
  return [locations, text];
}
