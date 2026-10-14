# 安装与更新 Wombat

中文 | [English](installation.en.md)

Wombat `v0.3.5` 支持 macOS arm64/x64、Linux glibc arm64/x64 和 Windows x64。安装内容已包含运行所需组件，无需开发工具或 API Key。

## 安装运行时与插件

在 macOS 或 Linux 中执行：

```sh
curl -fsSL https://raw.githubusercontent.com/YannByte/wombat/main/scripts/install/install.sh | sh -s -- --plugin
```

在 Windows PowerShell 中执行：

```powershell
& ([scriptblock]::Create((irm https://raw.githubusercontent.com/YannByte/wombat/main/scripts/install/install.ps1))) -Plugin
```

使用插件前需先安装兼容的本机 Codex。安装器先安装 Wombat，再通过 Codex 注册本地 marketplace 并安装插件，最后检查当前项目的发现状态和运行时兼容性。在新的 Codex 项目对话中调用 `$wombat:wombat`；已有已启用的采集插件时，安装器保留该模式，调用 `$wombat-collection:wombat`。安装不改变采集模式或 Hooks 信任。之后可以直接使用 `wombat` 命令。

插件安装失败时，已安装的 Wombat 会保留。检查报错步骤后，macOS/Linux 重新执行安装器并使用 `--plugin-only`，PowerShell 使用 `-PluginOnly`；保留自定义安装目录选项。此模式复用当前运行时，不重复下载，也不修改 PATH；不要附加版本或下载源选项。发现检查失败会保留已安装插件和数据。需要打开 Web 时，执行 `wombat web --open`，或给安装器附加 `--open` / `-Open`。仅安装运行时与 Web，可省略 `--plugin` 或 `-Plugin`。再次失败时，在 [Issues](https://github.com/YannByte/wombat/issues) 反馈问题。浏览器未自动打开时，使用终端输出的完整地址。

## 更新

更新 Wombat 与 Codex 插件时，重新执行上面的带插件选项的安装命令。自定义安装目录或指定版本时，保留相应选项。

下面的命令只检查或更新 Wombat 运行时，不会更新 Codex 管理的插件。

只检查是否有更新：

```sh
wombat update --check
```

安装最新稳定版：

```sh
wombat update
```

## 安装指定版本

安装预发行版本时必须指定版本。macOS 或 Linux 使用 `--version VERSION`，Windows 使用 `-Version VERSION`。预发行版本不会替代最新稳定版。

如需修改安装目录，macOS 或 Linux 使用 `--prefix PATH`，Windows 使用 `-Prefix PATH`。
