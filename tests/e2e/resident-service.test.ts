import assert from "node:assert/strict";
import { test } from "node:test";
import { setTimeout as delay } from "node:timers/promises";
import {
  appendFile,
  mkdir,
  readFile,
  rename,
  writeFile,
} from "node:fs/promises";
import path from "node:path";
import { DatabaseSync } from "node:sqlite";
import {
  withLocalProduct,
  row,
  measurement,
} from "../helpers/local-product.js";

test(
  "resident indexing updates without readers, pauses, resumes, restores and preserves fixed reads",
  { timeout: 180_000 },
  async (t) => {
    await withLocalProduct(async (f) => {
      const at = "2026-10-15T00:00:00Z";
      await f.write("task", [
        row("session_meta", { id: "task", cwd: f.project }, at),
        row("turn_context", { turn_id: "turn", model: "gpt-5.4" }, at),
        measurement("task", "first", 100, at),
      ]);
      const started = await f.client.service!({ action: "start" });
      assert(started.running && started.resident);
      const cached = () =>
        f.client.live!({
          query: { action: "usage", scope: { allTime: true } },
          mode: "cached",
        });
      const until = async (
        predicate: () => Promise<boolean>,
        timeout = 15_000,
      ) => {
        const deadline = Date.now() + timeout;
        while (Date.now() < deadline) {
          if (await predicate()) return;
          await delay(100);
        }
        assert.fail(
          "Resident service did not publish the expected committed state",
        );
      };
      await until(async () => {
        const s = await f.client.service!({ action: "status" });
        return s.ready === true && !s.preparing;
      });
      const first = await cached();
      assert.equal(first.result.summary.tokens.total, 100);
      // Cross the old idle-exit boundary with no clients connected.
      await delay(16_000);
      assert.equal(
        (await f.client.service!({ action: "status" })).instanceId,
        started.instanceId,
      );
      const file = path.join(f.root, "sessions/task.jsonl");
      await f.append("task", [measurement("task", "second", 200, at)]);
      // Waiting here is intentional: no live read is allowed to trigger synchronization.
      await until(async () => {
        const s = await f.client.service!({ action: "status" });
        return (
          !!s.revision &&
          s.revision !== first.result.snapshotRef.snapshotId &&
          !s.preparing
        );
      });
      const second = await cached();
      assert.equal(second.result.summary.tokens.total, 300);
      const fixed = await f.client.live!({
        query: {
          action: "usage",
          snapshotId: first.result.snapshotRef.snapshotId,
          scope: { allTime: true },
        },
        mode: "cached",
      });
      assert.equal(fixed.result.summary.tokens.total, 100);
      const settings = path.join(f.data, "user-v1/service.json");
      await writeFile(settings, JSON.stringify({ version: 99, paused: false }));
      await assert.rejects(f.client.service!({ action: "pause" }), {
        code: "UNSUPPORTED_VERSION",
      });
      assert.equal(JSON.parse(await readFile(settings, "utf8")).version, 99);
      await writeFile(settings, JSON.stringify({ version: 1, paused: false }));
      for (let n = 0; n < 5; n++) {
        await Promise.all([
          f.client.service!({ action: "pause" }),
          f.client.service!({ action: "resume" }),
        ]);
        assert.equal(
          (await f.client.service!({ action: "status" })).paused,
          JSON.parse(await readFile(settings, "utf8")).paused,
        );
      }
      await f.client.service!({ action: "resume" });
      t.diagnostic(
        "Background append and fixed reads passed; checking pause and incomplete tails",
      );
      const browser = await f.browser();
      assert((await browser.service!({ action: "status" })).running);
      await assert.rejects(browser.service!({ action: "pause" }), {
        code: "EACCES",
      });
      const paused = await f.client.service!({ action: "pause" });
      assert(paused.paused);
      await f.append("task", [measurement("task", "third", 300, at)]);
      const unchanged = await cached();
      assert.equal(unchanged.result.summary.tokens.total, 300);
      assert.equal(unchanged.freshness.status, "paused");
      await assert.rejects(
        f.client.live!({ query: { action: "usage" }, mode: "fresh" }),
        {
          code: "INDEX_PAUSED",
        },
      );
      await f.client.service!({ action: "resume" });
      await until(async () => {
        const s = await f.client.service!({ action: "status" });
        return (
          s.revision !== second.result.snapshotRef.snapshotId && !s.preparing
        );
      });
      assert.equal((await cached()).result.summary.tokens.total, 600);
      // A split line cannot publish a half observation.
      const tail = JSON.stringify(measurement("task", "fourth", 400, at));
      await appendFile(file, tail.slice(0, tail.length / 2));
      await f.client.live!({
        query: { action: "usage" },
        mode: "fresh",
        waitMs: 15000,
      });
      assert.equal((await cached()).result.summary.tokens.total, 600);
      await appendFile(file, tail.slice(tail.length / 2) + "\n");
      await until(async () => {
        const result = await cached();
        return result.result.summary.tokens.total === 1000;
      });
      await f.client.service!({ action: "pause" });
      t.diagnostic(
        "Incomplete-tail handling passed; checking restart while paused",
      );
      await f.stop();
      await f.start(true);
      await until(async () => {
        const s = await f.client.service!({ action: "status" });
        return s.paused && s.ready === true && !s.preparing;
      });
      const restored = await cached();
      assert.equal(restored.result.summary.tokens.total, 1000);
      assert.equal(restored.freshness.status, "paused");
      await f.client.service!({ action: "resume" });
      const replacement = file + ".next";
      await writeFile(replacement, await readFile(file));
      await rename(replacement, file);
      const verified = await f.client.live!({
        query: { action: "usage", scope: { allTime: true } },
        mode: "fresh",
        waitMs: 15000,
      });
      assert.equal(verified.result.summary.tokens.total, 1000);
      t.diagnostic(
        "Restart and replacement passed; checking caller source isolation",
      );
      const schema = JSON.parse(f.cli(["api", "--method", "service"]));
      assert(schema.methods.includes("service"));
      const cli = JSON.parse(f.cli(["service", "status", "--json"]));
      assert(cli.running && cli.resident);
      // The same data store can be used from a caller with a different source home.
      // The service must honor that caller's Rust-discovered scope.
      const alternate = path.join(f.dir, "alternate");
      await mkdir(path.join(alternate, "sessions"), { recursive: true });
      await writeFile(
        path.join(alternate, "sessions/alternate.jsonl"),
        [
          row("session_meta", { id: "alternate", cwd: f.project }, at),
          row("turn_context", { turn_id: "turn", model: "gpt-5.4" }, at),
          measurement("alternate", "only", 17, at),
        ]
          .map((value) => JSON.stringify(value) + "\n")
          .join(""),
      );
      const previous = process.env.CODEX_HOME;
      try {
        process.env.CODEX_HOME = alternate;
        const scoped = await f.client.live!({
          query: { action: "usage", scope: { allTime: true } },
          mode: "fresh",
          waitMs: 15000,
        });
        assert.equal(scoped.result.summary.tokens.total, 17);
      } finally {
        process.env.CODEX_HOME = previous;
      }
      t.diagnostic(
        "Source isolation passed; checking bounded shutdown and status without startup",
      );
      const writer = new DatabaseSync(
        path.join(f.data, "live-v2/index.sqlite"),
      );
      writer.exec("BEGIN IMMEDIATE");
      try {
        await f.append("task", [measurement("task", "during-shutdown", 2, at)]);
        await assert.rejects(
          f.client.live!({
            query: { action: "usage" },
            mode: "fresh",
            waitMs: 15000,
          }),
          { code: "INDEX_UNAVAILABLE" },
        );
        assert.equal((await cached()).result.summary.tokens.total, 1000);
      } finally {
        writer.exec("ROLLBACK");
        writer.close();
      }
      // Use source work to keep a real transaction active. An external writer
      // lock is deliberately rejected, so it cannot simulate a long transaction.
      // Observe real work through status; throughput is qualified by benchmarks.
      // Oversized fixtures can starve under Windows background I/O scheduling.
      const count = 5000;
      await f.append(
        "task",
        Array.from({ length: count }, (_, n) =>
          measurement("task", "shutdown-" + n, 2, at),
        ),
      );
      const refresh = f.client.live!({
        query: { action: "usage" },
        mode: "fresh",
        waitMs: 15000,
      }).then(
        (value) => value,
        (error) => error,
      );
      await until(
        async () =>
          (await f.client.service!({ action: "status" })).preparing === true,
      );
      const stopping = f.client.service!({ action: "stop" }).catch((error) => {
        assert.equal(error.code, "SERVICE_STOP_PENDING");
        return null;
      });
      await until(async () => {
        const s = await f.client.service!({ action: "status" });
        if (s.running) {
          assert.equal(s.instanceId, cli.instanceId);
          return s.stopping;
        }
        // A short transaction may finish before the next status request.
        return true;
      });
      const stopped = await stopping;
      await refresh;
      if (stopped) assert(!stopped.running);
      // A background transaction may exceed the control request's short wait.
      // Its pending response must preserve the listener until that work commits.
      await until(
        async () => !(await f.client.service!({ action: "status" })).running,
        120_000,
      );
      await f.start();
      assert.equal(
        (await cached()).result.summary.tokens.total,
        1002 + count * 2,
      );
      await f.client.service!({ action: "stop" });
    });
  },
);
