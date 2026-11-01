import { enableInstalledCompileCache } from "@wombat/client/node/runtime";
enableInstalledCompileCache();
process.exitCode = await (await import("./usage-app-cli.js")).runUsageCli(process.argv.slice(2));
