<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="assets/wombat-logo-dark.svg">
    <img src="assets/wombat-logo-light.svg" alt="Wombat" width="80" height="56">
  </picture>
</p>

# Wombat · 改进你的 Codex Harness

中文 | [English](README.md)

Wombat 是一个本地 Codex 插件，帮你从真实工作记录里找出值得改进的地方。遇到反复失败、漏跑检查或多次纠正的任务，可以回查原始对话，找出相关的项目指令、Skills、工具配置和验证步骤。这些共同构成了你的 Codex Harness。

Wombat 提供证据和检查结果。你选定要改的地方后，Codex 执行修改，再按原范围和规则复查。

## 安装 Wombat

**正式版：[`v0.9.0`](https://github.com/YannByte/wombat/releases/tag/v0.9.0)。**

先安装支持原生插件的 Codex，确保本机可用 `codex` 命令。Wombat 的本地分析无需单独配置 API Key 或安装开发工具。

在 macOS 或 Linux 上安装 Wombat 与插件。

```sh
curl -fsSL https://raw.githubusercontent.com/YannByte/wombat/main/scripts/install/install.sh | sh -s -- --plugin
```

在 Windows PowerShell 中安装。

```powershell
& ([scriptblock]::Create((irm https://raw.githubusercontent.com/YannByte/wombat/main/scripts/install/install.ps1))) -Plugin
```

## 从一个反复出错的任务开始

在 Codex 中打开一个做过任务的项目，新建对话，然后提问。

> $wombat:wombat 看看这个项目最近 7 天的 Codex 记录。找一个反复失败或需要我多次纠正的任务，引用原话，检查相关的项目指令和验证流程，给出一项具体修改和复查办法。

**示例项目。** 两次发布任务都停在安装验证。最近一次对话第 18 轮，你说「还是找不到安装检查清单，先别发布」。当前 `AGENTS.md` 仍指向已移除的 `docs/release-checklist.md`。相同的安装验证步骤现在记录在 `docs/install-checks.md`。建议只更新这条链接，再用原规则查一遍。

看过依据后，可以继续说：

> 只更新这条清单链接。改完按原规则复查，并告诉我还有什么没验证。

**示例结果。** Codex 更新了链接。原引用检查通过，现行清单可以打开。安装器能否顺利安装最新版，仍需单独验证。

如果所选日期内缺少可用记录，可以扩大日期范围，或请 Wombat 先检查当前项目指令。插件未出现时，按[安装指南](docs/guides/installation.md)排查。

## 按改进目标使用 Wombat

| 你想解决什么 | Wombat 能帮你做什么 |
|---|---|
| 找到值得复盘的任务 | 从失败、重试、用量和耗时变化入手，找到相关任务和原始对话。 |
| 定位 Harness 的问题 | 复盘任务过程，检查项目指令、Skills、MCP、Hooks 和项目验证入口，核对配置与实际使用的证据。 |
| 选择并落实改动 | 查看证据、影响范围和复查办法。由你决定修改或保留什么，再由 Codex 执行获准的项目改动。清理先预览候选，确认具体对象后才删除。删除后无法恢复。 |
| 改完确认有没有解决 | 在原范围按原规则复查问题或清理对象，查看做过的测试和待验证项。要判断长期效果，再比较工作量和验收条件相近的后续任务。 |
| 把经验带到下一次 | 找回过去的决定，接续未完成的工作，或把有来源的结论整理进项目文档。 |

在 Apple 芯片 Mac 上，可以用同一个真实任务对照四套项目指令，应用验证过的改动并确认新会话加载。[真实任务试跑](docs/guides/plugin.md#harness-trial)

查看各阶段的提问方式和预期结果。[插件指南](docs/guides/plugin.md#harness-workflow)

## 管理用量、安装和集成

- **管理用量。** 查看 Token 用量、费用估算、可用的账户额度记录和预算提醒。[CLI 用量与预算](docs/guides/cli.md#usage)
- **维护本机环境。** 检查运行和插件发现状态。只用 CLI 时，安装命令省略 `--plugin` 或 `-Plugin`。[安装与排查](docs/guides/installation.md)
- **接入脚本。** 用 JSON 查询或调用结构化接口。来源目录授权、可选采集和显示偏好也有各自的接口。[自动化接口](docs/guides/cli.md#automation)

## 平台与隐私

发行包支持 macOS arm64/x64、Linux glibc arm64/x64 和 Windows x64。Codex 产物清理目前针对 macOS 目录布局。

CLI 在本机分析，不上传日志、不调用模型。Skill 选取的证据会进入当前 Codex 对话并消耗 Token。详见[隐私说明](docs/reference/privacy.md)。

## 更新

重新执行上面的安装命令，可以更新 Wombat 与 Codex 插件。保留指定版本或自定义安装目录的选项。

更新选项和仅更新运行时的方式见[安装指南](docs/guides/installation.md)。

## 帮助与贡献

[反馈问题或需求](https://github.com/YannByte/wombat/issues)时，描述现象与所用工具，无需提供私有日志。安全问题按[安全政策](.github/SECURITY.zh-CN.md)报告。源码开发见[贡献指南](.github/CONTRIBUTING.zh-CN.md)。

## 许可证

[MIT](LICENSE)。依赖许可证见[第三方声明](licenses/THIRD_PARTY_NOTICES.md)。
