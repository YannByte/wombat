<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="assets/wombat-logo-dark.svg">
    <img src="assets/wombat-logo-light.svg" alt="Wombat" width="80" height="56">
  </picture>
</p>

# Wombat：Codex 历史检索、用量分析与项目诊断

中文 | [English](README.md)

Wombat 是 OpenAI Codex 的插件和本地 CLI。可以检索会话历史、统计 Token 用量、诊断项目问题，并检查哪些文件可以安全清理。

用自然语言找回之前的决定、接续工作或排查反复出现的问题。Wombat 检索本机证据、计算用量，Codex 根据证据解释问题、执行授权的修改并按原场景复查。

## 看看会得到什么

### 看清项目进展和用量

> **提问：** 这周这个项目进展如何？Token 主要用在哪，下一步先做什么？

**回答示例：** 本周主要在做安装修复和测试补充，共使用 120 万 Token，其中 90 万来自安装排查。

相关记录显示，macOS 安装已复查通过；Windows 安装失败后改过代码，尚未找到成功复查的结果。相关讨论位于「安装排查」第 18 至 21 轮。下一步先按原失败场景验证 Windows 安装，再决定是否需要继续修复。

### 解决反复出现的问题

> **提问：** 这个项目已经两次在发布后发现包里缺文件。看看之前怎么处理的，这次怎么避免再发生？

**回答示例：** 前两次都是补齐文件后重新打包。检查当前代码发现，`scripts/ship.ts` 上传前没有调用已有的 `scripts/check-package.ts`。建议把这项检查接到上传前，失败时停止发布。

> **继续提问：** 按这个方案改，用正常包和缺文件的包分别复查。

**复查示例：** 同一发布入口下，正常包通过；缺文件的包被检查拦住，上传步骤未执行。

### 清理工作留下的临时文件和编译产物

> **提问：** 这次工作结束了，看看项目里留下了哪些临时文件、测试输出和编译产物。先列清单，别直接删。

**清理预览示例：** 两个可清理候选约占 2.6 GB：

- `target/`：2.4 GB 的编译缓存，可通过重新编译生成。
- `.tmp/test-run-42/`：本次测试创建的临时目录，180 MB。测试已结束，后续步骤不再使用。

源码、配置、最终测试报告和待交付的安装包保留。尚未删除；清理会永久删除选中的两个目录。

> **继续提问：** 清理这两项，保留报告和安装包，然后复查。

**复查示例：** 两个目录的原路径已不存在；源码、配置、最终测试报告和安装包均保留。

## 为 Codex 安装 Wombat

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

回答会使用提问时的语言，包含时间范围、总用量、主要任务和缺失数据。安装遇到问题时，按提示恢复，详见[安装指南](docs/guides/installation.md)。

## 还可以这样问

用 `$wombat:wombat` 提问，例如：

| 任务 | 示例问题 |
|---|---|
| 找回决定 | 上次为什么选了这个安装方案？找到相关讨论，给出原对话位置。 |
| 接续工作 | 昨天这个项目做到哪里了？找出最后确认的结果、没做完的事和下一步。 |
| 复盘一周 | 这周在这个项目做了什么？区分已验证的结果和还需要检查的工作。 |
| 对比用量 | 这个项目本周和上周的用量有什么变化？主要差在哪些任务和模型？ |
| 排查反复出现的问题 | 这个安装问题反复出现，结合相关记录和当前代码，看看还有哪里没解决，建议怎么改。 |
| 验证修改 | 按原来的安装失败场景复查这次修复。哪些通过了，哪些还没有验证？ |
| 清理空间 | 这个项目有哪些临时文件、测试输出和编译产物可以清理？先列清单，保留要交付的文件。 |

以这些问题为起点，试着用自己的话描述日常任务，探索更多用法。说明项目、时间范围或具体问题，有助于缩小范围。也可以继续追问：「定位到原对话」「给出最小改动」「改完按原场景再查」。

更多用法见[插件指南](docs/guides/plugin.md)，直接查询见 [CLI 指南](docs/guides/cli.md)。

## 常见问题

### 如何搜索 Codex 的历史对话？

用 `$wombat:wombat` 查找之前的讨论、决定或未完成的任务。补充项目、时间范围或记得的关键词，Wombat 会定位相关记录并给出原对话位置。

### 不安装插件也能使用 Wombat 吗？

可以。安装时省略 `--plugin` 或 `-Plugin`，直接使用 CLI。CLI 查询支持 JSON 输出，详见 [CLI 指南](docs/guides/cli.md)。

## 使用须知

- CLI 在本机分析，不上传日志、不调用模型。Skill 选取的证据会进入当前 Codex 对话并消耗 Token，详见[隐私说明](docs/reference/privacy.md)。
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
