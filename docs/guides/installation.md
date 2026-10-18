# 安装与更新 Wombat

中文 | [English](installation.en.md)

Wombat `v0.3.6` 支持 macOS arm64/x64、Linux glibc arm64/x64 和 Windows x64。安装内容已包含运行所需组件，无需开发工具或 API Key。

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

插件阶段失败时，已安装的 Wombat 和数据会保留。错误提示会给出失败阶段、错误码、恢复操作和私有诊断摘要的路径。发现检查遇到权限不足、连接断开或超时时，先处理提示中的问题，再只运行 `wombat setup --project PATH --json`，无需重装。Skill 被禁用或存在冲突时，先检查 Codex 中该项目的插件设置，再执行同一只读检查。只有插件安装失败或已确认版本、内容不匹配时，才使用安装器的 `--plugin-only`（macOS/Linux）或 `-PluginOnly`（PowerShell）；保留自定义安装目录选项。此模式复用当前运行时，不重复下载，也不修改 PATH；不要附加版本或下载源选项。向 [Issues](https://github.com/YannByte/wombat/issues) 附加诊断摘要前，请先检查其内容：摘要包含本机项目和 Skill 路径，但不包含原始命令输出或 Hook 配置。

需要打开 Web 时，执行 `wombat web --open`，或给安装器附加 `--open` / `-Open`。仅安装运行时与 Web，可省略 `--plugin` 或 `-Plugin`。浏览器未自动打开时，使用终端输出的完整地址。

提供服务控制能力的运行时会在安装后注册当前用户的自动启动，并在后台准备近期数据，不打开 Web、不信任 Hooks，也不调用模型。若不需要自动启动，macOS/Linux 加 `--no-service`，Windows 加 `-NoService`。`--plugin-only` 或 `-PluginOnly` 不改变服务。尚未提供服务控制的公开运行时保留原有行为。可用 `wombat service status --json` 检查进度；安装成功不代表数据已经就绪。暂停、恢复和移除方式见[后台索引](cli.md)。

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
