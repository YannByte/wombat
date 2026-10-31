# 使用 Codex 插件

中文 | [English](plugin.en.md)

先安装兼容的本机 Codex。按[安装指南](installation.md)安装 Wombat 与插件。

## 选择调用名称

在 Codex 中打开项目，新建对话。使用该项目实际发现的调用名称：

- 基础插件使用 `$wombat:wombat`。
- 采集插件使用 `$wombat-collection:wombat`。
- 独立 Skill 副本使用 `$wombat`。

存在多个实例时，明确选择其中一个。安装不会扫描日志，也不能证明数据已经就绪。

一个主 Skill 按任务加载接续工作、排查阻碍、核对交付、阶段复盘或工程改善流程。简单用量与费用问题直接查询 CLI。Skill 不会先初始化项目，也不会为每个问题收集全部证据。

检查从只读开始。每项有依据的改善建议都指出已观察的问题、证据、具体负责文件、最小改法和原问题的复查方法。Codex 执行已授权的修改与检查。没有使用记录不能作为删除依据。静态复查通过不代表后续采用或节省。

## 可选采集

历史日志分析无需采集插件。原生观察可显式启用。状态与暂停／恢复方式见[采集命令](cli.md)。

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
