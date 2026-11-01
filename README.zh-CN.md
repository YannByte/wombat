<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="assets/wombat-logo-dark.svg">
    <img src="assets/wombat-logo-light.svg" alt="Wombat" width="80" height="56">
  </picture>
</p>

# Wombat

中文 | [English](README.md)

找回之前的决定，看清用量，找到项目问题和可清理的产物。

在 Codex 中使用 Wombat。Wombat 检索本机工作记录、计算用量，Codex 根据证据解释问题、执行授权的修改并复查。

## 看看会得到什么

### 查用量

> **提问：** 今天用了多少，主要花在哪里？

**回答示例：** 共 120 万 Token：安装排查 90 万、测试修复 20 万、文档更新 10 万。一个模型未计价，因此费用估算不完整。

### 找到具体改进点

> **提问：** 看看这个项目的发布流程有什么值得改进的。

**回答示例：** `scripts/ship.ts` 上传前没有执行已有的包校验。建议在上传前调用这项校验。改后用正常包和缺文件的包运行同一条发布路径：前者应通过，后者应在上传前被拦住。

### 清理

> **提问：** 看看 Codex 有哪些文件可以清理，先不要删除。

**清理预览示例：**

- **候选：** `releases/1.0.0-darwin-arm64`，扫描大小为 240 MB，既不是当前版本，也不是待更新版本。
- **保留：** 当前版本、待更新版本，以及 Codex 正在运行时的桌面缓存。
- **操作：** 尚未删除文件。清理选中的旧版本会永久删除它。

> **继续提问：** 只永久删除这个旧版本并复查，其他内容都保留。

**后续结果示例：** 已删除选中的旧版本，复查确认原路径已不存在。当前版本、待更新版本和桌面缓存均保留。

## 开始使用

**正式版：[`v0.5.0`](https://github.com/YannByte/wombat/releases/tag/v0.5.0)。**

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
3. 提问：

> $wombat:wombat 看看今天的用量，列出消耗最多的三个任务，并说明数据是否完整。

回答会使用提问时的语言，包含时间范围、总用量、主要任务和缺失数据。安装遇到问题时，按提示恢复，详见[安装指南](docs/guides/installation.md)。仅使用 CLI 时，安装命令省略 `--plugin` 或 `-Plugin`。

## 还可以这样问

用 `$wombat:wombat` 提问，例如：

| 任务 | 示例问题 |
|---|---|
| 找回决定 | 上次为什么选了这个安装方案？找到相关讨论，给出原对话位置。 |
| 接续工作 | 昨天这个项目做到哪里了？找出最后确认的结果、没做完的事和下一步。 |
| 复盘一周 | 这周在这个项目做了什么？区分已验证的结果和还需要检查的工作。 |
| 对比用量 | 这个项目本周和上周的用量有什么变化？主要差在哪些任务和模型？ |
| 核对费用 | 这周的用量按 API 价格估算是多少钱？哪些模型还没计价？ |
| 排查反复出现的问题 | 这个安装问题反复出现，结合相关记录和当前代码，看看还有哪里没解决，建议怎么改。 |
| 验证修改 | 按原来的安装失败场景复查这次修复。哪些通过了，哪些还没有验证？ |
| 清理空间 | 看看 Codex 的旧版本和缓存占多少空间，哪些可以清理，先不要删除。 |

以这些问题为起点，试着用自己的话描述日常任务，探索更多用法。说明项目、时间范围或具体问题，有助于缩小范围。也可以继续追问：「定位到原对话」「给出最小改动」「改完按原场景再查」。

更多用法见[插件指南](docs/guides/plugin.md)，直接查询见 [CLI 指南](docs/guides/cli.md)。

## 使用须知

- CLI 在本机分析，不上传日志、不调用模型。Skill 选取的证据会进入当前 Codex 对话并消耗 Token。缺价时可能下载官方价表，详见[隐私说明](docs/reference/privacy.md)。
- API 等价金额是估算值，不是订阅账单。缺价不等于零费用，账户额度单独计算，详见[计价说明](docs/reference/pricing.md)。
- 清理需要对选定对象的授权；保护会话、状态、配置和凭据，条件变化或无法判断时停止。详见[清理指南](docs/guides/cli.md)。

发行包支持 macOS arm64/x64、Linux glibc arm64/x64 和 Windows x64。Codex 产物清理目前针对 macOS 目录布局。不受支持的记录格式如何处理，见[格式恢复说明](docs/reference/cli.md)。

## 更新

重新执行上面的安装命令，可以更新 Wombat 与 Codex 插件。保留指定版本或自定义安装目录的选项。

更新选项和仅更新运行时的方式见[安装指南](docs/guides/installation.md)。

## 帮助与贡献

直接查询、JSON 输出和预算控制见 [CLI 指南](docs/guides/cli.md)。源码环境配置见[贡献指南](.github/CONTRIBUTING.zh-CN.md)。

[反馈问题或需求](https://github.com/YannByte/wombat/issues)时，描述现象与所用工具，无需提供私有日志。安全问题请按[安全政策](.github/SECURITY.zh-CN.md)报告。

## 许可证

[MIT](LICENSE)。依赖许可证见[第三方声明](licenses/THIRD_PARTY_NOTICES.md)。
