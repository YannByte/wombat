# Install and update Wombat

[中文](installation.md) | English

Wombat `v0.3.6` supports macOS arm64/x64, Linux glibc arm64/x64, and Windows x64. Installation includes everything needed to run Wombat. No development tools or API key are required.

## Install the runtime and plugin

On macOS or Linux, run:

```sh
curl -fsSL https://raw.githubusercontent.com/YannByte/wombat/main/scripts/install/install.sh | sh -s -- --plugin
```

On Windows PowerShell, run:

```powershell
& ([scriptblock]::Create((irm https://raw.githubusercontent.com/YannByte/wombat/main/scripts/install/install.ps1))) -Plugin
```

Install a compatible local Codex first. The installer installs Wombat, registers its local marketplace and installs the plugin through Codex, then verifies current-project discovery and runtime compatibility. Invoke `$wombat:wombat` in a new Codex project conversation. An existing enabled collection plugin keeps that mode and uses `$wombat-collection:wombat`. Installation does not change collection mode or Hook trust. The `wombat` command is available for later use.

If the plugin stage fails, Wombat and its data remain installed. The error identifies the stage, error code, recovery action and private diagnostic summary path. For discovery permission errors, disconnects or timeouts, correct the reported condition and run only `wombat setup --project PATH --json`; do not reinstall. For a disabled or conflicting Skill, check the project's plugin settings in Codex before the same read-only check. Only a failed plugin installation or confirmed version/content mismatch needs `--plugin-only` on macOS/Linux or `-PluginOnly` on PowerShell. Keep any custom prefix option. This reuses the current runtime without downloading it or changing PATH; omit version and download-source options. Review the diagnostic summary before attaching it to an [issue](https://github.com/YannByte/wombat/issues): it includes local project and Skill paths, but excludes raw command output and Hook configuration.

To open Web, run `wombat web --open`, or add `--open` / `-Open` to the installer. Omit `--plugin` or `-Plugin` to install only the runtime and Web. If the browser does not open, use the full URL printed in the terminal.

A runtime that exposes service controls registers current-user automatic startup and prepares recent data in the background after installation. It does not open Web, trust Hooks or invoke a model. To omit automatic startup, add `--no-service` on macOS/Linux or `-NoService` on Windows. `--plugin-only` / `-PluginOnly` does not change the service. Older public runtimes without service controls retain their existing behavior. Check progress with `wombat service status --json`; installation success does not establish data readiness. See [background indexing](cli.en.md) for pause, resume and removal.

## Update

To update Wombat and its Codex plugin, rerun the installation command above with the plugin option. Keep any options for a custom installation directory or a specific version.

The commands below only check or update the Wombat runtime. They do not update the plugin managed by Codex.

Check for an update without installing it:

```sh
wombat update --check
```

Install the latest stable release:

```sh
wombat update
```

## Install a specific version

Pre-release versions require an explicit version. On macOS or Linux, add `--version VERSION`; on Windows, add `-Version VERSION`. Pre-releases do not replace the latest stable release.

To use a different installation directory, add `--prefix PATH` on macOS or Linux, or `-Prefix PATH` on Windows.
