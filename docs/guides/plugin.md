# 使用 Codex 插件

中文 | [English](plugin.en.md)

先按[安装指南](installation.md)安装 Wombat 与兼容的本机 Codex。在项目的新对话中调用 `$wombat:wombat`。

## 按任务取证

一个主 Skill 按任务加载流程：继续工作、排查障碍、验证交付、阶段复盘或改进项目机制。简单用量与费用问题直接查询 CLI。无需初始化项目，也不预先收集所有证据。

检查先只读。建议应列出具体问题、证据位置、负责文件、最小修改与复查条件。Codex 负责授权后的编辑、测试、任务创建和恢复。Wombat 提供事实与证据；静态复查不证明后续采用或节省。

## 更新与移除

重新执行带插件选项的安装命令，可更新运行时和插件。`wombat update` 只更新运行时。Codex 负责插件管理：

```sh
codex plugin remove wombat@wombat-local
```

移除插件保留产品数据。发行包只包含一个插件和一个主 Skill，没有自动 Hook 或独立 Skill 安装方式。

## 可选观察与限制

日志查询无需 Hooks。需要检查原生 Hook 注册时，显式运行 `wombat setup --hooks --project /path/to/project --json`。自管事件接收见 [CLI 指南](cli.md)；接收记录不新增 Token，也不证明完整覆盖。

Wombat 查询不调用模型。Codex 对话和授权工作消耗模型 Token。插件发现、运行时兼容、数据就绪和具体对话效果需要分别验证。跨平台与完整原生事件覆盖仍有限制，见[开发说明](../development/plugin.md)。
