<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="assets/wombat-logo-dark.svg">
    <img src="assets/wombat-logo-light.svg" alt="Wombat" width="80" height="56">
  </picture>
</p>

# Wombat · Codex 项目用量与对话回查

中文 | [English](README.md)

Wombat 是 OpenAI Codex 的本地 CLI 和插件。按项目查看 Token 用量，找出消耗最多的任务，再回查原话，看看当时发生了什么。

Wombat 从本机 Codex 记录计算用量、定位证据，也会提供可复查的项目检查结果和清理清单。Codex 根据这些证据解释问题，并执行已授权的项目修改。

## 为 Codex 安装 Wombat

**正式版：[`v0.7.1`](https://github.com/YannByte/wombat/releases/tag/v0.7.1)。**

先安装支持原生插件的 Codex，并确保本机可用 `codex` 命令。Wombat 本机分析无需单独配置 API Key 或安装开发工具。

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
3. 提出以下问题。

> $wombat:wombat 今天这个项目用了多少 Token？列出消耗最多的三个任务，并引用第一项任务的原始请求。

回答会给出日期范围、项目总用量、任务排行和原话位置，也会说明记录有哪些缺口。安装失败时，按提示恢复。详见[安装指南](docs/guides/installation.md)。

## 看看会得到什么

### 找出最耗 Token 的任务

> 这个项目本周哪三个任务最耗 Token？给我看看第一项任务的原始请求，以及后来发生了什么。

**回答示例。** 本周这个项目记录了 140 万 Token。用量最高的三项是安装排查（90 万）、Windows 测试修复（18 万）和发布说明（12 万）。

「安装排查」第 18 轮的原话是「在干净目录里重新测一下安装器」。第 19 至 21 轮记录了安装检查失败，以及更换安装路径后重试的过程。

在项目目录中运行以下命令，可直接查看任务排行。

```sh
wombat usage top week --project . --limit 3
```

### 按原规则复查项目问题

> 检查这个项目的发布说明。找出失效的引用，并告诉我怎么验证修复。

**检查示例。** Wombat 在 `AGENTS.md` 中定位到一条链接，指向缺失的 `docs/release-checklist.md`。检查结果包含项目范围和发现问题所用的规则。

> 恢复这份清单，再跑一次原来的检查。

**复查示例。** Codex 恢复清单后，Wombat 在同一项目中再次执行原引用检查。复查结果显示，缺失文件的问题已消失。

### 先确认哪些文件可以清理

> 检查项目里的编译产物和临时文件。保留 `dist/` 中待发布的安装包，先给我看清理清单。

**预览示例。** Wombat 列出路径、检查到的大小和判断依据。

- `target/` 是 2.4 GB 的 Cargo 编译缓存，可以重新生成。
- `.tmp/test-run-42/` 有 180 MB 文件。暂时无法确认归属，Wombat 将它列为待核查。
- `dist/` 含待发布的安装包，已标记为保留。

> 只删除 `target/`，然后复查原路径。

**复查示例。** Codex 删除获准清理的目录后，Wombat 按原清单确认 `target/` 已不存在。

## 还可以这样问

用 `$wombat:wombat` 提问，例如：

| 任务 | 示例问题 |
|---|---|
| 对比项目用量 | 对比这个项目最近两个完整自然周的用量。哪些任务和模型的变化最大？ |
| 回看高用量任务 | 这个任务有多少用量来自缓存输入？给我看看原始请求和记录处理过程的轮次。 |
| 跨对话找回决定 | 查这个项目本月的对话，找我说过的重试、超时或退避决策，附上原话和位置。 |
| 排查反复失败 | 找出以前发生过的同类安装失败。给我看原始报错、试过的做法和已有结果。 |
| 复查一项问题 | 改完后在同一项目里按原规则再查一次。还能发现同样的问题吗？ |
| 核对清理候选 | 哪些编译产物可以重新生成？列出路径和判断依据，保留待交付的文件。 |

可以从这些问题出发，用自己的话描述日常任务。补充项目、日期范围或具体问题，有助于缩小范围。还可以追问「给我看原话」或「按原规则再查一次」。

更多用法见[插件指南](docs/guides/plugin.md)，直接查询见 [CLI 指南](docs/guides/cli.md)。

## 常见问题

### 如何回查任务的原始对话？

用 `$wombat:wombat` 引用排行中某项任务的对话，也可以提供项目、日期范围和记得的关键词。Wombat 会返回匹配的片段和原对话位置。

### 不安装插件也能使用 Wombat 吗？

可以。安装时省略 `--plugin` 或 `-Plugin`，直接使用 CLI。CLI 查询支持 JSON 输出，详见 [CLI 指南](docs/guides/cli.md)。

## 使用须知

- CLI 在本机分析，不上传日志、不调用模型。Skill 选取的证据会进入当前 Codex 对话并消耗 Token，详见[隐私说明](docs/reference/privacy.md)。
- 清理需要对选定对象的授权。删除后无法恢复。清理检查会保护会话、状态、配置和凭据，条件变化或无法判断时停止删除。详见[清理指南](docs/guides/cli.md)。

发行包支持 macOS arm64/x64、Linux glibc arm64/x64 和 Windows x64。Codex 产物清理目前针对 macOS 目录布局。不受支持的记录格式如何处理，见[格式恢复说明](docs/reference/cli.md)。

## 更新

重新执行上面的安装命令，可以更新 Wombat 与 Codex 插件。保留指定版本或自定义安装目录的选项。

更新选项和仅更新运行时的方式见[安装指南](docs/guides/installation.md)。

## 帮助与贡献

直接查询、JSON 输出和预算控制见 [CLI 指南](docs/guides/cli.md)。源码环境配置见[贡献指南](.github/CONTRIBUTING.zh-CN.md)。

[反馈问题或需求](https://github.com/YannByte/wombat/issues)时，描述现象与所用工具，无需提供私有日志。安全问题请按[安全政策](.github/SECURITY.zh-CN.md)报告。

## 许可证

[MIT](LICENSE)。依赖许可证见[第三方声明](licenses/THIRD_PARTY_NOTICES.md)。
