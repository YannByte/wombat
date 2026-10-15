import { test } from "node:test";
import assert from "node:assert/strict";
import { serviceDefinition } from "../src/node/service.js";
import { createUsageClient, CoreError } from "../src/index.js";
import { runtimeMatches } from "../src/node/live.js";
import {
  mkdtempSync,
  writeFileSync,
  chmodSync,
  symlinkSync,
  statSync,
  rmSync,
} from "node:fs";
import os from "node:os";
import path from "node:path";

test(
  "runtime identity accepts a native path alias but rejects changed or absent binaries",
  { skip: process.platform === "win32" },
  () => {
    const root = mkdtempSync(
      path.join(os.tmpdir(), "wombat-runtime-identity-"),
    );
    try {
      const binary = path.join(root, "core"),
        alias = path.join(root, "alias");
      writeFileSync(binary, "synthetic-executable");
      chmodSync(binary, 0o700);
      symlinkSync(binary, alias);
      const metadata = statSync(binary, { bigint: true });
      const identity = {
        binary: alias,
        bytes: Number(metadata.size),
        modifiedNs: String(metadata.mtimeNs),
      };
      assert(runtimeMatches(identity, { binaryPath: binary }));
      assert(
        !runtimeMatches(
          { ...identity, bytes: identity.bytes + 1 },
          { binaryPath: binary },
        ),
      );
      assert(
        !runtimeMatches(
          { ...identity, modifiedNs: "0" },
          { binaryPath: binary },
        ),
      );
      rmSync(alias);
      assert(!runtimeMatches(identity, { binaryPath: binary }));
    } finally {
      rmSync(root, { recursive: true, force: true });
    }
  },
);

test("service definitions retain native argument boundaries and current-user privileges", () => {
  const binary = '/path with spaces/<core>&"',
    startup = "/private/config with spaces.json";
  const mac = serviceDefinition(
    "darwin",
    "0123456789abcdef",
    binary,
    {},
    startup,
  );
  assert.match(mac.content, /&lt;core&gt;&amp;&quot;/);
  assert.match(
    mac.content,
    /<string>--daemon-config<\/string><string>\/private\/config with spaces.json<\/string>/,
  );
  assert.match(mac.content, /<key>SuccessfulExit<\/key><false\/>/);
  assert.match(
    mac.content,
    /<key>ThrottleInterval<\/key><integer>5<\/integer>/,
  );
  assert.match(
    mac.content,
    /<key>ProcessType<\/key><string>Interactive<\/string>/,
  );
  const linux = serviceDefinition(
    "linux",
    "0123456789abcdef",
    "/path/$core%name",
    { CHECK: "/literal/$value" },
    "/private/$SOURCE config.json",
  );
  assert.match(
    linux.content,
    /ExecStart="\/path\/\$\$core%%name" --daemon-config "\/private\/\$\$SOURCE config.json"/,
  );
  assert.match(linux.content, /Environment="CHECK=\/literal\/\$value"/);
  assert.match(linux.content, /Restart=on-failure/);
  assert.doesNotMatch(linux.content, /Nice=/);
  assert.doesNotMatch(linux.content, /User=root/);
  const windows = serviceDefinition(
    "win32",
    "0123456789abcdef",
    "C:\\Path & Tools\\core.exe",
    {},
    "C:\\Path & Tools\\config.json",
    "S-1-5-21-1234",
  );
  assert.match(windows.content, /<RunLevel>LeastPrivilege<\/RunLevel>/);
  assert.match(windows.content, /<Priority>6<\/Priority>/);
  assert.match(windows.content, /<LogonType>InteractiveToken<\/LogonType>/);
  assert.match(
    windows.content,
    /--daemon-config &quot;C:\\Path &amp; Tools\\config.json&quot;/,
  );
  assert.throws(
    () => serviceDefinition("unsupported", "id", binary, {}, startup),
    CoreError,
  );
});

test("service validation rejects unknown controls before the host runs", async () => {
  let called = 0;
  const client = createUsageClient({
    query: async () => ({}),
    service: async () => {
      called++;
      return {};
    },
  });
  await assert.rejects(client.service!({ action: "erase" } as never), {
    code: "INVALID_ARGUMENT",
  });
  assert.equal(called, 0);
  await assert.rejects(client.service!({ action: "status" }), {
    code: "PROTOCOL_ERROR",
  });
  assert.equal(called, 1);
});
