<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="assets/wombat-logo-dark.svg">
    <img src="assets/wombat-logo-light.svg" alt="Wombat" width="80" height="56">
  </picture>
</p>

# Wombat — 诊断并改善编码 Agent 的工作

中文 | [English](README.md)

找出编码 Agent 任务的阻碍，给出具体改法，再复查原问题。

Wombat 结合高效的本地工作历史检索与准确的用量计算。一个主 Skill 按任务选取证据，帮助 Codex 检查、修改和验证项目。直接提出问题即可。静态工程检查无需项目初始化或扫描全部历史。

## 开始使用

**正式版：[`v0.4.0`](https://github.com/YannByte/wombat/releases/tag/v0.4.0)。**

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

> $wombat:wombat 查清这个项目的交付检查为什么失败。给出证据、具体修改位置和复查方法。

## 选择任务

所有任务共用 `$wombat:wombat` 入口：

| 任务 | 示例问题 |
|---|---|
| 接续工作 | 这项任务之前怎么决定的，下一步做什么？ |
| 排查阻碍 | 这个检查为什么反复失败？定位到实际工程入口。 |
| 核对交付 | 结果满足原始要求了吗？还缺哪些检查？ |
| 复盘工作与用量 | 这周做了什么，哪一项改善最有帮助？ |
| 改善工作流程 | 把已有检查接到交付入口，再验证无效输入会被拦住。 |

简单的用量、费用和耗时问题也使用同一入口。检查从只读开始。获得授权后修改，再复查原问题。静态检查、后续采用和结果改善分别报告。报告请求不会自动建立定时任务。

回答跟随你的语言。集合插件及独立副本的调用方式见[插件指南](docs/guides/plugin.md)，直接查询见 [CLI 指南](docs/guides/cli.md)。

## 首次准备与查询结果

安装时不启动索引，也不启用自动启动。首次查询才准备所需数据，后续查询复用本地索引。近期查询优先处理最近 30 天的数据；查询更早的历史记录时，可能需要额外准备。

查询准备期间可能返回不完整结果。查看当前查询的准备状态：

```sh
wombat service status --json
```

缺失或部分数据不能当作零用量。高用量本身不能证明浪费，没有使用记录也不能证明配置闲置。

插件阶段失败时，Wombat 和数据会保留。请按报错中的恢复步骤和[安装指南](docs/guides/installation.md)处理。服务控制方式也见该指南。仅使用 CLI 时，安装命令省略 `--plugin` 或 `-Plugin`。

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
