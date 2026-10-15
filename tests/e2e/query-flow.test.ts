import assert from "node:assert/strict";
import { spawnSync } from "node:child_process";
import { readFile } from "node:fs/promises";
import { test } from "node:test";
import type {
  AgentRequest,
  ConversationResult,
  LiveResult,
} from "@wombat/client";
import { readLocated } from "../../plugin/skills/wombat/scripts/read-located.ts";
import {
  withLocalProduct,
  row,
  measurement,
} from "../helpers/local-product.js";

test(
  "Skill batch template and consecutive evidence reads use real CLI and Web contracts",
  { timeout: 60000 },
  async () => {
    await withLocalProduct(async (f) => {
      const at = "2026-01-02T01:00:00Z";
      await f.write("focus", [
        row("session_meta", { id: "focus", cwd: f.project }, at),
        row("turn_context", { turn_id: "turn", model: "gpt-5.4" }, at),
        measurement("focus", "first", 1000, at),
        ...["verified evidence one", "verified evidence two"].map((message) =>
          row(
            "event_msg",
            {
              type: "agent_message",
              message,
              phase: "final_answer",
              turn_id: "turn",
            },
            at,
          ),
        ),
      ]);
      await f.write("other", [
        row("session_meta", { id: "other", cwd: f.project }, at),
        measurement("other", "second", 300, at),
      ]);
      await f.write("excluded", [
        row("session_meta", { id: "excluded", cwd: f.project + "-other" }, at),
        measurement("excluded", "third", 9000, at),
      ]);
      const invoke = (request: AgentRequest) => {
        const run = spawnSync(process.execPath, [f.entry, "call"], {
          input: JSON.stringify(request),
          encoding: "utf8",
          timeout: 10000,
          maxBuffer: 2 * 1024 * 1024,
        });
        assert.ifError(run.error);
        assert([0, 2].includes(run.status ?? -1), run.stdout + run.stderr);
        return JSON.parse(run.stdout);
      };
      const prose = await readFile(
        "plugin/skills/wombat/references/query-flow.md",
        "utf8",
      );
      const template = JSON.parse(prose.match(/```json\n([^]*?)\n```/)![1]);
      template.params.query.scope.project = f.project;
      for (const q of template.params.also) q.scope.project = f.project;
      await f.client.live!({
        query: { ...template.params.query, roots: [f.root] },
        mode: "fresh",
      });
      const batch: LiveResult = invoke(template);
      assert.equal(batch.result.summary.tokens.total, 1300);
      assert(batch.additional);
      assert.equal(batch.additional.length, 2);
      assert(
        batch.additional.every(
          (r) =>
            r.snapshotRef.snapshotId === batch.result.snapshotRef.snapshotId,
        ),
      );
      const selected = batch.additional[1].items[0];
      assert.equal(selected.kind, "thread");
      if (selected.kind !== "thread") assert.fail("Expected task");
      assert.equal(selected.upstreamId, "focus");
      const turns: LiveResult = invoke({
        method: "usage",
        params: {
          query: {
            action: "turns",
            snapshotId: batch.result.snapshotRef.snapshotId,
            threadId: selected.id,
            scope: template.params.query.scope,
            matchedOnly: true,
            compact: true,
            limit: 1,
          },
          mode: "cached",
        },
      });
      const turn = turns.result.items[0];
      assert.equal(turn.kind, "turn");
      const request = {
        action: "locate" as const,
        snapshotId: batch.result.snapshotRef.snapshotId,
        selection: {
          since: "2026-01-02",
          until: "2026-01-03",
          timezone: "Asia/Shanghai",
          projects: [f.project],
          threads: [selected.id],
          turns: [turn.id],
          origins: ["assistant_visible" as const],
        },
        limit: 1,
      };
      const calls: AgentRequest[] = [];
      const [locations, text] = await readLocated(
        async (r) => {
          calls.push(r);
          const result: ConversationResult = invoke(r);
          return result;
        },
        request,
        12,
        1,
      );
      assert.equal(calls.length, 2);
      assert(locations.nextCursor);
      assert(text);
      assert.equal(text.snapshotId, batch.result.snapshotRef.snapshotId);
      assert.equal(text.fragments[0].text, "verified evi");
      assert(text.fragments[0].nextCursor);
      assert(text.sourceBytesRead <= 64);
      const browser = await f.browser();
      await browser.live!({ query: template.params.query, mode: "cached" });
      const [, webText] = await readLocated(
        (r) => browser.conversation!(r.params),
        request,
        12,
        1,
      );
      assert.deepEqual(webText?.fragments, text.fragments);
    });
  },
);
