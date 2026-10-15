import { Readable } from "node:stream";
import {
  CoreError,
  validateAgentRequest,
  type AgentRequest,
  type AgentErrorOutput,
  type UsageClient,
  type QueryOptions,
} from "@wombat/client";
import { createNodeClient } from "@wombat/client/node";
import { t } from "@wombat/client/locale";
import { configureLanguage } from "./locale.js";
import { resultExitCode } from "./exit-codes.js";
import { timingExitCode } from "./timing-format.js";
import {
  configExitCode,
  optimizeExitCode,
  accountExitCode,
  directoriesExitCode,
  handoffExitCode,
  setupExitCode,
  collectionExitCode,
} from "./exit-codes.js";

import { INPUT_BYTES, INPUT_TIMEOUT_MS, OUTPUT_BYTES } from "./agent-limits.js";
const invalid = () =>
  new CoreError("INVALID_ARGUMENT", "Invalid Agent request");
export function agentErrorOutput(
  error: unknown,
  cancelled = false,
): AgentErrorOutput {
  const code = cancelled
    ? "CANCELLED"
    : error instanceof CoreError && /^[A-Z][A-Z0-9_]{0,63}$/.test(error.code)
      ? error.code
      : "INTERNAL_ERROR";
  const recovery: AgentErrorOutput["error"]["recovery"] =
    code === "INVALID_ARGUMENT"
      ? "read_schema"
      : ["VIEW_EXPIRED", "SOURCE_CHANGED"].includes(code)
        ? "reacquire_view"
        : ["RESOURCE_LIMIT", "OUTPUT_LIMIT", "INPUT_LIMIT"].includes(code)
          ? "narrow_query"
          : code === "SYNC_PENDING"
            ? "retry_same_scope"
            : [
                  "CODEX_UNAVAILABLE",
                  "CORE_UNAVAILABLE",
                  "SKILL_RUNTIME_MISMATCH",
                  "SKILL_RESOURCE_CHANGED",
                ].includes(code)
              ? "check_setup"
              : code === "CANCELLED"
                ? "none"
                : "inspect_state";
  const message = t(
    code === "INVALID_ARGUMENT"
      ? "cli.agent.invalid"
      : recovery === "narrow_query"
        ? "cli.agent.resource"
        : code === "CANCELLED"
          ? "common.cancelled"
          : "cli.agent.failure",
    { code },
  );
  return { outputVersion: 1, error: { code, message, recovery } };
}
interface AgentHost {
  stdin?: Readable;
  stdout?: (text: string) => void;
  signal?: AbortSignal;
  createClient?: () => Partial<UsageClient>;
}
function readRequest(input: Readable, signal: AbortSignal): Promise<unknown> {
  return new Promise((resolve, reject) => {
    const chunks: Buffer[] = [];
    let bytes = 0,
      finished = false;
    const finish = (error?: unknown) => {
      if (finished) return;
      finished = true;
      clearTimeout(timer);
      input.off("data", data);
      input.off("end", end);
      input.off("error", fail);
      input.pause();
      signal.removeEventListener("abort", abort);
      if (error) {
        reject(error);
        return;
      }
      try {
        resolve(
          JSON.parse(
            new TextDecoder("utf-8", { fatal: true }).decode(
              Buffer.concat(chunks),
            ),
          ),
        );
      } catch {
        reject(invalid());
      }
    };
    const data = (chunk: Buffer | string) => {
      const buffer = Buffer.isBuffer(chunk) ? chunk : Buffer.from(chunk);
      bytes += buffer.length;
      if (bytes > INPUT_BYTES) {
        finish(new CoreError("RESOURCE_LIMIT", "Agent input budget exceeded"));
        return;
      }
      chunks.push(buffer);
    };
    const end = () => finish(),
      fail = () =>
        finish(new CoreError("TRANSPORT_ERROR", "Agent input unavailable"));
    const abort = () => finish(new CoreError("CANCELLED", "Cancelled"));
    const timer = setTimeout(
      () => finish(new CoreError("TIMEOUT", "Agent input timeout")),
      INPUT_TIMEOUT_MS,
    );
    input.on("data", data);
    input.once("end", end);
    input.once("error", fail);
    signal.addEventListener("abort", abort, { once: true });
    if (signal.aborted) abort();
  });
}
async function call(
  request: AgentRequest,
  client: Partial<UsageClient>,
  options: QueryOptions,
): Promise<{ result: unknown; exitCode: number }> {
  const unavailable = (): never => {
    throw new CoreError("CORE_UNAVAILABLE", "Method unavailable");
  };
  switch (request.method) {
    case "service":
      return {
        result: await (client.service?.(request.params, options) ??
          unavailable()),
        exitCode: 0,
      };
    case "usage": {
      const r = await (client.live?.(request.params, options) ?? unavailable());
      return { result: r, exitCode: resultExitCode(r.result) };
    }
    case "snapshot": {
      const r = await (client.query?.(request.params, options) ??
        unavailable());
      return { result: r, exitCode: resultExitCode(r) };
    }
    case "config": {
      const r = await (client.config?.(request.params, options) ??
        unavailable());
      return { result: r, exitCode: configExitCode(r) };
    }
    case "optimize": {
      const r = await (client.optimize?.(request.params, options) ??
        unavailable());
      return { result: r, exitCode: optimizeExitCode(r) };
    }
    case "conversation": {
      const r = await (client.conversation?.(request.params, options) ??
        unavailable());
      return { result: r, exitCode: r.partial ? 2 : 0 };
    }
    case "timing": {
      const r = await (client.timing?.(request.params, options) ??
        unavailable());
      return { result: r, exitCode: timingExitCode(r) };
    }
    case "setup": {
      const r = await (client.setup?.(request.params, options) ??
        unavailable());
      return { result: r, exitCode: setupExitCode(r) };
    }
    case "account": {
      const r = await (client.account?.(request.params, options) ??
        unavailable());
      return { result: r, exitCode: accountExitCode(r) };
    }
    case "directories": {
      const r = await (client.directories?.(request.params, options) ??
        unavailable());
      return { result: r, exitCode: directoriesExitCode(r) };
    }
    case "collection": {
      const r = await (client.collection?.(request.params, options) ??
        unavailable());
      return { result: r, exitCode: collectionExitCode(r) };
    }
    case "handoff": {
      const r = await (client.handoff?.(request.params, options) ??
        unavailable());
      return { result: r, exitCode: handoffExitCode(r) };
    }
    case "prices":
      return {
        result: await (client.prices?.(request.params, options) ??
          unavailable()),
        exitCode: 0,
      };
    case "monitor": {
      const r = await (client.monitor?.(request.params, options) ??
        unavailable());
      return {
        result: r,
        exitCode: r.notifications.some((n) => n.partial) ? 2 : 0,
      };
    }
    case "preferences":
      return {
        result: await (client.preferences?.(request.params, options) ??
          unavailable()),
        exitCode: 0,
      };
  }
}
/** Agent entry accepts only generated product requests, never arbitrary dispatch or a shell. */
export async function runAgentCli(
  command: "api" | "call",
  argv: string[],
  host: AgentHost = {},
): Promise<number> {
  const stdout =
    host.stdout ??
    ((text) => {
      process.stdout.write(text);
    });
  const controller = new AbortController(),
    stop = () => controller.abort();
  const signal = host.signal
    ? AbortSignal.any([host.signal, controller.signal])
    : controller.signal;
  process.once("SIGINT", stop);
  process.once("SIGTERM", stop);
  try {
    argv = configureLanguage(argv);
    let method: string | undefined;
    const seen = new Set<string>();
    for (let i = 0; i < argv.length; i++) {
      const [flag, inline] = argv[i].split(/=(.*)/s);
      if (seen.has(flag)) throw invalid();
      seen.add(flag);
      if (
        [
          "--json",
          "--help",
          "-h",
          ...(command === "api" ? ["--output-schema"] : []),
        ].includes(flag)
      ) {
        if (inline !== undefined) throw invalid();
        continue;
      }
      if (command !== "api" || flag !== "--method") throw invalid();
      method = inline ?? argv[++i];
      if (!method || method.startsWith("-")) throw invalid();
    }
    if (seen.has("--output-schema") && !method) throw invalid();
    if (signal.aborted) throw new CoreError("CANCELLED", "Cancelled");
    if (command === "api" || seen.has("--help") || seen.has("-h")) {
      const { describeAgentApi } = await import("./agent-description.js");
      stdout(
        JSON.stringify(describeAgentApi(method, seen.has("--output-schema"))) +
          "\n",
      );
      return 0;
    }
    if (!host.stdin && process.stdin.isTTY) throw invalid();
    const request = await readRequest(host.stdin ?? process.stdin, signal);
    if (!validateAgentRequest(request)) throw invalid();
    const response = await call(
      request,
      host.createClient?.() ?? createNodeClient(),
      { signal },
    );
    if (signal.aborted) throw new CoreError("CANCELLED", "Cancelled");
    const encoded = JSON.stringify(response.result) + "\n";
    if (Buffer.byteLength(encoded) > OUTPUT_BYTES)
      throw new CoreError("OUTPUT_LIMIT", "Agent output budget exceeded");
    stdout(encoded);
    return response.exitCode;
  } catch (error) {
    const output = agentErrorOutput(error, signal.aborted);
    stdout(JSON.stringify(output) + "\n");
    return output.error.code === "CANCELLED" ? 130 : 1;
  } finally {
    process.off("SIGINT", stop);
    process.off("SIGTERM", stop);
  }
}
