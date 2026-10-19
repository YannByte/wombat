# 使用 Codex 插件

中文 | [English](plugin.en.md)

先安装兼容的本机 Codex。按[安装指南](installation.md)安装 Wombat 与插件。

## 选择调用名称

在 Codex 中打开项目，新建对话。使用该项目实际发现的调用名称：

- 基础插件使用 `$wombat:wombat`。
- 采集插件使用 `$wombat-collection:wombat`。
- 独立 Skill 副本使用 `$wombat`。

存在多个实例时，明确选择其中一个。安装不会扫描日志，也不能证明数据已经就绪。

插件提供以下入口，按用户任务选择：

| Skill | 任务 | 问法示例 |
|---|---|---|
| `wombat` | 直接查询、预算与接入 | “今天用了多少 Token？” |
| `wombat-daily-review` | 每日或阶段用量复盘 | “复盘今天的用量，给一个有依据的改善建议。” |
| `wombat-task-retro` | 调查一项已选任务 | “这项任务为什么反复折腾？给我看原始依据。” |
| `wombat-config-review` | 审查配置并复查授权修改 | “审查这个项目的 Skills，指出具体文件。” |

例如，基础插件使用 `$wombat:wombat-daily-review`，采集插件使用 `$wombat-collection:wombat-daily-review`。其他任务入口使用相同的命名规则。独立副本只提供通用 `$wombat` 入口。回答跟随用户语言，无需分别安装中英文版本。

审查先只读。生成报告不会自动安排后续报告，配置问题也不构成修改授权。配置审查在授权修改后复查原建议；指令变短不等于已证实节省用量。

## 可选采集

历史日志分析无需采集插件。如需原生观察，打开 Web 中的「接入与采集」。状态与暂停／恢复方式见[采集命令](cli.md)。

插件安装、采集模式、原生 Hook 信任和事件接收是独立步骤。在 Codex `/hooks` 中审查采集声明后，再决定是否信任。接收记录不会增加 Token 计量，也不代表完整覆盖。POSIX 桥接已实现；Windows Hook 采集尚未验证。

## 更新和移除

更新运行时与插件时，重新执行带插件选项的安装命令。`wombat update` 只更新运行时。

Codex 管理插件移除。移除选定的基础插件：

```sh
codex plugin remove wombat@wombat-local
```

采集插件使用：

```sh
codex plugin remove wombat-collection@wombat-local
```

从独立副本切换前，运行：

```sh
wombat skill uninstall --json
```

卸载只移除未修改的受管副本。修改过的副本需要人工审查。移除插件会保留产品数据和其他 Hook 声明。

## 限制

Wombat 查询和检查不调用模型。Codex 对话与授权处理会产生模型用量。交接接收成功不代表执行完成或问题已解决；修改后需复查原问题。

本地插件发现和移除曾在 macOS 的 Codex 0.160.0 上通过验证。这些证据不代表其他平台或完整对话流程已通过验收。不声称已提交公开插件市场。本地试用和验证范围见[插件开发](../development/plugin.md)。
