# Wombat 插件源码

中文 | [English](README.md)

本目录包含用户插件源码。每个 Skill 同时支持中文和英文。Codex 管理已安装的插件。

## 源码入口

- `skills/` 包含[查询与任务入口](../docs/guides/plugin.md)和共用运行参考。
- `hooks/` 包含采集声明。
- `scripts/` 包含 POSIX 采集桥接脚本。
- `package.json` 定义插件元数据、分发的 Skill 名称和所需能力。根清单维护版本号。

## 限制

从源码安装本地插件前，先完成构建。插件安装、发现、Hook 信任、事件接收和数据就绪需要分别验证。Windows Hook 采集尚未验证。

## 延伸阅读

- [安装指南](../docs/guides/installation.md)：安装或更新运行时和插件。
- [插件指南](../docs/guides/plugin.md)：调用、采集和移除。
- [插件开发](../docs/development/plugin.md)：构建资源、本地试用和原生验证。
- [插件指令](AGENTS.md)：维护规则。
