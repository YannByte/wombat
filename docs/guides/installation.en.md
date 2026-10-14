# Install and update Wombat

[中文](installation.md) | English

Wombat `v0.3.4` supports macOS arm64/x64, Linux glibc arm64/x64, and Windows x64. Installation includes everything needed to run Wombat. No development tools or API key are required.

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

If plugin installation fails, Wombat remains installed. Check the reported stage, then rerun the installer with `--plugin-only` on macOS/Linux or `-PluginOnly` on PowerShell. Keep any custom prefix option. This reuses the current runtime without downloading it or changing PATH; omit version and download-source options. A failed discovery check retains the installed plugin and data. To open Web, run `wombat web --open`, or add `--open` / `-Open` to the installer. Omit `--plugin` or `-Plugin` to install only the runtime and Web. If installation still fails, [report the problem](https://github.com/YannByte/wombat/issues). If the browser does not open, use the full URL printed in the terminal.

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
