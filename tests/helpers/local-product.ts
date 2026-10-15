import assert from "node:assert/strict";
import { spawn, spawnSync, type ChildProcess } from "node:child_process";
import { once } from "node:events";
import { setTimeout as delay } from "node:timers/promises";
import { realpathSync } from "node:fs";
import { mkdtemp, mkdir, rm, writeFile, appendFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import path from "node:path";
import { createNodeClient } from "@wombat/client/node";
import { createHttpClient } from "@wombat/client/http";
import { startWebHost } from "@wombat/web";

/** Real assembled entries, with synthetic sources and a private data home. */
export async function withLocalProduct(
  run: (fixture: Awaited<ReturnType<typeof localProduct>>) => Promise<void>,
) {
  const fixture = await localProduct();
  try {
    await run(fixture);
  } finally {
    await fixture.close();
  }
}

async function localProduct() {
  const dir = realpathSync.native(
    await mkdtemp(path.join(tmpdir(), "wombat-acceptance-")),
  );
  const root = path.join(dir, "source"),
    project = path.join(dir, "project"),
    data = path.join(dir, "data");
  await mkdir(path.join(root, "sessions"), { recursive: true });
  await mkdir(project);
  const binary = path.resolve(
    "dist",
    process.platform === "win32" ? "wombat-core.exe" : "wombat-core",
  );
  const entry = path.resolve("dist/wombat.js");
  const saved = {
    WOMBAT_DATA_HOME: process.env.WOMBAT_DATA_HOME,
    CODEX_HOME: process.env.CODEX_HOME,
    WOMBAT_AUTO_PRICES: process.env.WOMBAT_AUTO_PRICES,
    WOMBAT_CORE_BIN: process.env.WOMBAT_CORE_BIN,
  };
  Object.assign(process.env, {
    WOMBAT_DATA_HOME: data,
    CODEX_HOME: root,
    WOMBAT_AUTO_PRICES: "0",
    WOMBAT_CORE_BIN: binary,
  });
  let service: ChildProcess | undefined;
  let host: Awaited<ReturnType<typeof startWebHost>> | undefined;
  const client = createNodeClient({
    binaryPath: binary,
    automaticPrices: false,
  });
  const stop = async () => {
    if (!service || service.exitCode !== null || service.signalCode !== null)
      return;
    const exiting = once(service, "exit");
    const force = setTimeout(() => service?.kill("SIGKILL"), 5_000);
    try {
      service.kill("SIGTERM");
      await exiting;
    } finally {
      clearTimeout(force);
    }
  };
  const start = async (resident = false) => {
    // Listener removal can precede the final checkpoint and process lock release.
    // Reuse only this fixture's confirmed child, never a matching process name.
    if (service && service.exitCode === null && service.signalCode === null) {
      await once(service, "exit", { signal: AbortSignal.timeout(10000) });
    }
    service = spawn(binary, [resident ? "--daemon" : "--serve-usage"], {
      env: process.env,
      stdio: "ignore",
    });
    await once(service, "spawn");
    const deadline = Date.now() + 5000;
    while (Date.now() < deadline) {
      assert.equal(service.exitCode, null, "Owned core exited during startup");
      if ((await client.service!({ action: "status" })).running) return;
      await delay(25);
    }
    assert.fail("Owned service listener did not become available");
  };
  const close = async () => {
    try {
      await host?.close();
    } finally {
      try {
        await stop();
      } finally {
        for (const [key, value] of Object.entries(saved)) {
          if (value === undefined) delete process.env[key];
          else process.env[key] = value;
        }
        await rm(dir, { recursive: true, force: true });
      }
    }
  };
  const rows = (values: unknown[]) =>
    values.map((value) => JSON.stringify(value) + "\n").join("");
  try {
    await start();
  } catch (error) {
    await close();
    throw error;
  }
  return {
    dir,
    root,
    project,
    data,
    entry,
    client,
    stop,
    start,
    close,
    write: (id: string, values: unknown[]) =>
      writeFile(path.join(root, "sessions", id + ".jsonl"), rows(values)),
    append: (id: string, values: unknown[]) =>
      appendFile(path.join(root, "sessions", id + ".jsonl"), rows(values)),
    cli(args: string[], input = "", status = 0) {
      const result = spawnSync(process.execPath, [entry, ...args], {
        env: process.env,
        input,
        encoding: "utf8",
        timeout: 15_000,
        maxBuffer: 2 * 1024 * 1024,
      });
      assert.ifError(result.error);
      assert.equal(result.status, status, result.stderr + result.stdout);
      return result.stdout;
    },
    async browser() {
      host ??= await startWebHost({
        client,
        roots: [root],
        projectRoots: [project],
        assets: path.resolve("dist/web"),
        automaticPrices: false,
      });
      const origin = host.origin,
        token = new URLSearchParams(new URL(host.url).hash.slice(1)).get(
          "token",
        )!;
      return createHttpClient({
        origin,
        token,
        fetch: (input, init) =>
          fetch(input, {
            ...init,
            headers: { ...init?.headers, Origin: origin },
          }),
      });
    },
  };
}

export const row = (type: string, payload: unknown, timestamp: string) => ({
  type,
  payload,
  timestamp,
});
export function measurement(
  thread: string | null,
  response: string,
  total: number | null,
  at: string,
) {
  return row(
    "event_msg",
    {
      type: "token_usage_record",
      ...(thread ? { thread_id: thread, turn_id: "turn" } : {}),
      response_id: response,
      usage:
        total === null
          ? {}
          : {
              input_tokens: total,
              cached_input_tokens: 0,
              cache_write_input_tokens: 0,
              output_tokens: 0,
              reasoning_output_tokens: 0,
              total_tokens: total,
            },
    },
    at,
  );
}
