<p align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="assets/wombat-logo-dark.svg">
    <img src="assets/wombat-logo-light.svg" alt="Wombat" width="80" height="56">
  </picture>
</p>

# Wombat — Codex Token 用量分析与配置检查

中文 | [English](README.md)

Wombat 分析本机 Codex 记录，找出推高 Token 用量的任务，查看已记录的耗时。根据本机记录中的证据，检查 AGENTS.md、Skills、MCP 和 Hooks。

通过 Codex 插件提问。需要图表和详细记录时，打开本机 Web 面板。

## 开始使用

**正式版：[`v0.3.4`](https://github.com/YannByte/wombat/releases/tag/v0.3.4)。**

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

> $wombat:wombat 这周的 Token 用量增加在哪里？找出主要任务和轮次。

回答会列出主要贡献，以及输入、缓存和输出的变化。需要核对记录时，可以让 Skill 打开对应的 Web 详情。

已有采集插件使用 `$wombat-collection:wombat`。存在多个实例时，选择该项目实际发现的调用名称。详见[插件指南](docs/guides/plugin.md)。

插件安装失败时，Wombat 会保留。请按[安装恢复步骤](docs/guides/installation.md)处理。仅使用 Web 时，安装命令省略 `--plugin` 或 `-Plugin`。

## 主要用途

- 比较任务用量和周期增长，查看变化涉及的任务与轮次。
- 查看已记录的时长、活动区间、重叠和覆盖缺口。
- 找出跨任务的重复操作，供 Codex 判断是否适合整理为脚本或 Skill。
- 检查项目指令与扩展，审阅具体位置、依据和建议操作。
- 保存配置检查的用户决定。完成授权修改后，使用相同规则复查。
- 设置日／周／月 Token 预算，复盘最近结束的周期。

持续预算检查需要运行 `wombat monitor watch`，或在打开的 Web 预算面板中启用定期检查。

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
- 本机分析不上传日志、不调用模型。缺价时可能下载官方价表，见[隐私说明](docs/reference/privacy.md)。
- Token、API 等价估算金额和账户额度分别展示。估算金额不是订阅账单，见[计价说明](docs/reference/pricing.md)。

不受支持的数据格式如何处理，见[格式恢复说明](docs/reference/cli.md)。

## 帮助与贡献

JSON 查询与预算控制见 [CLI 指南](docs/guides/cli.md)。源码环境配置见[贡献指南](.github/CONTRIBUTING.zh-CN.md)。

[反馈问题或需求](https://github.com/YannByte/wombat/issues)时，描述现象与所用工具，无需提供私有日志。安全问题请按[安全政策](.github/SECURITY.zh-CN.md)报告。

## 许可证

[MIT](LICENSE)。依赖许可证见[第三方声明](licenses/THIRD_PARTY_NOTICES.md)。
