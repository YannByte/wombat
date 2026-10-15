/** Current-user service registration. The Rust service owns indexing and status. */
import { spawn } from "node:child_process";
import {
  existsSync,
  lstatSync,
  mkdirSync,
  readFileSync,
  realpathSync,
  renameSync,
  rmSync,
  writeFileSync,
} from "node:fs";
import os from "node:os";
import path from "node:path";
import { setTimeout as delay } from "node:timers/promises";
import { CoreError } from "../errors.js";
import type { ServiceRequest, ServiceResult, QueryOptions } from "../client.js";
import {
  binaryPath,
  invokeOperation,
  type CoreProcessOptions,
} from "./core.js";
import { queryLive, runtimeMatches, serviceProcessId } from "./live.js";
import { managedInstallationRoot } from "./installation.js";
import { validate as validateResponse } from "../generated/validate-service-response.js";

function registrationName(platform: string, identity: string): string {
  return (
    "io.wombat.agent." + identity + (platform === "linux" ? ".service" : "")
  );
}

export function serviceDefinition(
  platform: string,
  identity: string,
  binary: string,
  env: Record<string, string>,
  startup?: string,
  user?: string,
): { name: string; relative: string; content: string } {
  const xml = (s: string) =>
    s
      .replaceAll("&", "&amp;")
      .replaceAll("<", "&lt;")
      .replaceAll(">", "&gt;")
      .replaceAll('"', "&quot;")
      .replaceAll("'", "&apos;");
  const name = registrationName(platform, identity);
  if (platform === "darwin")
    return {
      name,
      relative: "Library/LaunchAgents/" + name + ".plist",
      content: `<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0"><dict><key>Label</key><string>${name}</string><key>ProgramArguments</key><array><string>${xml(binary)}</string><string>${startup ? "--daemon-config" : "--daemon"}</string>${startup ? `<string>${xml(startup)}</string>` : ""}</array><key>EnvironmentVariables</key><dict>${Object.entries(
        env,
      )
        .map(([k, v]) => `<key>${xml(k)}</key><string>${xml(v)}</string>`)
        .join(
          "",
        )}</dict><key>RunAtLoad</key><true/><key>KeepAlive</key><dict><key>SuccessfulExit</key><false/></dict><key>ThrottleInterval</key><integer>5</integer><key>ProcessType</key><string>Interactive</string></dict></plist>\n`,
    };
  if (platform === "linux") {
    // systemd has its own quoting and percent expansion, independent of a shell.
    const quote = (s: string) =>
      '"' +
      s
        .replaceAll("\\", "\\\\")
        .replaceAll('"', '\\"')
        .replaceAll("%", "%%")
        .replaceAll("\n", "\\n")
        .replaceAll("\r", "\\r") +
      '"';
    const commandQuote = (s: string) => quote(s.replaceAll("$", () => "$$"));
    return {
      name,
      relative: ".config/systemd/user/" + name,
      content: `[Unit]\nDescription=Wombat local indexing\nStartLimitIntervalSec=300\nStartLimitBurst=5\n[Service]\nType=simple\nExecStart=${commandQuote(binary)} ${startup ? "--daemon-config " + commandQuote(startup) : "--daemon"}\n${Object.entries(
        env,
      )
        .map(([k, v]) => "Environment=" + quote(k + "=" + v))
        .join(
          "\n",
        )}\nRestart=on-failure\nRestartSec=30\n[Install]\nWantedBy=default.target\n`,
    };
  }
  if (platform === "win32" && startup && user)
    return {
      name,
      relative: "service-task.xml",
      content: `<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task"><Triggers><LogonTrigger><Enabled>true</Enabled><UserId>${xml(user)}</UserId></LogonTrigger></Triggers><Principals><Principal id="User"><UserId>${xml(user)}</UserId><LogonType>InteractiveToken</LogonType><RunLevel>LeastPrivilege</RunLevel></Principal></Principals><Settings><Priority>6</Priority><MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy><DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries><StopIfGoingOnBatteries>false</StopIfGoingOnBatteries><ExecutionTimeLimit>PT0S</ExecutionTimeLimit><RestartOnFailure><Interval>PT1M</Interval><Count>5</Count></RestartOnFailure></Settings><Actions Context="User"><Exec><Command>${xml(binary)}</Command><Arguments>--daemon-config &quot;${xml(startup)}&quot;</Arguments></Exec></Actions></Task>\n`,
    };
  throw new CoreError(
    "UNSUPPORTED_PLATFORM",
    "Automatic startup is unavailable on this platform",
  );
}
function command(
  binary: string,
  args: string[],
  options: QueryOptions,
  allowed = [0],
): Promise<string> {
  if (options.signal?.aborted) throw new CoreError("CANCELLED", "Cancelled");
  return new Promise((resolve, reject) => {
    const child = spawn(binary, args, {
      stdio: ["ignore", "pipe", "pipe"],
      windowsHide: true,
      detached: process.platform !== "win32",
    });
    let bytes = 0,
      output = "",
      failure: Error | undefined,
      kill: ReturnType<typeof setTimeout> | undefined;
    const stop = () => {
      try {
        if (process.platform !== "win32" && child.pid)
          process.kill(-child.pid, "SIGTERM");
        else child.kill();
      } catch {}
      kill ??= setTimeout(() => {
        try {
          if (process.platform !== "win32" && child.pid)
            process.kill(-child.pid, "SIGKILL");
          else child.kill("SIGKILL");
        } catch {}
      }, 1000);
      kill.unref();
    };
    const abort = () => {
      failure = new CoreError("CANCELLED", "Cancelled");
      stop();
    };
    const timer = setTimeout(() => {
      failure = new CoreError("TIMEOUT", "Service registration timed out");
      stop();
    }, 20_000);
    options.signal?.addEventListener("abort", abort, { once: true });
    process.once("SIGINT", abort);
    process.once("SIGTERM", abort);
    process.once("exit", stop);
    const cleanup = () => {
      clearTimeout(timer);
      if (kill) clearTimeout(kill);
      options.signal?.removeEventListener("abort", abort);
      process.off("SIGINT", abort);
      process.off("SIGTERM", abort);
      process.off("exit", stop);
    };
    child.stdout.on("data", (data: Buffer) => {
      bytes += data.length;
      if (bytes > 65536) {
        failure = new CoreError(
          "OUTPUT_LIMIT",
          "Service command exceeded output budget",
        );
        stop();
      } else output += data.toString("utf8");
    });
    child.stderr.on("data", (data: Buffer) => {
      bytes += data.length;
      if (bytes > 65536) {
        failure = new CoreError(
          "OUTPUT_LIMIT",
          "Service command exceeded output budget",
        );
        stop();
      }
    });
    child.once("error", (error) => {
      cleanup();
      reject(error);
    });
    child.once("close", (code) => {
      cleanup();
      if (failure) reject(failure);
      else if (code === null || !allowed.includes(code))
        reject(
          new CoreError(
            "SERVICE_REGISTRATION_FAILED",
            "Operating-system service command failed; installed files and indexed data are retained",
          ),
        );
      else resolve(output);
    });
    if (options.signal?.aborted) abort();
  });
}
interface Registration {
  version: 1;
  name: string;
  file: string;
  content: string;
  enabled: boolean;
}
function installation(
  identity: string,
  user?: string,
): {
  root: string;
  record: string;
  environment: Record<string, string>;
  definition: ReturnType<typeof serviceDefinition>;
} {
  const configured = managedInstallationRoot();
  if (!configured)
    throw new CoreError(
      "SERVICE_INSTALL_REQUIRED",
      "Automatic startup requires the managed installer",
    );
  const root = path.resolve(configured);
  if (
    lstatSync(root).isSymbolicLink() ||
    readFileSync(path.join(root, ".managed-by-wombat"), "utf8").trim() !==
      "managed GitHub installation"
  )
    throw new CoreError("EACCES", "Invalid managed installation");
  const env: Record<string, string> = {
    WOMBAT_ENDPOINT_FILE: path.join(root, "endpoint.json"),
  };
  for (const key of [
    "WOMBAT_DATA_HOME",
    "CODEX_HOME",
    "XDG_DATA_HOME",
    "LOCALAPPDATA",
  ])
    if (process.env[key] !== undefined)
      env[key] = path.resolve(process.env[key]!);
  if (!/^[0-9a-f]{64}$/.test(identity))
    throw new CoreError(
      "PROTOCOL_ERROR",
      "Invalid service registration identity",
    );
  return {
    root,
    record: path.join(root, "service-registration.json"),
    environment: env,
    definition: serviceDefinition(
      process.platform,
      identity,
      realpathSync(binaryPath()),
      {},
      path.join(root, "service-environment.json"),
      user,
    ),
  };
}
function registration(): Registration | undefined {
  const root = managedInstallationRoot();
  if (!root) return;
  const file = path.join(root, "service-registration.json");
  try {
    const stat = lstatSync(file);
    if (!stat.isFile() || stat.isSymbolicLink() || stat.size > 32768)
      throw new CoreError("EACCES", "Invalid service registration record");
    const value = JSON.parse(
      readFileSync(file, "utf8"),
    ) as Partial<Registration>;
    if (
      value.version !== 1 ||
      typeof value.name !== "string" ||
      typeof value.file !== "string" ||
      typeof value.content !== "string" ||
      typeof value.enabled !== "boolean"
    )
      throw new CoreError(
        "UNSUPPORTED_VERSION",
        "Unsupported service registration record",
      );
    return value as Registration;
  } catch (error) {
    if ((error as NodeJS.ErrnoException).code === "ENOENT") return;
    throw error;
  }
}
export function observedAutoStart(): boolean {
  const record = registration();
  return (
    !!record?.enabled &&
    existsSync(record.file) &&
    !lstatSync(record.file).isSymbolicLink() &&
    readFileSync(record.file, "utf8") === record.content
  );
}
/** A query must restart the registered job, so OS supervision remains intact. */
export async function startRegisteredService(
  options: QueryOptions,
  config: CoreProcessOptions,
): Promise<boolean> {
  const record = registration();
  if (!record?.enabled || !observedAutoStart()) return false;
  const endpoint = (await invokeOperation(
    "live_endpoint",
    {},
    options,
    config,
  )) as {
    registrationId?: unknown;
  };
  if (
    typeof endpoint.registrationId !== "string" ||
    !/^[0-9a-f]{64}$/.test(endpoint.registrationId)
  )
    throw new CoreError(
      "PROTOCOL_ERROR",
      "Invalid service registration identity",
    );
  if (
    record.name !== registrationName(process.platform, endpoint.registrationId)
  )
    return false;
  if (process.platform === "darwin")
    await command(
      "/bin/launchctl",
      ["kickstart", "gui/" + process.getuid!() + "/" + record.name],
      options,
    );
  else if (process.platform === "linux")
    await command(
      "/usr/bin/systemctl",
      ["--user", "start", record.name],
      options,
    );
  else if (process.platform === "win32")
    await command(
      path.join(
        process.env.SystemRoot ?? "C:\\Windows",
        "System32/schtasks.exe",
      ),
      ["/Run", "/TN", record.name],
      options,
    );
  else
    throw new CoreError(
      "UNSUPPORTED_PLATFORM",
      "Automatic startup is unavailable on this platform",
    );
  return true;
}
async function probe(
  request: ServiceRequest,
  options: QueryOptions,
  config: CoreProcessOptions,
): Promise<ServiceResult> {
  try {
    const result = await queryLive(
      { service: request },
      options,
      config,
      false,
    );
    if (
      !validateResponse(result) ||
      result.outputVersion !== 1 ||
      result.action !== request.action
    )
      throw new CoreError("PROTOCOL_ERROR", "Invalid service response");
    return result;
  } catch (error) {
    if (error instanceof CoreError && error.code === "SERVICE_STOPPED") {
      if (!["status", "stop"].includes(request.action)) throw error;
      const result = await invokeOperation(
        "service_offline",
        request,
        options,
        config,
      );
      if (
        !validateResponse(result) ||
        result.outputVersion !== 1 ||
        result.action !== request.action
      )
        throw new CoreError(
          "PROTOCOL_ERROR",
          "Invalid stopped service response",
        );
      return { ...result, autoStart: observedAutoStart() };
    }
    throw error;
  }
}
async function stopService(
  options: QueryOptions,
  config: CoreProcessOptions,
): Promise<ServiceResult> {
  const result = await probe({ action: "stop" }, options, config);
  if (!result.running) return result;
  const pid = serviceProcessId(config, result.instanceId);
  const alive = () => {
    if (pid === undefined) return false;
    try {
      process.kill(pid, 0);
      return true;
    } catch (error) {
      if ((error as NodeJS.ErrnoException).code === "ESRCH") return false;
      throw error;
    }
  };
  const deadline = Date.now() + 25_000;
  while (Date.now() < deadline) {
    await delay(100, undefined, { signal: options.signal });
    const status = await probe({ action: "status" }, options, config);
    if (!status.running) return { ...status, action: "stop" };
    if (status.running && status.instanceId !== result.instanceId)
      throw new CoreError(
        "SERVICE_CHANGED",
        "Another service started during shutdown",
      );
  }
  throw new CoreError(
    "SERVICE_STOP_PENDING",
    "Service is still stopping at a safe transaction boundary",
  );
}
async function configure(
  enable: boolean,
  options: QueryOptions,
  config: CoreProcessOptions,
): Promise<void> {
  const windows = process.platform === "win32",
    system = process.env.SystemRoot ?? "C:\\Windows";
  const user = windows
    ? (
        await command(
          path.join(system, "System32/whoami.exe"),
          ["/user", "/fo", "csv", "/nh"],
          options,
        )
      ).match(/S-1-(?:\d+-)*\d+/)?.[0]
    : undefined;
  const endpoint = (await invokeOperation(
    "live_endpoint",
    {},
    options,
    config,
  )) as {
    registrationId?: unknown;
  };
  if (typeof endpoint.registrationId !== "string")
    throw new CoreError(
      "PROTOCOL_ERROR",
      "Missing service registration identity",
    );
  const { root, record, definition, environment } = installation(
      endpoint.registrationId,
      user,
    ),
    previous = registration();
  if (previous?.enabled && previous.name !== definition.name)
    throw new CoreError(
      "SERVICE_SCOPE_CHANGED",
      "Disable the existing service with its original data-directory settings before changing its scope",
    );
  const file = windows
    ? path.join(root, definition.relative)
    : path.join(os.homedir(), definition.relative);

  if (
    existsSync(file) &&
    (lstatSync(file).isSymbolicLink() ||
      !previous ||
      previous.file !== file ||
      readFileSync(file, "utf8") !== previous.content)
  )
    throw new CoreError(
      "EACCES",
      "Refusing to replace an unmanaged service definition",
    );
  if (enable) await stopService(options, config);
  if (enable)
    writeFileSync(
      path.join(root, "service-environment.json"),
      JSON.stringify({ version: 1, values: environment }) + "\n",
      { mode: 0o600 },
    );
  if (process.platform === "darwin") {
    const domain = "gui/" + process.getuid!(),
      target = domain + "/" + definition.name;
    if (previous?.enabled) {
      await stopService(options, config);
      const loaded = await command(
        "/bin/launchctl",
        ["print", target],
        options,
        [0, 113],
      );
      if (loaded)
        await command("/bin/launchctl", ["bootout", domain, file], options);
    }
    if (enable) {
      mkdirSync(path.dirname(file), { recursive: true, mode: 0o700 });
      writeFileSync(file, definition.content, { mode: 0o600 });
      const next: Registration = {
        version: 1,
        name: definition.name,
        file,
        content: definition.content,
        enabled: false,
      };
      writeFileSync(record, JSON.stringify(next) + "\n", { mode: 0o600 });
      await command("/bin/launchctl", ["bootstrap", domain, file], options);
    }
  } else if (process.platform === "linux") {
    if (previous?.enabled) {
      await stopService(options, config);
      await command(
        "/usr/bin/systemctl",
        ["--user", "disable", "--now", definition.name],
        options,
      );
    }
    if (enable) {
      mkdirSync(path.dirname(file), { recursive: true, mode: 0o700 });
      writeFileSync(file, definition.content, { mode: 0o600 });
      await command("/usr/bin/systemctl", ["--user", "daemon-reload"], options);
      await command(
        "/usr/bin/systemctl",
        ["--user", "enable", "--now", definition.name],
        options,
      );
    }
  } else if (windows) {
    const scheduler = path.join(system, "System32/schtasks.exe");
    if (previous?.enabled) {
      await stopService(options, config);
      await command(
        scheduler,
        ["/Delete", "/TN", definition.name, "/F"],
        options,
      );
    }
    if (enable) {
      writeFileSync(file, "\ufeff" + definition.content, {
        encoding: "utf16le",
      });
      await command(
        scheduler,
        ["/Create", "/TN", definition.name, "/XML", file],
        options,
      );
      await command(scheduler, ["/Run", "/TN", definition.name], options);
    }
  }
  const next: Registration = {
    version: 1,
    name: definition.name,
    file,
    content: enable ? readFileSync(file, "utf8") : definition.content,
    enabled: enable,
  };
  const temporary = path.join(root, "service-registration.next-" + process.pid);
  writeFileSync(temporary, JSON.stringify(next) + "\n", { mode: 0o600 });
  renameSync(temporary, record);
  if (!enable) rmSync(file, { force: true });
}

export async function manageService(
  request: ServiceRequest,
  options: QueryOptions,
  config: CoreProcessOptions,
): Promise<ServiceResult> {
  const action = request.action;
  if (action === "enable" || action === "disable") {
    await configure(action === "enable", options, config);
    const deadline = Date.now() + 5000;
    do {
      const result = await probe({ action: "status" }, options, config);
      if (
        action === "disable" ||
        (result.running &&
          result.resident &&
          runtimeMatches(result.runtime, config))
      )
        return { ...result, action, autoStart: observedAutoStart() };
      await delay(100, undefined, { signal: options.signal });
    } while (Date.now() < deadline);
    throw new CoreError(
      "SERVICE_START_PENDING",
      "Automatic startup is registered, but service startup is not confirmed",
    );
  }
  if (action === "stop")
    return {
      ...(await stopService(options, config)),
      autoStart: observedAutoStart(),
    };
  const result =
    action === "start" || action === "resume"
      ? ((await queryLive(
          { service: request },
          options,
          config,
        )) as ServiceResult)
      : await probe(request, options, config);
  return { ...result, autoStart: observedAutoStart() };
}
