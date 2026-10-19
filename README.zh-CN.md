<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="assets/wombat-logo-dark.svg">
    <img src="assets/wombat-logo-light.svg" alt="Wombat" width="80" height="56">
  </picture>
</p>

# Wombat — 复盘 Codex 工作与用量

中文 | [English](README.md)

看清做了什么工作、Token 用在哪里，以及哪些地方可以改善。

Wombat 根据本机 Codex 记录计算用量、定位历史对话。通过配套 Skills，Codex 可以按需读取原文，复盘每日工作、调查指定任务并检查配置。

## 开始使用

**正式版：[`v0.3.7`](https://github.com/YannByte/wombat/releases/tag/v0.3.7)。**

本机分析无需 API Key 或开发工具。使用插件前，先安装兼容的本机 Codex。

在 macOS 或 Linux 上安装 Wombat 与插件：

```sh
curl -fsSL https://raw.githubusercontent.com/YannByte/wombat/main/scripts/install/install.sh | sh -s -- --plugin
```

Windows PowerShell：

```powershell
& ([scriptblock]::Create((irm https://raw.githubusercontent.com/YannByte/wombat/main/scripts/install/install.ps1))) -Plugin
```

1. 在 Codex 中打开项目。
2. 新建对话。
3. 提问：

> $wombat:wombat-daily-review 复盘这个项目今天的工作和 Token 用量，结合原文给出一项具体改进建议。

复盘会结合已记录的用量和选定原文，区分请求、已记录的工作与已验证的交付，并说明证据缺口。

## 按任务选择 Skill

| 任务 | Skill | 提问示例 |
|---|---|---|
| 复盘每日或每周工作与用量 | `$wombat:wombat-daily-review` | 这周做了什么工作？用量增加在哪里，有哪些具体改进？ |
| 调查指定任务 | `$wombat:wombat-task-retro` | 复盘这项任务的重复工作、失败和耗时，给出原文依据。 |
| 检查项目配置 | `$wombat:wombat-config-review` | 检查这个项目的 AGENTS.md、Skills、MCP 和 Hooks，指出具体文件并给出修改建议。 |

直接查询用量、费用、耗时、账户额度或 Token 预算时，使用 `$wombat:wombat`。例如：「这个项目今天用了多少 Token？」

复盘和检查从只读开始。Codex 仅在获得授权后执行修改。配置修改完成后，可以要求复查原问题。请求一份报告不会自动创建定期报告任务。

回答使用提问所用的语言。采集插件及其他调用名称见[插件指南](docs/guides/plugin.md)，持续预算检查见 [CLI 指南](docs/guides/cli.md)。

## 首次准备与查询结果

安装器会启用后台索引和当前用户的自动启动。首次准备优先处理最近 30 天的数据。查询更早的历史记录时，可能需要额外准备。

安装成功不代表数据已经就绪。首次查询可能返回不完整结果。查看准备状态：

```sh
wombat service status --json
```

缺失或部分数据不能当作零用量。高用量本身不能证明浪费，没有使用记录也不能证明配置闲置。

插件阶段失败时，Wombat 和数据会保留。请按报错中的恢复步骤和[安装指南](docs/guides/installation.md)处理。服务控制方式也见该指南。仅使用 Web 时，安装命令省略 `--plugin` 或 `-Plugin`。

## 打开 Web

```sh
wombat web --open
```

浏览器未自动打开时，使用终端输出的完整地址。页面和数据恢复方式见 [Web 指南](docs/guides/web.md)。

## 更新

重新执行上面的安装命令，可以更新 Wombat 与 Codex 插件。检查更新，或只更新 Wombat 运行时：

```sh
wombat update --check
wombat update
```

指定版本和自定义安装目录见[安装指南](docs/guides/installation.md)。

## 范围与限制

- 分析本机 Codex 记录。发行包支持 macOS arm64/x64、Linux glibc arm64/x64 和 Windows x64。
- Wombat 本机查询不上传日志、不调用模型。在 Codex 中使用这些 Skills 会消耗模型 Token。
- 缺价时可能下载官方价表。网络行为和原文访问边界见[隐私说明](docs/reference/privacy.md)。
- Token、API 等价估算金额和账户额度分别展示。估算金额不是订阅账单，见[计价说明](docs/reference/pricing.md)。

不受支持的数据格式如何处理，见[格式恢复说明](docs/reference/cli.md)。

## 帮助与贡献

JSON 查询与预算控制见 [CLI 指南](docs/guides/cli.md)。源码环境配置见[贡献指南](.github/CONTRIBUTING.zh-CN.md)。

[反馈问题或需求](https://github.com/YannByte/wombat/issues)时，描述现象与所用工具，无需提供私有日志。安全问题请按[安全政策](.github/SECURITY.zh-CN.md)报告。

## 许可证

[MIT](LICENSE)。依赖许可证见[第三方声明](licenses/THIRD_PARTY_NOTICES.md)。
