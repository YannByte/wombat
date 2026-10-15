import type { CodexOptions } from "./codex/process.js";
import type { captureHooks } from "./codex/hooks.js";
import { spawn } from "node:child_process";
import { createConnection } from "node:net";
import { lstatSync, readFileSync, realpathSync, statSync } from "node:fs";
import { setTimeout as delay } from "node:timers/promises";
import { CoreError } from "../errors.js";
import type {
  ConfigRequest,
  OptimizeRequest,
  LiveRequest,
  QueryOptions,
  HandoffRequest,
  TimingRequest,
  MonitorRequest,
  ConversationRequest,
  ServiceRequest,
} from "../client.js";
import {
  binaryPath,
  decode,
  invokeOperation,
  type CoreProcessOptions,
} from "./core.js";
import { isLocalAccessDenied, localAccessError } from "./local-access.js";
import { managedInstallationRoot } from "./installation.js";
import path from "node:path";
import os from "node:os";

type ProductRequest =
  | { service: ServiceRequest }
  | { conversation: ConversationRequest }
  | { monitor: MonitorRequest }
  | { timing: TimingRequest }
  | LiveRequest
  | { config: ConfigRequest }
  | { optimize: OptimizeRequest }
  | { handoff: HandoffRequest };
type NativeRequest = ProductRequest & {
  nativeHooks?: Awaited<ReturnType<typeof captureHooks>>;
};
function exchange(
  socket: string,
  request: NativeRequest & { serviceInstanceId?: string },
  options: QueryOptions,
  config: CoreProcessOptions,
): Promise<unknown> {
  return new Promise((resolve, reject) => {
    const client = createConnection(socket);
    const chunks: Buffer[] = [];
    let bytes = 0,
      done = false;
    const finish = (error?: Error, result?: unknown) => {
      if (done) return;
      done = true;
      clearTimeout(timer);
      options.signal?.removeEventListener("abort", abort);
      if (
        error instanceof CoreError &&
        ["CANCELLED", "TIMEOUT"].includes(error.code) &&
        client.readyState === "open"
      ) {
        // Release this waiter without interrupting the shared scan. Bound cleanup
        // even if the peer stops reading after accepting a large request.
        const cleanup = setTimeout(() => client.destroy(), 100);
        cleanup.unref();
        client.end("\0", () => {
          clearTimeout(cleanup);
          client.destroy();
        });
      } else client.destroy();
      if (error) reject(error);
      else resolve(result);
    };
    const abort = () => finish(new CoreError("CANCELLED", "已取消"));
    // Explicit refresh also writes and syncs an immutable generation. Keep the
    // short read deadline separate from that bounded publication operation.
    const timeoutMs =
      config.timeoutMs ??
      ("query" in request && request.waitMs
        ? request.waitMs + 2_000
        : "query" in request && request.query.action === "refresh"
          ? 120_000
          : 12_000);
    const timer = setTimeout(
      () => finish(new CoreError("TIMEOUT", "实时用量查询超时")),
      timeoutMs,
    );
    options.signal?.addEventListener("abort", abort, { once: true });
    if (options.signal?.aborted) {
      abort();
      return;
    }
    client.on("connect", () => client.write(JSON.stringify(request) + "\n"));
    client.on("error", (error) =>
      finish(isLocalAccessDenied(error) ? localAccessError() : error),
    );
    client.on("data", (chunk: Buffer) => {
      bytes += chunk.length;
      if (bytes > (config.maxResponseBytes ?? 256_000_000)) {
        finish(new CoreError("OUTPUT_LIMIT", "实时用量响应超过上限"));
        return;
      }
      chunks.push(chunk);
      // A complete response is newline-delimited. Named-pipe servers retain their
      // send buffer until the client closes; waiting only for EOF would deadlock.
      if (chunk.includes(10)) {
        try {
          finish(undefined, decode(Buffer.concat(chunks).toString("utf8")));
        } catch (error) {
          finish(error as Error);
        }
      }
    });
    client.on("end", () => {
      try {
        if (bytes === 0) {
          finish(
            new CoreError(
              "SERVICE_CLOSED",
              "Local service closed before responding",
            ),
          );
          return;
        }
        finish(undefined, decode(Buffer.concat(chunks).toString("utf8")));
      } catch (e) {
        finish(e as Error);
      }
    });
  });
}
interface Endpoint {
  pid?: number;
  protocolVersion: number;
  socket: string;
  instanceId?: string;
  defaultRoots?: string[];
  discoveryError?: boolean;
  contextKey?: string;
}
export function runtimeMatches(
  value: unknown,
  config: CoreProcessOptions,
): boolean {
  if (
    !value ||
    typeof value !== "object" ||
    !("binary" in value) ||
    typeof value.binary !== "string" ||
    !("bytes" in value) ||
    !("modifiedNs" in value)
  )
    return false;
  try {
    const binary = realpathSync(binaryPath(config.binaryPath)),
      meta = statSync(binary, { bigint: true });
    const normalize = (s: string) =>
      process.platform === "win32" ? s.toLowerCase() : s;
    return (
      normalize(realpathSync(value.binary)) === normalize(binary) &&
      value.bytes === Number(meta.size) &&
      value.modifiedNs === String(meta.mtimeNs)
    );
  } catch {
    return false;
  }
}
function installedEndpoint(
  config: CoreProcessOptions,
  management: boolean,
): Endpoint | undefined {
  const root = managedInstallationRoot();
  const file =
    process.env.WOMBAT_ENDPOINT_FILE ??
    (root ? path.join(root, "endpoint.json") : undefined);
  if (!file || config.binaryPath || process.env.WOMBAT_CORE_BIN) return;
  try {
    const stat = lstatSync(file);
    if (
      !stat.isFile() ||
      stat.isSymbolicLink() ||
      stat.size > 65536 ||
      (process.platform !== "win32" &&
        ((stat.mode & 0o077) !== 0 || stat.uid !== process.getuid?.()))
    )
      throw new CoreError(
        "EACCES",
        "Invalid local service endpoint permissions",
      );
    const value: unknown = JSON.parse(readFileSync(file, "utf8"));
    if (
      !value ||
      typeof value !== "object" ||
      !("endpointVersion" in value) ||
      value.endpointVersion !== 1 ||
      !("protocolVersion" in value) ||
      value.protocolVersion !== 2 ||
      !("socket" in value) ||
      typeof value.socket !== "string" ||
      !("instanceId" in value) ||
      typeof value.instanceId !== "string" ||
      !("runtime" in value) ||
      !("context" in value) ||
      !value.context ||
      typeof value.context !== "object"
    )
      throw new CoreError("PROTOCOL_ERROR", "Invalid local service endpoint");
    const keys = [
      "CODEX_HOME",
      "HOME",
      "LOCALAPPDATA",
      "WOMBAT_DATA_HOME",
      "XDG_DATA_HOME",
    ];
    if (Object.keys(value.context).sort().join(",") !== keys.join(","))
      throw new CoreError("PROTOCOL_ERROR", "Invalid service source context");
    const normalize = (s: string) =>
      process.platform === "win32"
        ? path.resolve(s).toLowerCase()
        : path.resolve(s);
    for (const [key, actual] of Object.entries(value.context)) {
      const expected = key === "HOME" ? os.homedir() : process.env[key];
      if (actual !== null && typeof actual !== "string")
        throw new CoreError("PROTOCOL_ERROR", "Invalid service source context");
      if (
        (actual === null) !== (expected === undefined) ||
        (actual !== null &&
          expected !== undefined &&
          normalize(actual) !== normalize(expected))
      )
        return;
    }
    if (!management && !runtimeMatches(value.runtime, config))
      throw new CoreError(
        "SERVICE_VERSION_MISMATCH",
        "Restart the local service after updating Wombat",
      );
    return {
      protocolVersion: 2,
      ...("pid" in value &&
      Number.isSafeInteger(value.pid) &&
      Number(value.pid) > 0
        ? { pid: Number(value.pid) }
        : {}),
      socket: value.socket,
      instanceId: value.instanceId,
    };
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === "ENOENT") return;
    throw error;
  }
}
/** Only an owner-checked endpoint for this exact instance can identify its process. */
export function serviceProcessId(
  config: CoreProcessOptions,
  instanceId: string | null | undefined,
): number | undefined {
  if (!instanceId) return;
  const endpoint = installedEndpoint(config, true);
  return endpoint?.instanceId === instanceId ? endpoint.pid : undefined;
}
export async function queryLive(
  request: ProductRequest,
  options: QueryOptions,
  config: CoreProcessOptions & CodexOptions,
  start = true,
): Promise<unknown> {
  if (options.signal?.aborted) throw new CoreError("CANCELLED", "已取消");
  let observed: NativeRequest = request;
  const product =
    "config" in request
      ? request.config
      : "optimize" in request
        ? request.optimize
        : "handoff" in request
          ? request.handoff
          : undefined;
  const fresh =
    product &&
    product.action !== "capabilities" &&
    product.action !== "activity" &&
    (!product.readView ||
      product.action === "recheck" ||
      product.action === "send");
  if (fresh) {
    const { captureHooks } = await import("./codex/hooks.js");
    observed = {
      ...request,
      nativeHooks: await captureHooks(
        { roots: product.roots, projectRoots: product.projectRoots },
        options,
        config,
      ),
    };
  }
  if ("query" in request && request.query.action === "refresh")
    options.onProgress?.("同步本机日志并保存用量");
  let endpoint =
    installedEndpoint(
      config,
      "service" in request &&
        ["status", "stop"].includes(request.service.action),
    ) ??
    ((await invokeOperation("live_endpoint", {}, options, config)) as Endpoint);
  if (endpoint.protocolVersion !== 2 || typeof endpoint.socket !== "string")
    throw new CoreError("PROTOCOL_ERROR", "实时用量接口版本不兼容");
  let started = false;
  let deadline = Date.now() + 5_000;
  for (;;) {
    endpoint =
      installedEndpoint(
        config,
        "service" in request &&
          ["status", "stop"].includes(request.service.action),
      ) ?? endpoint;
    try {
      if (endpoint.contextKey && !endpoint.instanceId) {
        const status = await exchange(
          endpoint.socket,
          { service: { action: "status" } },
          options,
          config,
        );
        const { validate } =
          await import("../generated/validate-service-response.js");
        const management =
          "service" in request &&
          ["status", "stop"].includes(request.service.action);
        if (
          !validate(status) ||
          status.outputVersion !== 1 ||
          (!management && !runtimeMatches(status.runtime, config))
        )
          throw new CoreError(
            "SERVICE_VERSION_MISMATCH",
            "Another runtime owns this data directory; stop its service before starting this installation",
          );
        // This handshake already is the requested read-only status. A second
        // exchange adds work and races with a service completing shutdown.
        if ("service" in request && request.service.action === "status")
          return status;
        if (status.runtime?.contextKey !== endpoint.contextKey) {
          const needsDefaults =
            ("query" in observed &&
              !observed.query.roots?.length &&
              !observed.query.snapshotId) ||
            ("config" in observed &&
              observed.config.action !== "capabilities" &&
              !observed.config.roots?.length &&
              !observed.config.readView) ||
            ("optimize" in observed &&
              !observed.optimize.roots?.length &&
              !observed.optimize.readView) ||
            ("timing" in observed &&
              observed.timing.action !== "capabilities" &&
              !observed.timing.roots?.length &&
              !observed.timing.snapshotId);
          if (endpoint.defaultRoots && needsDefaults) {
            if (endpoint.discoveryError)
              throw new CoreError(
                "SOURCE_DISCOVERY_FAILED",
                "Default source discovery is incomplete",
              );
            if (
              !Array.isArray(endpoint.defaultRoots) ||
              !endpoint.defaultRoots.every((r) => typeof r === "string")
            )
              throw new CoreError(
                "PROTOCOL_ERROR",
                "Invalid default source roots",
              );
            const roots = endpoint.defaultRoots;
            if (
              "query" in observed &&
              !observed.query.roots?.length &&
              !observed.query.snapshotId
            )
              observed = { ...observed, query: { ...observed.query, roots } };
            else if (
              "config" in observed &&
              observed.config.action !== "capabilities" &&
              !observed.config.roots?.length &&
              !observed.config.readView
            )
              observed = { ...observed, config: { ...observed.config, roots } };
            else if (
              "optimize" in observed &&
              !observed.optimize.roots?.length &&
              !observed.optimize.readView
            )
              observed = {
                ...observed,
                optimize: { ...observed.optimize, roots },
              };
            else if (
              "timing" in observed &&
              observed.timing.action !== "capabilities" &&
              !observed.timing.roots?.length &&
              !observed.timing.snapshotId
            )
              observed = { ...observed, timing: { ...observed.timing, roots } };
          }
        }
        endpoint = { ...endpoint, instanceId: status.instanceId ?? undefined };
      }
      return await exchange(
        endpoint.socket,
        {
          ...observed,
          ...(endpoint.instanceId
            ? { serviceInstanceId: endpoint.instanceId }
            : {}),
        },
        options,
        config,
      );
    } catch (error) {
      if (
        "service" in request &&
        request.service.action === "status" &&
        ["SERVICE_CLOSED", "ECONNRESET", "EPIPE"].includes(
          (error as NodeJS.ErrnoException).code ?? "",
        ) &&
        Date.now() < deadline
      ) {
        endpoint = { ...endpoint, instanceId: undefined };
        await delay(25, undefined, { signal: options.signal });
        continue;
      }
      if (
        error instanceof CoreError &&
        error.code === "SERVICE_CHANGED" &&
        Date.now() < deadline
      )
        continue;
      if (
        !["ENOENT", "ECONNREFUSED"].includes(
          (error as NodeJS.ErrnoException).code ?? "",
        ) ||
        Date.now() >= deadline
      )
        throw error;
      if (!start)
        throw new CoreError("SERVICE_STOPPED", "Local service is not running");
      if (!started) {
        // This shared service owns its lifecycle. Never attach it to caller process-tree cleanup.
        const binary = binaryPath(config.binaryPath);
        const script = /\.[cm]js$/.test(binary);
        const root = managedInstallationRoot();
        const registered =
          root &&
          (await (
            await import("./service.js")
          ).startRegisteredService(options, config));
        if (registered) {
          // Native managers may throttle a recent successful exit. Start the
          // listener wait after their bounded start request has completed.
          deadline = Date.now() + 5_000;
          started = true;
          continue;
        }
        const flag = "--serve-usage";
        const env = root
          ? {
              ...process.env,
              WOMBAT_INSTALL_ROOT: root,
              WOMBAT_ENDPOINT_FILE: path.join(root, "endpoint.json"),
            }
          : process.env;
        const child = spawn(
          script ? process.execPath : binary,
          script ? [binary, flag] : [flag],
          {
            detached: true,
            windowsHide: true,
            stdio: "ignore",
            env,
          },
        );
        started = true;
        const retry = () => {
          started = false;
        };
        child.once("error", retry);
        child.once("exit", retry);
        child.unref();
      }
      try {
        await delay(75, undefined, { signal: options.signal });
      } catch {
        throw new CoreError("CANCELLED", "已取消");
      }
    }
  }
}
