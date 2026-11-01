# Wombat 用户插件

中文 | [English](plugin.en.md)

`plugin/skills/wombat/` 是唯一用户主 Skill，按任务加载参考；开发工作流位于 `.agents/skills/`。元数据由 `plugin/package.json` 与 `agents/openai.yaml` 维护，根清单独占版本号。主 Skill 的[共用规则](../../plugin/skills/wombat/SKILL.md)使用清楚、简洁的中英文，不要求严格词典匹配。

## 构建与安装

```sh
corepack pnpm build
codex plugin marketplace add ./dist/skill
codex plugin add wombat@wombat-local
```

构建只生成 `dist/skill/plugin` 与单条本地 marketplace，发行包使用 `lib/skill`。`wombat-runtime.json` 绑定内容哈希、版本和所需能力。没有独立 Skill 副本或采集插件资源。Codex 管理安装、更新和移除；`wombat system update` 只更新运行时，完整更新见[安装指南](../guides/installation.md)。

## 验证边界

`corepack pnpm verify:skill-native -- --output-dir /tmp/wombat-native --codex-bin /absolute/path/to/codex` 使用全新配置，验证单插件安装、发现、运行时兼容及保留数据的移除。默认 setup 不探测 Hooks；显式 `--hooks` 才检查可选注册。插件安装不代表已建立索引或事件覆盖。

构建后可用 `corepack pnpm verify:skill-conversation -- --output-dir /tmp/wombat-conversations --agent-bin /absolute/path/to/codex --language both` 做需授权的模型对话测试。`--scenario` 选择 usage、pricing、timing、statistics、monitor、account、daily-review、task-retro 或 config-review；`all` 运行全部。每例使用全新数据与安装目录；只有该例后续对话延续状态。配置例只授权修改选定 description，并按原 readView/baselineId 复查。报告保存用量与事实，建议质量仍需人工审阅。没有运行该检查时，不能声称已验证模型建议效果。
