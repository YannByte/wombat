import assert from "node:assert/strict";
import { test } from "node:test";
import type { ConversationRequest, ConversationResult } from "@wombat/client";
import { readLocated } from "../plugin/skills/wombat/scripts/read-located.ts";

const request: ConversationRequest = {
  action: "search",
  snapshotId: "fixed",
  query: "needle",
  roots: ["/synthetic"],
  selection: {
    threads: ["task"],
    turns: ["turn"],
    since: "2026-01-02",
    until: "2026-01-03",
    timezone: "Asia/Shanghai",
  },
  cursor: "location-cursor",
  limit: 7,
};
function result(): ConversationResult {
  return {
    outputVersion: 1,
    action: "locate",
    snapshotId: "fixed",
    selection: request.selection!,
    items: [],
    fragments: [],
    scanComplete: false,
    partial: true,
    limits: ["scan_budget"],
    nextCursor: "next-location",
    scannedEvents: 2,
    sourceBytesRead: 0,
  };
}

test("evidence composition preserves both responses, original filters and separate continuation", async () => {
  const locations = result();
  for (let i = 0; i < 5; i++)
    locations.items.push({
      evidence: {
        eventId: String(i),
        sourceInstanceId: "source",
        eventOffset: i,
      },
      collectedAt: "2026-01-02T00:00:00Z",
      kind: "message",
      hasTextLocator: i > 0,
      contentPartial: false,
    });
  const text = {
    ...result(),
    action: "read" as const,
    fragments: [
      {
        evidence: locations.items[1].evidence,
        text: "needle",
        nextCursor: "next-content",
        locations: [],
        context: [],
        partial: true,
      },
    ],
  };
  const calls: ConversationRequest[] = [];
  const before = JSON.stringify(request);
  const responses = await readLocated(
    async (r) => {
      calls.push(r.params);
      return calls.length === 1 ? locations : text;
    },
    request,
    64,
    2,
  );
  assert.equal(responses[0], locations);
  assert.equal(responses[1], text);
  assert.equal(responses[0].nextCursor, "next-location");
  assert.equal(responses[1].fragments[0].nextCursor, "next-content");
  assert.equal(calls.length, 2);
  assert.equal(calls[0].limit, 2);
  assert.equal(calls[0].query, "needle");
  assert.equal(calls[0].cursor, request.cursor);
  assert.deepEqual(calls[1], {
    action: "read",
    snapshotId: "fixed",
    roots: request.roots,
    selection: request.selection,
    maxScanBytes: undefined,
    targets: locations.items
      .slice(1, 3)
      .map((i) => ({ evidence: i.evidence, maxBytes: 64 })),
  });
  assert.equal(JSON.stringify(request), before);
});

test("empty budgeted location remains incomplete and makes no read or hidden continuation", async () => {
  let calls = 0;
  const locations = result();
  const responses = await readLocated(async () => {
    calls++;
    return locations;
  }, request);
  assert.equal(calls, 1);
  assert.equal(responses[0], locations);
  assert.equal(responses[1], null);
  assert.equal(responses[0].scanComplete, false);
  assert.equal(responses[0].nextCursor, "next-location");
});

test("expiry, cancellation and changed versions propagate without retries", async () => {
  for (const code of ["VIEW_EXPIRED", "CANCELLED", "SOURCE_CHANGED"]) {
    let calls = 0;
    const failure = Object.assign(new Error(code), { code });
    await assert.rejects(
      readLocated(async () => {
        calls++;
        throw failure;
      }, request),
      (e) => e === failure,
    );
    assert.equal(calls, 1);
  }
  await assert.rejects(
    readLocated(
      async () => ({ ...result(), snapshotId: "replacement" }),
      request,
    ),
    /view changed/,
  );
  const locations = result();
  locations.items.push({
    evidence: { eventId: "one", sourceInstanceId: "source", eventOffset: 0 },
    collectedAt: "",
    kind: "message",
    hasTextLocator: true,
    contentPartial: false,
  });
  let calls = 0;
  const failure = new Error("SOURCE_CHANGED");
  await assert.rejects(
    readLocated(async () => {
      if (++calls === 1) return locations;
      throw failure;
    }, request),
    (e) => e === failure,
  );
  assert.equal(calls, 2);
});

test("invalid action or byte and target budgets fail before dispatch", async () => {
  const call = async () => {
    assert.fail("Invalid workflow dispatched");
  };
  await assert.rejects(readLocated(call, { ...request, action: "read" }));
  for (const [bytes, targets] of [
    [0, 1],
    [262145, 1],
    [64, 0],
    [64, 21],
    [NaN, 1],
    [64, 1.5],
  ])
    await assert.rejects(readLocated(call, request, bytes, targets));
});
