# 安装与更新 Wombat

中文 | [English](installation.en.md)

Wombat `v0.7.0` 支持 macOS arm64/x64、Linux glibc arm64/x64 和 Windows x64。安装内容已包含运行所需组件，无需开发工具或 API Key。

## 安装运行时与插件

在 macOS 或 Linux 中执行：

```sh
curl -fsSL https://raw.githubusercontent.com/YannByte/wombat/main/scripts/install/install.sh | sh -s -- --plugin
```

在 Windows PowerShell 中执行：

```powershell
& ([scriptblock]::Create((irm https://raw.githubusercontent.com/YannByte/wombat/main/scripts/install/install.ps1))) -Plugin
```

先安装兼容的本机 Codex。安装器安装 Wombat，通过 Codex 注册单条本地 marketplace、安装唯一插件，再检查当前项目的发现状态与运行时兼容性。在新项目对话中调用 `$wombat:wombat`。默认不探测可选 Hooks，不改变采集模式或信任。之后可直接使用 `wombat`。

插件阶段失败时，已安装的 Wombat 和数据会保留。错误提示会给出失败阶段、错误码、恢复操作和私有诊断摘要的路径。发现检查遇到权限不足、连接断开或超时时，先处理提示中的问题，再只运行 `wombat system doctor --project PATH --json`，无需重装。Skill 被禁用或存在冲突时，先检查 Codex 中该项目的插件设置，再执行同一只读检查。只有插件安装失败或已确认版本、内容不匹配时，才使用安装器的 `--plugin-only`（macOS/Linux）或 `-PluginOnly`（PowerShell）；保留自定义安装目录选项。此模式复用当前运行时，不重复下载，也不修改 PATH；不要附加版本或下载源选项。向 [Issues](https://github.com/YannByte/wombat/issues) 附加诊断摘要前，请先检查其内容：摘要包含本机项目和 Skill 路径，但不包含原始命令输出或 Hook 配置。


安装仅提供 CLI 与 Skill，不启动索引、常驻服务或自动启动。首次查询在前台准备内部索引，后续查询复用已提交事实并采集增量，不需要初始化命令。如果旧版曾明确启用自动启动，请先用旧版停用；新版不管理旧服务注册。

## 更新

更新 Wombat 与 Codex 插件时，重新执行上面的带插件选项的安装命令。自定义安装目录或指定版本时，保留相应选项。

下面的命令只检查或更新 Wombat 运行时，不会更新 Codex 管理的插件。

只检查是否有更新：

```sh
wombat system update --check
```

安装最新稳定版：

```sh
wombat system update
```

## 安装指定版本

安装预发行版本时必须指定版本。macOS 或 Linux 使用 `--version VERSION`，Windows 使用 `-Version VERSION`。预发行版本不会替代最新稳定版。

如需修改安装目录，macOS 或 Linux 使用 `--prefix PATH`，Windows 使用 `-Prefix PATH`。

Wombat 文件已安装且程序包兼容性检查通过时，其他 Codex Skill 的解析错误会标记为项目发现尚未完成。安装器保留成功的安装结果，并列出已报告的错误路径。修复对应 Skill 后运行 `wombat system doctor --json`，无需重装 Wombat。若解析错误指向选中的 Wombat Skill，或 Wombat 自身兼容性检查失败，安装验证仍明确失败。

验证命令以错误状态退出时，安装摘要保留退出码和结构化失败检查；插件兼容性、本机安装检查与 Skill 解析问题分别报告。没有结构化证据的命令失败，不归因于其他 Skill。按提示运行只读 doctor 检查，不重复安装。
