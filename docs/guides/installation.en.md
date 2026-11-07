# Install and update Wombat

[中文](installation.md) | English

Wombat `v0.7.0` supports macOS arm64/x64, Linux glibc arm64/x64, and Windows x64. Installation includes everything needed to run Wombat. No development tools or API key are required.

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


Installation provides the CLI and Skill without indexing, a resident service or automatic startup. The first query prepares its internal index in the foreground; later queries reuse committed facts and collect increments. No initialization command is required. If an older version was explicitly registered for automatic startup, disable it with that version before upgrading; the new runtime does not manage old service registrations.

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

If Wombat files are installed and its bundle passes compatibility checks, unrelated Codex Skill parsing errors are reported as incomplete project discovery. The installer retains the successful installation and identifies reported error paths. Fix those Skill errors and run `wombat system doctor --json`; do not reinstall Wombat. A parsing error attributed to the selected Wombat Skill or failed Wombat compatibility remains an installation verification failure.

When verification exits with an error, the installation summary preserves its exit code and any structured failing check. Compatible plugin files, local installation checks and Skill parsing errors are reported separately. A command failure without structured evidence is not attributed to another Skill. Use the supplied read-only doctor command rather than reinstalling.
