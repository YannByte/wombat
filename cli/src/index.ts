import { firstCommandIndex } from "./command-routing.js";
import { enableInstalledCompileCache } from "@wombat/client/node/runtime";
enableInstalledCompileCache();
const argv = process.argv.slice(2),
  index = firstCommandIndex(argv);
const command = argv[index];
process.exitCode =
  command === "api" || command === "call"
    ? await (
        await import("./agent-cli.js")
      ).runAgentCli(command, [
        ...argv.slice(0, index),
        ...argv.slice(index + 1),
      ])
    : await (await import("./usage-app-cli.js")).runUsageCli(argv);
