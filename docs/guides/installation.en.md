# Install and update Wombat

[中文](installation.md) | English

Wombat `v0.5.0` supports macOS arm64/x64, Linux glibc arm64/x64, and Windows x64. Installation includes everything needed to run Wombat. No development tools or API key are required.

## Install the runtime and plugin

On macOS or Linux, run:

```sh
curl -fsSL https://raw.githubusercontent.com/YannByte/wombat/main/scripts/install/install.sh | sh -s -- --plugin
```

On Windows PowerShell, run:

```powershell
& ([scriptblock]::Create((irm https://raw.githubusercontent.com/YannByte/wombat/main/scripts/install/install.ps1))) -Plugin
```

Install a compatible local Codex first. The installer installs Wombat, registers one local marketplace entry, installs the single plugin through Codex, then checks current-project discovery and runtime compatibility. Invoke `$wombat:wombat` in a new project conversation. Default setup does not probe optional Hooks or change collection mode or trust. The `wombat` command remains available.

If the plugin stage fails, Wombat and its data remain installed. The error identifies the stage, error code, recovery action and private diagnostic summary path. For discovery permission errors, disconnects or timeouts, correct the reported condition and run only `wombat system doctor --project PATH --json`; do not reinstall. For a disabled or conflicting Skill, check the project's plugin settings in Codex before the same read-only check. Only a failed plugin installation or confirmed version/content mismatch needs `--plugin-only` on macOS/Linux or `-PluginOnly` on PowerShell. Keep any custom prefix option. This reuses the current runtime without downloading it or changing PATH; omit version and download-source options. Review the diagnostic summary before attaching it to an [issue](https://github.com/YannByte/wombat/issues): it includes local project and Skill paths, but excludes raw command output and Hook configuration.


Installation starts no indexing and registers no automatic startup by default. Queries trigger preparation and reuse committed results. To keep an optional resident query service, add `--service` on macOS/Linux or `-Service` on Windows; it stays idle without queries. `--plugin-only` / `-PluginOnly` does not change the service. Existing automatic-start registrations remain until method `service` with action `disable` through `wombat system call` removes them; updated services also index only on demand. Installation success does not establish data readiness. See [query service controls](cli.en.md) for progress, pause, resume and removal.

## Update

To update Wombat and its Codex plugin, rerun the installation command above with the plugin option. Keep any options for a custom installation directory or a specific version.

The commands below only check or update the Wombat runtime. They do not update the plugin managed by Codex.

Check for an update without installing it:

```sh
wombat system update --check
```

Install the latest stable release:

```sh
wombat system update
```

## Install a specific version

Pre-release versions require an explicit version. On macOS or Linux, add `--version VERSION`; on Windows, add `-Version VERSION`. Pre-releases do not replace the latest stable release.

To use a different installation directory, add `--prefix PATH` on macOS or Linux, or `-Prefix PATH` on Windows.
