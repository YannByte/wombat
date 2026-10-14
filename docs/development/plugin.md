# Wombat 用户插件

中文 | [English](plugin.en.md)

本目录保存面向用户的插件源码。`skills/` 存放 Skill 工作流，`hooks/` 存放 Hook 声明，`scripts/` 存放采集脚本。仓库开发工作流保留在 `.agents/skills/`。唯一 Skill 入口为 [skills/wombat/SKILL.md](../../plugin/skills/wombat/SKILL.md)，按任务读取相应参考文件。一个 Skill 同时服务中文和英文。 共用的[回答规范](../../plugin/skills/wombat/references/expression.md)将 STE 写作原则用于两种语言。英文使用标准的句长限制；中文使用自然语法，不套用英文词数限制。这些指引以清晰、自然的表达为主，不要求严格匹配词典，也不代表生成的回答已通过合规认证。`agents/openai.yaml` 使用 JSON 形式的 YAML，维护显示名称、简短说明和默认 Prompt；构建从该源生成插件入口，并替换插件调用名。维护规则见 [AGENTS.md](../../plugin/AGENTS.md)。

构建从 `skills/wombat/` 提取 `dist/skill/wombat` 独立资源，生成 `dist/skill/plugin` 插件和本地 marketplace，并只向采集包加入采集资源。发行包继续将这些资源放在 `lib/skill` 下。`package.json` 维护插件元数据和所需能力，版本只由根清单维护。构建生成的 `plugin.json` 和 `.codex-plugin/plugin.json` 提供所支持的宿主格式；本源码目录需先构建再安装插件。独立清单绑定版本、来源提交、文件列表和哈希，`wombat-runtime.json` 绑定插件版本、内容哈希和所需能力。两个安装方式共享 Skill 内容哈希与所需运行能力。推荐使用[安装命令](../guides/installation.md)，一并安装运行时与 Codex 插件，再检查发现状态和运行时兼容性；是否打开 Web 可选。已有已启用的采集插件会保留；首次安装选择基础插件。重新执行带插件选项的安装命令，可将本地 marketplace 更新到所选运行时版本。仅重试插件时复用当前受管运行时，不下载发行包，也不修改 PATH；选项见安装指南。

正式插件由 Codex 管理，当前本地试用入口为：

```sh
corepack pnpm build
codex plugin marketplace add ./dist/skill
codex plugin add wombat@wombat-local
```

插件在 Codex 0.160.0 中的调用名为 $wombat:wombat。独立试用可执行 wombat skill install，默认安装到 ~/.agents/skills/wombat，调用名为 $wombat；status 单独报告文件与原生发现状态，uninstall 只移除未被用户改动的受管副本。两种方式择一安装；已有多个实例时由用户选择。安装不扫描日志、不读取登录凭据，不代表数据就绪。

已在 macOS 的 Codex 0.160.0 上验证本地 marketplace 安装、启用发现和移除。打包后的 marketplace 根位于所选受管版本的 lib/skill 下；尚未提交公共 marketplace，也未提供公共插件安装页面。Codex 管理插件更新与移除；只更新运行时的 `wombat update` 命令不会更新插件。使用 `codex plugin remove wombat@wombat-local` 或 `codex plugin remove wombat-collection@wombat-local` 移除所选插件。从独立副本切换前，可用 `wombat skill uninstall --json` 移除未修改的受管副本；已修改副本保留，需手动审阅。发现、队列接受、浏览器交互和数据就绪需要分别验收；其他平台和完整对话流程仍未验收。

双入口、初始化和分工见[产品方案](../decisions/proposed/product/2026-10-04-codex-skill.md)，参数与范围见 [CLI 指南](../guides/cli.md)，本机运行时安装见[安装指南](../guides/installation.md)。

## 本地采集插件

同一构建还生成 `dist/skill/collection-plugin` 和 `wombat-collection@wombat-local` 目录项。它复用唯一 Skill，包含 `hooks/` 中受审的 Hook 声明和 `scripts/` 中的最小 POSIX Shell 桥接资源。仅含 Skill 的基础插件不包含这两个目录，也不声明生命周期 Hooks。插件与独立 Skill 方式择一使用，避免多个启用实例；调用名以原生发现为准。

源码构建试用先按上文注册 `./dist/skill`，再执行 `codex plugin add wombat-collection@wombat-local`，替代基础插件安装。发行包的同一 marketplace 位于 `lib/skill`。桥接优先使用带所有权标记的用户受管启动器，其次使用 Codex PATH 中的 `wombat`；自定义安装前缀需加入该 PATH。采集模式与原生 Hooks 信任仍需分别操作，见 [CLI 采集指南](../guides/cli.md)。安装或移除插件不删除产品数据、不修改其他 Hook 声明。POSIX 桥接已实现；Windows 与完整原生事件仍未验收。Hooks 插件用于本机手动安装，不声称已在公共目录发布。

本地采集包仅使用生成的 `.codex-plugin/plugin.json`。Codex 0.160.1 在存在根清单时不返回该包的 Hooks 注册；纯 Skill 插件继续保留标准根清单。原生安装验收需检出这一差异。使用全新 profile 运行 `corepack pnpm verify:skill-native -- --output-dir /tmp/wombat-native --codex-bin /absolute/path/to/codex`，检查安装、发现、未信任的注册及保留数据的移除。这不证明 Hooks 已执行或完成公开分发。

构建后运行 `corepack pnpm verify:skill-conversation -- --output-dir /tmp/wombat-conversations --agent-bin /absolute/path/to/codex --language both`，以合成用量、额度和配置验收选定的真实 Codex 对话。默认 `workflow` 场景通过 Web 核对同一固定版本与项目，恢复原会话执行获授权的 description 修改，再检查原建议、用户决定及导航文件未被改写。`--scenario usage|pricing|timing|statistics|monitor|account` 选择单项读取，`--scenario all` 选择全部场景；`--variant 0..3` 选择单项读取的中英文自然问法变体。用量续问依次改变日期范围和授权项目，保留原快照。每项场景在准备时确认产品数据和运行时路径不存在；仅该场景的续问复用本轮生成的状态。检查能力范围、独立事实，以及来源、配置、决定和预算未被改动。报告保留命令数、输出字节数和原生 Token 观测；自然回答的清晰度与限定说明由人工审阅。使用全新的仓库外输出目录。原生模型执行使用当前认证 profile，不读取或复制凭据。该检查不改变 Hooks 信任，也不证明所有真实项目流程已验收。

在原生 `codex` → `/hooks` 中审核并信任采集声明后，运行 `corepack pnpm verify:skill-native-events -- --output-dir /tmp/wombat-native-events --codex-bin /absolute/path/to/codex --wombat-bin /absolute/path/to/wombat --project /absolute/project --source-root /absolute/codex-home`。该真实 profile 检查创建一项新的只读 `pwd` 任务，验证常用事件的实际接收，再依据已提交日志事实复查任务与轮次的精确关联。它保留源文件和凭据，在仓库外存储私有证据，并在产品存储中保留安全接收记录与派生索引。它要求已有原生信任，不授予或绕过信任；未观察到的生命周期事件仍未验收。 可在新的输出目录追加 `--session UUID`，只复查该原生会话，不重复执行模型任务。日志准备最多等待 5 分钟，超时或存储失败保留证据；固定用量版本复查不重复计量。
